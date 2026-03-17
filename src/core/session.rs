//! Session run loop: receives events, feeds them to the state machine,
//! and executes the resulting commands.

use tokio::sync::mpsc;

use super::bridge::OutMessage;
use super::commands::Command;
use super::events::{Event, SessionId};
use super::metrics::{SessionMetrics, TurnMetrics};
use super::state::DialogueState;
use crate::protocol::ServerMessage;

pub struct Session {
    id: SessionId,
    ev_rx: mpsc::Receiver<Event>,
    out_tx: mpsc::Sender<OutMessage>,
    state: DialogueState,
    session_metrics: SessionMetrics,
    /// Metrics for the currently active turn, if any.
    current_turn_metrics: Option<TurnMetrics>,
}

impl Session {
    pub fn new(
        id: SessionId,
        ev_rx: mpsc::Receiver<Event>,
        out_tx: mpsc::Sender<OutMessage>,
    ) -> Self {
        Self {
            id,
            ev_rx,
            out_tx,
            state: DialogueState::new(),
            session_metrics: SessionMetrics::new(),
            current_turn_metrics: None,
        }
    }

    /// Execute a single command produced by the state machine.
    async fn exec(&mut self, cmd: Command) {
        tracing::debug!(session=?self.id, ?cmd, "exec command");

        match cmd {
            Command::StartTurn { turn } => {
                self.current_turn_metrics = Some(TurnMetrics::new(turn));
                tracing::info!(session=?self.id, turn=turn.0, "turn started");
            }

            Command::CancelTurn { turn, reason } => {
                self.session_metrics.turns_cancelled += 1;
                if let Some(ref mut m) = self.current_turn_metrics {
                    if m.turn == turn {
                        m.cancel_requested = Some(std::time::Instant::now());
                    }
                }
                tracing::info!(session=?self.id, turn=turn.0, ?reason, "turn cancelled");
            }

            Command::ResetContext => {
                tracing::info!(session=?self.id, "context reset");
            }

            // --- ASR commands (forwarded to ASR worker in Phase 3) ---
            Command::AsrStart { turn, .. } => {
                tracing::debug!(session=?self.id, turn=turn.0, "asr start (stub)");
            }
            Command::AsrAudioFrame { turn, .. } => {
                // Record first audio frame time for metrics.
                if let Some(ref mut m) = self.current_turn_metrics {
                    if m.turn == turn && m.asr_first_audio.is_none() {
                        m.asr_first_audio = Some(std::time::Instant::now());
                    }
                }
            }
            Command::AsrFinalize { turn } => {
                tracing::debug!(session=?self.id, turn=turn.0, "asr finalize (stub)");
            }
            Command::AsrCancel { turn } => {
                tracing::debug!(session=?self.id, turn=turn.0, "asr cancel (stub)");
            }

            // --- LLM commands (forwarded to LLM worker in Phase 4) ---
            Command::LlmStart { turn, .. } => {
                tracing::debug!(session=?self.id, turn=turn.0, "llm start (stub)");
            }
            Command::LlmCancel { turn } => {
                tracing::debug!(session=?self.id, turn=turn.0, "llm cancel (stub)");
            }

            // --- TTS commands (forwarded to TTS worker in Phase 5) ---
            Command::TtsStart { turn, .. } => {
                tracing::debug!(session=?self.id, turn=turn.0, "tts start (stub)");
            }
            Command::TtsCancel { turn } => {
                tracing::debug!(session=?self.id, turn=turn.0, "tts cancel (stub)");
            }
            Command::ClearAudioOutput { turn } => {
                tracing::debug!(session=?self.id, turn=turn.0, "clear audio output (stub)");
            }

            // --- Send commands (write to WebSocket via out_tx) ---
            Command::SendAsrPartial { turn, text, start_ms, end_ms } => {
                let msg = ServerMessage::AsrPartial {
                    turn_id: turn.0,
                    text,
                    start_ms,
                    end_ms,
                };
                self.send_text(msg).await;
            }
            Command::SendAsrFinal { turn, text, start_ms, end_ms } => {
                if let Some(ref mut m) = self.current_turn_metrics {
                    if m.turn == turn {
                        m.asr_final = Some(std::time::Instant::now());
                    }
                }
                let msg = ServerMessage::AsrFinal {
                    turn_id: turn.0,
                    text,
                    start_ms,
                    end_ms,
                };
                self.send_text(msg).await;
            }
            Command::SendLlmDelta { turn, seq, text } => {
                // Record first LLM token time for TTFT metric.
                if let Some(ref mut m) = self.current_turn_metrics {
                    if m.turn == turn && m.llm_first_token.is_none() {
                        m.llm_first_token = Some(std::time::Instant::now());
                    }
                }
                let msg = ServerMessage::LlmDelta {
                    turn_id: turn.0,
                    seq,
                    text,
                };
                self.send_text(msg).await;
            }
            Command::SendTtsMeta { turn, audio_offset_ms, text_span } => {
                let msg = ServerMessage::TtsMeta {
                    turn_id: turn.0,
                    audio_offset_ms,
                    text_span,
                };
                self.send_text(msg).await;
            }
            Command::SendTextToClient { turn, text } => {
                let msg = ServerMessage::LlmDelta {
                    turn_id: turn.0,
                    seq: 0,
                    text,
                };
                self.send_text(msg).await;
            }
            Command::SendAudioToClient { turn, chunk, is_last } => {
                // Record first TTS audio time for TTFA metric.
                if let Some(ref mut m) = self.current_turn_metrics {
                    if m.turn == turn && m.tts_first_audio.is_none() {
                        m.tts_first_audio = Some(std::time::Instant::now());
                    }
                }
                let _ = self.out_tx.send(OutMessage::Binary(chunk.to_vec())).await;
                if is_last {
                    if let Some(ref mut m) = self.current_turn_metrics {
                        if m.turn == turn {
                            m.tts_done = Some(std::time::Instant::now());
                            m.emit();
                        }
                    }
                    self.session_metrics.turns_completed += 1;
                }
            }
            Command::SendUiAction { name, data } => {
                let msg = ServerMessage::UiAction { name, data };
                self.send_text(msg).await;
            }
            Command::SendError { code, message } => {
                let msg = ServerMessage::Error { code, message };
                self.send_text(msg).await;
            }
        }
    }

    /// Serialize a ServerMessage and send it as a text frame.
    async fn send_text(&self, msg: ServerMessage) {
        let json = msg.to_json();
        if self.out_tx.send(OutMessage::Text(json)).await.is_err() {
            tracing::warn!(session=?self.id, "failed to send message, client disconnected");
        }
    }

    /// Main event loop.
    pub async fn run(mut self) {
        tracing::info!(session=?self.id, "session started");

        loop {
            tokio::select! {
                maybe_ev = self.ev_rx.recv() => {
                    let Some(ev) = maybe_ev else {
                        tracing::info!(session=?self.id, "event channel closed, session ending");
                        break;
                    };
                    tracing::debug!(session=?self.id, ?ev, "event");

                    let cmds = self.state.handle(ev);
                    for c in cmds {
                        self.exec(c).await;
                    }
                }
            }
        }

        tracing::info!(
            session=?self.id,
            turns_completed = self.session_metrics.turns_completed,
            turns_cancelled = self.session_metrics.turns_cancelled,
            "session ended"
        );
    }
}
