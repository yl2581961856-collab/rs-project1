//! Latency measurement infrastructure for paper experiments.
//!
//! Every event flowing through the pipeline is timestamped at key points.
//! Metrics are emitted as structured JSON log lines that can be parsed
//! offline by the analysis scripts in `bench/analyze.py`.

use std::time::Instant;

use super::events::TurnId;

/// High-resolution timestamps attached to pipeline stages.
/// All durations are relative to `session_start`.
#[derive(Debug, Clone)]
pub struct TurnMetrics {
    pub turn: TurnId,
    /// Instant when the turn was allocated.
    pub turn_start: Instant,
    /// Instant when ASR received the first audio frame for this turn.
    pub asr_first_audio: Option<Instant>,
    /// Instant when ASR emitted the final transcript.
    pub asr_final: Option<Instant>,
    /// Instant when LLM produced the first token.
    pub llm_first_token: Option<Instant>,
    /// Instant when LLM finished generating.
    pub llm_done: Option<Instant>,
    /// Instant when TTS produced the first audio frame.
    pub tts_first_audio: Option<Instant>,
    /// Instant when TTS finished generating all audio.
    pub tts_done: Option<Instant>,
    /// Instant when cancel was requested (if any).
    pub cancel_requested: Option<Instant>,
    /// Instant when all workers acknowledged the cancel.
    pub cancel_propagated: Option<Instant>,
}

impl TurnMetrics {
    pub fn new(turn: TurnId) -> Self {
        Self {
            turn,
            turn_start: Instant::now(),
            asr_first_audio: None,
            asr_final: None,
            llm_first_token: None,
            llm_done: None,
            tts_first_audio: None,
            tts_done: None,
            cancel_requested: None,
            cancel_propagated: None,
        }
    }

    /// Time to First Token: ASR final -> LLM first token.
    pub fn ttft(&self) -> Option<std::time::Duration> {
        match (self.asr_final, self.llm_first_token) {
            (Some(a), Some(b)) => Some(b.duration_since(a)),
            _ => None,
        }
    }

    /// Time to First Audio: ASR final -> TTS first audio frame.
    pub fn ttfa(&self) -> Option<std::time::Duration> {
        match (self.asr_final, self.tts_first_audio) {
            (Some(a), Some(b)) => Some(b.duration_since(a)),
            _ => None,
        }
    }

    /// Cancel Propagation Delay: cancel requested -> cancel propagated.
    pub fn cpd(&self) -> Option<std::time::Duration> {
        match (self.cancel_requested, self.cancel_propagated) {
            (Some(a), Some(b)) => Some(b.duration_since(a)),
            _ => None,
        }
    }

    /// End-to-end latency: turn start -> TTS first audio frame.
    pub fn e2e_to_first_audio(&self) -> Option<std::time::Duration> {
        self.tts_first_audio
            .map(|t| t.duration_since(self.turn_start))
    }

    /// Emit metrics as a tracing event (structured JSON line).
    pub fn emit(&self) {
        let ttft_us = self.ttft().map(|d| d.as_micros() as u64);
        let ttfa_us = self.ttfa().map(|d| d.as_micros() as u64);
        let cpd_us = self.cpd().map(|d| d.as_micros() as u64);
        let e2e_us = self.e2e_to_first_audio().map(|d| d.as_micros() as u64);

        tracing::info!(
            turn_id = self.turn.0,
            ttft_us = ttft_us,
            ttfa_us = ttfa_us,
            cpd_us = cpd_us,
            e2e_first_audio_us = e2e_us,
            "turn_metrics"
        );
    }
}

/// Session-level metrics aggregator.
#[derive(Debug)]
pub struct SessionMetrics {
    pub session_start: Instant,
    pub turns_completed: u64,
    pub turns_cancelled: u64,
    /// Stale data leaked after cancel (bytes of audio sent to client
    /// from a superseded turn).
    pub stale_leakage_bytes: u64,
}

impl SessionMetrics {
    pub fn new() -> Self {
        Self {
            session_start: Instant::now(),
            turns_completed: 0,
            turns_cancelled: 0,
            stale_leakage_bytes: 0,
        }
    }
}

impl Default for SessionMetrics {
    fn default() -> Self {
        Self::new()
    }
}
