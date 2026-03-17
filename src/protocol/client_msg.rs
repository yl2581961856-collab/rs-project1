//! Client -> Server JSON message types.
//!
//! Matches the WebSocket protocol defined in README.md.

use serde::Deserialize;

/// Audio configuration sent by the client in the `hello` message.
#[derive(Debug, Clone, Deserialize)]
pub struct AudioConfigMsg {
    #[serde(default = "default_codec")]
    pub codec: String,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_channels")]
    pub channels: u8,
    #[serde(default = "default_frame_ms")]
    pub frame_ms: u16,
}

fn default_codec() -> String {
    "pcm16".to_string()
}
fn default_sample_rate() -> u32 {
    16_000
}
fn default_channels() -> u8 {
    1
}
fn default_frame_ms() -> u16 {
    20
}

impl Default for AudioConfigMsg {
    fn default() -> Self {
        Self {
            codec: default_codec(),
            sample_rate: default_sample_rate(),
            channels: default_channels(),
            frame_ms: default_frame_ms(),
        }
    }
}

/// All possible client -> server text messages.
///
/// Deserialized from JSON with a `type` field discriminator.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Client initiates session with audio config.
    Hello {
        #[serde(default)]
        audio: AudioConfigMsg,
    },
    /// Client sends supplementary text.
    Text { text: String },
    /// Client requests cancellation of the current turn.
    Cancel { turn_id: Option<u64> },
    /// Client requests full context reset.
    Reset,
    /// Client heartbeat.
    Ping { ts: u64 },
}

impl ClientMessage {
    /// Parse a JSON string into a ClientMessage.
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hello_default_audio() {
        let msg = ClientMessage::parse(r#"{"type":"hello"}"#).unwrap();
        match msg {
            ClientMessage::Hello { audio } => {
                assert_eq!(audio.codec, "pcm16");
                assert_eq!(audio.sample_rate, 16_000);
            }
            _ => panic!("expected Hello"),
        }
    }

    #[test]
    fn parse_hello_custom_audio() {
        let json = r#"{"type":"hello","audio":{"codec":"opus","sample_rate":48000,"channels":1,"frame_ms":20}}"#;
        let msg = ClientMessage::parse(json).unwrap();
        match msg {
            ClientMessage::Hello { audio } => {
                assert_eq!(audio.codec, "opus");
                assert_eq!(audio.sample_rate, 48_000);
            }
            _ => panic!("expected Hello"),
        }
    }

    #[test]
    fn parse_text() {
        let msg = ClientMessage::parse(r#"{"type":"text","text":"hello world"}"#).unwrap();
        match msg {
            ClientMessage::Text { text } => assert_eq!(text, "hello world"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn parse_cancel_with_turn() {
        let msg = ClientMessage::parse(r#"{"type":"cancel","turn_id":42}"#).unwrap();
        match msg {
            ClientMessage::Cancel { turn_id } => assert_eq!(turn_id, Some(42)),
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn parse_cancel_no_turn() {
        let msg = ClientMessage::parse(r#"{"type":"cancel"}"#).unwrap();
        match msg {
            ClientMessage::Cancel { turn_id } => assert_eq!(turn_id, None),
            _ => panic!("expected Cancel"),
        }
    }

    #[test]
    fn parse_reset() {
        let msg = ClientMessage::parse(r#"{"type":"reset"}"#).unwrap();
        assert!(matches!(msg, ClientMessage::Reset));
    }

    #[test]
    fn parse_ping() {
        let msg = ClientMessage::parse(r#"{"type":"ping","ts":1700000000}"#).unwrap();
        match msg {
            ClientMessage::Ping { ts } => assert_eq!(ts, 1_700_000_000),
            _ => panic!("expected Ping"),
        }
    }

    #[test]
    fn parse_unknown_type_fails() {
        let result = ClientMessage::parse(r#"{"type":"unknown"}"#);
        assert!(result.is_err());
    }
}
