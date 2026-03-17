use bytes::Bytes;
use super::events::{AudioConfig, TurnId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Superseded,
    ClientRequest,
    Disconnect,
    Timeout,
    Error,
}

#[derive(Debug, Clone)]
pub enum Command {
    StartTurn { turn: TurnId },
    CancelTurn { turn: TurnId, reason: CancelReason },
    ResetContext,

    AsrStart { turn: TurnId, config: AudioConfig },
    AsrAudioFrame { turn: TurnId, pcm16: Bytes, sample_rate: u32 },
    AsrFinalize { turn: TurnId },
    AsrCancel { turn: TurnId },

    LlmStart { turn: TurnId, text: String },
    LlmCancel { turn: TurnId },

    TtsStart { turn: TurnId, text: String },
    TtsCancel { turn: TurnId },

    ClearAudioOutput { turn: TurnId },

    SendAsrPartial { turn: TurnId, text: String, start_ms: u32, end_ms: u32 },
    SendAsrFinal { turn: TurnId, text: String, start_ms: u32, end_ms: u32 },
    SendLlmDelta { turn: TurnId, seq: u32, text: String },
    SendTtsMeta { turn: TurnId, audio_offset_ms: u32, text_span: (u32, u32) },
    SendTextToClient { turn: TurnId, text: String },
    SendAudioToClient { turn: TurnId, chunk: Bytes, is_last: bool },
    SendUiAction { name: String, data: String },
    SendError { code: String, message: String },
}
