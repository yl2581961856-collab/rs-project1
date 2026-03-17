use super::commands::{CancelReason, Command};
use super::events::{AudioConfig, Event, TurnId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Listening,
    Thinking,
    Speaking,
}

#[derive(Debug)]
pub struct DialogueState {
    pub phase: Phase,
    pub active_turn: Option<TurnId>,
    pub active_turn_started: bool,
    pub next_turn: u64,
    pub audio: AudioConfig,
}

impl DialogueState {
    pub fn new() -> Self {
        Self {
            phase: Phase::Listening,
            active_turn: None,
            active_turn_started: false,
            next_turn: 1,
            audio: AudioConfig::default(),
        }
    }

    fn allocate_turn(&mut self) -> TurnId {
        let turn = TurnId(self.next_turn);
        self.next_turn = self.next_turn.saturating_add(1);
        self.active_turn = Some(turn);
        self.active_turn_started = false;
        turn
    }

    fn supersede_turn(&mut self) -> (TurnId, Option<TurnId>) {
        let prev = self.active_turn.take();
        let turn = self.allocate_turn();
        (turn, prev)
    }

    fn cancel_turn_cmds(&mut self, turn: TurnId, reason: CancelReason) -> Vec<Command> {
        self.active_turn = None;
        self.active_turn_started = false;
        self.phase = Phase::Listening;
        vec![
            Command::CancelTurn { turn, reason },
            Command::AsrCancel { turn },
            Command::LlmCancel { turn },
            Command::TtsCancel { turn },
            Command::ClearAudioOutput { turn },
        ]
    }

    pub fn handle(&mut self, ev: Event) -> Vec<Command> {
        match ev {
            Event::ClientConnected => vec![],
            Event::ClientDisconnected => {
                if let Some(turn) = self.active_turn {
                    return self.cancel_turn_cmds(turn, CancelReason::Disconnect);
                }
                vec![]
            }

            Event::ClientHello { audio } => {
                self.audio = audio;
                vec![]
            }
            Event::ClientPing { .. } => vec![],

            Event::ClientText(text) => {
                let turn = self.active_turn.unwrap_or_else(|| self.allocate_turn());
                self.phase = Phase::Thinking;
                vec![
                    Command::StartTurn { turn },
                    Command::LlmStart { turn, text },
                ]
            }

            Event::ClientAudioFrame { pcm16, sample_rate } => {
                let turn = self.active_turn.unwrap_or_else(|| {
                    self.phase = Phase::Listening;
                    let t = self.allocate_turn();
                    return t;
                });

                let mut cmds = Vec::new();
                if !self.active_turn_started {
                    cmds.push(Command::StartTurn { turn });
                    cmds.push(Command::AsrStart {
                        turn,
                        config: self.audio.clone(),
                    });
                    self.active_turn_started = true;
                }
                cmds.push(Command::AsrAudioFrame {
                    turn,
                    pcm16,
                    sample_rate,
                });
                cmds
            }

            Event::ClientCancel { turn } => {
                let target = turn.or(self.active_turn);
                if let Some(t) = target {
                    return self.cancel_turn_cmds(t, CancelReason::ClientRequest);
                }
                vec![]
            }
            Event::ClientReset => {
                let mut cmds = Vec::new();
                if let Some(t) = self.active_turn {
                    cmds.extend(self.cancel_turn_cmds(t, CancelReason::ClientRequest));
                }
                cmds.push(Command::ResetContext);
                cmds
            }

            Event::VadSpeechStart => {
                let (turn, prev) = self.supersede_turn();
                let mut cmds = Vec::new();
                if let Some(t) = prev {
                    // cancel_turn_cmds clears active_turn, so we must restore
                    // the newly allocated turn after cancelling the old one.
                    cmds.extend(self.cancel_turn_cmds(t, CancelReason::Superseded));
                }
                // Restore new turn state that cancel_turn_cmds may have cleared.
                self.active_turn = Some(turn);
                self.active_turn_started = true;
                self.phase = Phase::Listening;
                cmds.push(Command::StartTurn { turn });
                cmds.push(Command::AsrStart {
                    turn,
                    config: self.audio.clone(),
                });
                cmds
            }
            Event::VadSpeechEnd => {
                if let Some(turn) = self.active_turn {
                    return vec![Command::AsrFinalize { turn }];
                }
                vec![]
            }

            Event::AsrPartial {
                turn,
                text,
                start_ms,
                end_ms,
            } => {
                if self.active_turn == Some(turn) {
                    return vec![Command::SendAsrPartial {
                        turn,
                        text,
                        start_ms,
                        end_ms,
                    }];
                }
                vec![]
            }
            Event::AsrFinal {
                turn,
                text,
                start_ms,
                end_ms,
            } => {
                if self.active_turn == Some(turn) {
                    self.phase = Phase::Thinking;
                    return vec![
                        Command::SendAsrFinal {
                            turn,
                            text: text.clone(),
                            start_ms,
                            end_ms,
                        },
                        Command::LlmStart { turn, text },
                    ];
                }
                vec![]
            }

            Event::LlmDelta { turn, seq, text } => {
                if self.active_turn == Some(turn) {
                    self.phase = Phase::Speaking;
                    return vec![Command::SendLlmDelta { turn, seq, text }];
                }
                vec![]
            }

            Event::TtsMeta {
                turn,
                audio_offset_ms,
                text_span,
            } => {
                if self.active_turn == Some(turn) {
                    return vec![Command::SendTtsMeta {
                        turn,
                        audio_offset_ms,
                        text_span,
                    }];
                }
                vec![]
            }
            Event::TtsAudio {
                turn,
                chunk,
                is_last,
            } => {
                if self.active_turn == Some(turn) {
                    self.phase = Phase::Speaking;
                    return vec![Command::SendAudioToClient {
                        turn,
                        chunk,
                        is_last,
                    }];
                }
                vec![]
            }

            Event::BackendError { code, message, .. } => {
                vec![Command::SendError { code, message }]
            }
            Event::Timeout { .. } => {
                if let Some(turn) = self.active_turn {
                    return self.cancel_turn_cmds(turn, CancelReason::Timeout);
                }
                vec![]
            }
        }
    }
}
