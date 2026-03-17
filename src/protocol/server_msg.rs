//! Server -> Client JSON message types.
//!
//! Matches the WebSocket protocol defined in README.md.

use serde::Serialize;

/// All possible server -> client text messages.
///
/// Serialized to JSON with a `type` field discriminator.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Session established.
    Session { session_id: String },
    /// ASR partial transcript.
    #[serde(rename = "asr.partial")]
    AsrPartial {
        turn_id: u64,
        text: String,
        start_ms: u32,
        end_ms: u32,
    },
    /// ASR final transcript.
    #[serde(rename = "asr.final")]
    AsrFinal {
        turn_id: u64,
        text: String,
        start_ms: u32,
        end_ms: u32,
    },
    /// LLM token delta.
    #[serde(rename = "llm.delta")]
    LlmDelta {
        turn_id: u64,
        seq: u32,
        text: String,
    },
    /// TTS metadata (alignment info).
    #[serde(rename = "tts.meta")]
    TtsMeta {
        turn_id: u64,
        audio_offset_ms: u32,
        text_span: (u32, u32),
    },
    /// UI action command for the frontend.
    #[serde(rename = "ui.action")]
    UiAction { name: String, data: String },
    /// Pong response to client ping.
    Pong { ts: u64 },
    /// Error message.
    Error { code: String, message: String },
}

impl ServerMessage {
    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        // ServerMessage is always valid for serialization, unwrap is safe.
        serde_json::to_string(self).expect("ServerMessage serialization should never fail")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_msg_json() {
        let msg = ServerMessage::Session {
            session_id: "abc-123".to_string(),
        };
        let json: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
        assert_eq!(json["type"], "session");
        assert_eq!(json["session_id"], "abc-123");
    }

    #[test]
    fn asr_partial_json() {
        let msg = ServerMessage::AsrPartial {
            turn_id: 1,
            text: "hello".to_string(),
            start_ms: 0,
            end_ms: 800,
        };
        let json: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
        assert_eq!(json["type"], "asr.partial");
        assert_eq!(json["turn_id"], 1);
        assert_eq!(json["text"], "hello");
    }

    #[test]
    fn llm_delta_json() {
        let msg = ServerMessage::LlmDelta {
            turn_id: 1,
            seq: 42,
            text: "world".to_string(),
        };
        let json: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
        assert_eq!(json["type"], "llm.delta");
        assert_eq!(json["seq"], 42);
    }

    #[test]
    fn error_msg_json() {
        let msg = ServerMessage::Error {
            code: "ERR_BAD_REQUEST".to_string(),
            message: "missing type field".to_string(),
        };
        let json: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["code"], "ERR_BAD_REQUEST");
    }

    #[test]
    fn pong_msg_json() {
        let msg = ServerMessage::Pong { ts: 1_700_000_000 };
        let json: serde_json::Value = serde_json::from_str(&msg.to_json()).unwrap();
        assert_eq!(json["type"], "pong");
        assert_eq!(json["ts"], 1_700_000_000u64);
    }
}
