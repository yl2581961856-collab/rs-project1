//! WebSocket handler for the Tokio/Axum mode.
//!
//! Handles WebSocket upgrade, splits the connection into reader/writer,
//! parses JSON protocol messages, and bridges to the Session.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use bytes::Bytes;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::core::bridge::OutMessage;
use crate::core::events::{AudioCodec, AudioConfig, Event, SessionId, TurnId};
use crate::core::session::Session;
use crate::protocol::client_msg::{AudioConfigMsg, ClientMessage};
use crate::protocol::ServerMessage;

/// Channel buffer size for events and outbound messages.
const CHANNEL_BUFFER: usize = 256;

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let session_id = SessionId(Uuid::new_v4());
    tracing::info!(?session_id, "client connected");

    // Split WebSocket into reader and writer.
    let (ws_writer, mut ws_reader) = socket.split();

    // Event channel: WS reader -> Session.
    let (ev_tx, ev_rx) = mpsc::channel::<Event>(CHANNEL_BUFFER);
    // Outbound channel: Session -> WS writer.
    let (out_tx, out_rx) = mpsc::channel::<OutMessage>(CHANNEL_BUFFER);

    // Spawn the outbound writer task.
    let writer_task = tokio::spawn(ws_writer_loop(ws_writer, out_rx));

    // Send session-ready message.
    let session_msg = ServerMessage::Session {
        session_id: session_id.0.to_string(),
    };
    let _ = out_tx.send(OutMessage::Text(session_msg.to_json())).await;

    // Spawn the session state machine.
    let session = Session::new(session_id, ev_rx, out_tx.clone());
    let session_task = tokio::spawn(session.run());

    let _ = ev_tx.send(Event::ClientConnected).await;

    // Read loop: receive WS messages and forward as Events.
    use futures_util::StreamExt;
    while let Some(result) = ws_reader.next().await {
        match result {
            Ok(msg) => {
                let event = match msg {
                    Message::Binary(bin) => Some(Event::ClientAudioFrame {
                        pcm16: Bytes::from(bin),
                        sample_rate: 16_000,
                    }),
                    Message::Text(text) => parse_text_message(&text),
                    Message::Close(_) => {
                        let _ = ev_tx.send(Event::ClientDisconnected).await;
                        break;
                    }
                    Message::Ping(_) | Message::Pong(_) => None,
                };

                if let Some(ev) = event {
                    if ev_tx.send(ev).await.is_err() {
                        break;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(?session_id, error = %e, "ws read error");
                let _ = ev_tx.send(Event::ClientDisconnected).await;
                break;
            }
        }
    }

    // Clean shutdown: drop ev_tx so session sees channel close.
    drop(ev_tx);
    let _ = session_task.await;
    // Drop out_tx so writer task sees channel close.
    drop(out_tx);
    let _ = writer_task.await;

    tracing::info!(?session_id, "client fully disconnected");
}

/// Parse a JSON text message into an Event.
fn parse_text_message(text: &str) -> Option<Event> {
    match ClientMessage::parse(text) {
        Ok(msg) => match msg {
            ClientMessage::Hello { audio } => Some(Event::ClientHello {
                audio: audio_config_from_msg(&audio),
            }),
            ClientMessage::Text { text } => Some(Event::ClientText(text)),
            ClientMessage::Cancel { turn_id } => Some(Event::ClientCancel {
                turn: turn_id.map(TurnId),
            }),
            ClientMessage::Reset => Some(Event::ClientReset),
            ClientMessage::Ping { .. } => {
                // Ping is handled at protocol level, not forwarded to state machine.
                // The pong response is sent by the WS layer or we could handle it here.
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, raw = text, "failed to parse client message");
            None
        }
    }
}

/// Convert protocol AudioConfigMsg to internal AudioConfig.
fn audio_config_from_msg(msg: &AudioConfigMsg) -> AudioConfig {
    let codec = match msg.codec.as_str() {
        "opus" => AudioCodec::Opus,
        _ => AudioCodec::Pcm16,
    };
    AudioConfig {
        codec,
        sample_rate: msg.sample_rate,
        channels: msg.channels,
        frame_ms: msg.frame_ms,
    }
}

/// Writer loop: receives OutMessages and sends them over the WebSocket.
async fn ws_writer_loop(
    mut writer: futures_util::stream::SplitSink<WebSocket, Message>,
    mut out_rx: mpsc::Receiver<OutMessage>,
) {
    use futures_util::SinkExt;

    while let Some(msg) = out_rx.recv().await {
        let ws_msg = match msg {
            OutMessage::Text(json) => Message::Text(json),
            OutMessage::Binary(data) => Message::Binary(data),
            OutMessage::Close(code, reason) => {
                let close_frame = axum::extract::ws::CloseFrame {
                    code,
                    reason: reason.into(),
                };
                Message::Close(Some(close_frame))
            }
        };

        if writer.send(ws_msg).await.is_err() {
            tracing::debug!("ws writer: connection closed");
            break;
        }
    }
}
