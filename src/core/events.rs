use bytes::Bytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TurnId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioCodec {
    Pcm16,
    Opus,
}

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub codec: AudioCodec,
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_ms: u16,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            codec: AudioCodec::Pcm16,
            sample_rate: 16_000,
            channels: 1,
            frame_ms: 20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeoutKind {
    ReadIdle,
    WriteIdle,
    Heartbeat,
}

#[derive(Debug, Clone)]
pub enum Event {
    ClientConnected,
    ClientDisconnected,

    ClientHello { audio: AudioConfig },
    ClientPing { ts: u64 },
    ClientText(String),

    ClientAudioFrame {
        pcm16: Bytes,
        sample_rate: u32,
    },

    ClientCancel { turn: Option<TurnId> },
    ClientReset,

    VadSpeechStart,
    VadSpeechEnd,

    AsrPartial {
        turn: TurnId,
        text: String,
        start_ms: u32,
        end_ms: u32,
    },
    AsrFinal {
        turn: TurnId,
        text: String,
        start_ms: u32,
        end_ms: u32,
    },

    LlmDelta {
        turn: TurnId,
        seq: u32,
        text: String,
    },

    TtsMeta {
        turn: TurnId,
        audio_offset_ms: u32,
        text_span: (u32, u32),
    },
    TtsAudio {
        turn: TurnId,
        chunk: Bytes,
        is_last: bool,
    },

    BackendError {
        turn: Option<TurnId>,
        code: String,
        message: String,
    },
    Timeout { kind: TimeoutKind },
}
