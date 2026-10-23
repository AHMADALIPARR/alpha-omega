// ALPHA-Ω domain types and protocol constants.
// Copyright (C) 2025–2026 ALPHA-Ω contributors
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// RECONSTRUCTED 2026-10-23: the original *_body.rs sources were never
// committed to this repository (src/core_impl.rs include!s them and
// src/lib.rs.b64.0 is a placeholder). This file is rebuilt from the
// in-repo contracts: src/main.rs (driver API), verification/
// AlphaSafety.lean (semantics), semantics/predicates.dl (fact names),
// contracts/alpha_feedback.mod (bounds), README.md (invariants).

use std::fmt;

/// Wire / ledger protocol version.
pub const PROTOCOL_VERSION: u32 = 1;
/// Default sample rate of the synthetic acquisition stream (Hz).
pub const DEFAULT_RATE: u32 = 256;
/// Sliding spectral window length, in samples.
pub const WINDOW: usize = 128;
/// Maximum number of feedback actions the engine may ever execute.
pub const MAX_FEEDBACK_WINDOW: u64 = 3;
/// Alpha power at or above this fixed-point value counts as elevated.
/// Matches `powerThreshold` in verification/AlphaSafety.lean.
pub const POWER_THRESHOLD: Fixed = Fixed(50_000);
/// Minimum acceptable signal quality (0..=100). Matches Lean `minQuality`.
pub const MIN_QUALITY: u8 = 10;
/// Maximum observation age for freshness, in nanoseconds (Lean `maxAge`).
pub const MAX_AGE_NS: u64 = 500_000_000;
/// Cooldown, in committed steps, imposed after each feedback action.
pub const COOLDOWN_TICKS: u64 = 16;
/// Alpha band lower edge (Hz).
pub const ALPHA_BAND_MIN_HZ: f64 = 8.0;
/// Alpha band upper edge (Hz).
pub const ALPHA_BAND_MAX_HZ: f64 = 13.0;
/// Spectral target of the signal processor: band centre (Hz).
pub const ALPHA_TARGET_HZ: f64 = 10.0;

/// Fixed-point power value, scaled by 100_000 (Lean `FixedPower`).
/// 50_000 therefore represents a normalized power of 0.5.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Fixed(pub i64);

impl Fixed {
    pub const SCALE: i64 = 100_000;
    pub const ZERO: Fixed = Fixed(0);

    pub fn from_f64(x: f64) -> Fixed {
        Fixed((x * Self::SCALE as f64).round() as i64)
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }
}

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.5}", self.to_f64())
    }
}

/// Executable actions. Physical stimulation does not exist in this enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    NoOp,
    VirtualFeedback,
}

/// Outcome classification of one closed-loop step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransStatus {
    Accepted,
    RejectedInvalidObservation,
    RejectedChecksum,
    RejectedGate,
}

/// One acquired sample with its processing annotations.
///
/// The acquisition layer fills `sample`, `valid`, `band_detected` and a
/// provisional `quality`; the signal processor's windowed estimate
/// overwrites `alpha_power` and `quality` before the decision step,
/// after which `checksum` is recomputed over the decision fields.
#[derive(Clone, Debug)]
pub struct Observation {
    pub timestamp_ns: u64,
    pub sample_index: u64,
    pub sample: f64,
    pub alpha_power: Fixed,
    pub quality: u8,
    pub valid: bool,
    pub band_detected: bool,
    pub checksum: u64,
}

impl Observation {
    /// Integrity checksum over the decision-relevant fields:
    /// SHA-256 of the canonical little-endian encoding, truncated to
    /// the first 8 digest bytes.
    pub fn compute_checksum(
        timestamp_ns: u64,
        sample_index: u64,
        alpha_power: Fixed,
        quality: u8,
        valid: bool,
    ) -> u64 {
        let mut buf = [0u8; 26];
        buf[0..8].copy_from_slice(&timestamp_ns.to_le_bytes());
        buf[8..16].copy_from_slice(&sample_index.to_le_bytes());
        buf[16..24].copy_from_slice(&alpha_power.0.to_le_bytes());
        buf[24] = quality;
        buf[25] = valid as u8;
        let h = sha256(&buf);
        u64::from_le_bytes(h[0..8].try_into().expect("sha256 digest is 32 bytes"))
    }

    pub fn checksum_valid(&self) -> bool {
        self.checksum
            == Self::compute_checksum(
                self.timestamp_ns,
                self.sample_index,
                self.alpha_power,
                self.quality,
                self.valid,
            )
    }
}

/// Logical engine state carried across steps (Lean `LogicalState`,
/// extended with the ledger head and protocol version).
#[derive(Clone, Debug)]
pub struct LogicalState {
    pub protocol_version: u32,
    pub alpha_power: Fixed,
    pub quality: u8,
    pub feedback_cooldown: u64,
    pub action_count: u64,
    pub logical_clock: u64,
    pub last_action: Action,
    pub ledger_head: [u8; 32],
}

impl LogicalState {
    pub fn initial(protocol_version: u32) -> LogicalState {
        LogicalState {
            protocol_version,
            alpha_power: Fixed::ZERO,
            quality: 0,
            feedback_cooldown: 0,
            action_count: 0,
            logical_clock: 0,
            last_action: Action::NoOp,
            ledger_head: sha256(b"ALPHA-OMEGA/WORM/GENESIS"),
        }
    }
}

/// Typed semantic facts derived from one observation in context
/// (semantics/predicates.dl, verification/AlphaSafety.lean).
/// `provenance_digest` binds the facts to (sample_index, timestamp,
/// power) as required by the Datalog contract's provenance note.
#[derive(Clone, Copy, Debug, Default)]
pub struct SemanticFacts {
    pub observation_valid: bool,
    pub alpha_band_detected: bool,
    pub alpha_power_elevated: bool,
    pub signal_quality_acceptable: bool,
    pub timestamp_fresh: bool,
    pub feedback_eligible: bool,
    pub feedback_permitted: bool,
    pub provenance_digest: [u8; 32],
}

/// Synthetic acquisition configuration (see src/main.rs for the
/// driver's canonical values).
#[derive(Clone, Debug)]
pub struct AcqConfig {
    pub seed: u64,
    pub sample_rate: u32,
    pub duration: u64,
    pub alpha_freq: f64,
    pub amplitude: f64,
    pub noise_std: f64,
    pub quality_base: u8,
    pub drop_permille: u16,
}

/// Engine error type: acquisition init, stream gaps, end of stream.
#[derive(Clone, Debug)]
pub struct AlphaError(pub String);

impl fmt::Display for AlphaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AlphaError {}
