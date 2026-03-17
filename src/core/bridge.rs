//! Bridge interface between the I/O layer (Tokio WS or Monoio WS) and
//! the Tokio business logic (Session + Workers).
//!
//! This module defines the message types that cross the runtime boundary.
//! In pure-Tokio mode, these are sent via `tokio::sync::mpsc`.
//! In Monoio+Tokio hybrid mode, these are sent via `crossbeam-channel`.

/// Raw WebSocket message from the I/O layer to the business logic.
/// The I/O layer does NOT parse JSON - it just forwards raw frames.
#[derive(Debug, Clone)]
pub enum RawWsMessage {
    /// UTF-8 text frame (JSON).
    Text(String),
    /// Binary frame (audio data).
    Binary(Vec<u8>),
    /// Connection closed by the client.
    Close(Option<String>),
}

/// Outbound message from the business logic to the I/O layer.
#[derive(Debug, Clone)]
pub enum OutMessage {
    /// UTF-8 text frame (serialized JSON).
    Text(String),
    /// Binary frame (audio data).
    Binary(Vec<u8>),
    /// Close the connection with a code and reason.
    Close(u16, String),
}
