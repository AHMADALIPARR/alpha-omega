// ALPHA-Ω WORM ledger and the closed-loop step.
// Copyright (C) 2025–2026 ALPHA-Ω contributors
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// RECONSTRUCTED 2026-10-23: see the note in domain_body.rs. Every
// step — accepted or rejected — appends exactly one hash-chained
// audit event, so the ledger is a complete decision record.

/// One append-only audit event. `hash` chains the event to its
/// predecessor: sha256(prev_hash || digest).
#[derive(Clone, Debug)]
pub struct LedgerEvent {
    pub id: u64,
    pub prev_hash: [u8; 32],
    pub digest: [u8; 32],
    pub hash: [u8; 32],
}

/// Write-once-read-many ledger: an append-only SHA-256 hash chain.
pub struct WormLedger {
    events: Vec<LedgerEvent>,
    genesis: [u8; 32],
}

impl WormLedger {
    pub fn new() -> WormLedger {
        WormLedger {
            events: Vec::new(),
            genesis: sha256(b"ALPHA-OMEGA/WORM/GENESIS"),
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Hash of the most recent event, or the genesis hash when empty.
    pub fn head(&self) -> [u8; 32] {
        self.events.last().map(|e| e.hash).unwrap_or(self.genesis)
    }

    /// Append one event carrying `digest`; returns the event hash.
    pub fn append(&mut self, digest: [u8; 32]) -> [u8; 32] {
        let prev_hash = self.head();
        let mut buf = [0u8; 64];
        buf[0..32].copy_from_slice(&prev_hash);
        buf[32..64].copy_from_slice(&digest);
        let hash = sha256(&buf);
        let id = self.events.len() as u64;
        self.events.push(LedgerEvent {
            id,
            prev_hash,
            digest,
            hash,
        });
        hash
    }

    /// Recompute the whole chain. `Err(id)` names the first event
    /// whose linkage or hash does not check out.
    pub fn verify_chain(&self) -> Result<(), u64> {
        let mut prev = self.genesis;
        for e in &self.events {
            if e.prev_hash != prev {
                return Err(e.id);
            }
            let mut buf = [0u8; 64];
            buf[0..32].copy_from_slice(&e.prev_hash);
            buf[32..64].copy_from_slice(&e.digest);
            if sha256(&buf) != e.hash {
                return Err(e.id);
            }
            prev = e.hash;
        }
        Ok(())
    }
}

impl Default for WormLedger {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of one closed-loop step.
pub struct StepResult {
    pub next_state: LogicalState,
    pub executed: Action,
    pub status: TransStatus,
}

/// Canonical digest of one decision, as committed to the ledger:
/// binds the pre-step head, clock, observation coordinates, facts
/// provenance, executed action and status.
fn step_digest(
    state: &LogicalState,
    obs: &Observation,
    facts: &SemanticFacts,
    executed: Action,
    status: TransStatus,
) -> [u8; 32] {
    let mut buf = Vec::with_capacity(32 + 8 + 8 + 8 + 32 + 2);
    buf.extend_from_slice(&state.ledger_head);
    buf.extend_from_slice(&state.logical_clock.to_le_bytes());
    buf.extend_from_slice(&obs.sample_index.to_le_bytes());
    buf.extend_from_slice(&obs.timestamp_ns.to_le_bytes());
    buf.extend_from_slice(&facts.provenance_digest);
    buf.push(match executed {
        Action::NoOp => 0,
        Action::VirtualFeedback => 1,
    });
    buf.push(match status {
        TransStatus::Accepted => 0,
        TransStatus::RejectedInvalidObservation => 1,
        TransStatus::RejectedChecksum => 2,
        TransStatus::RejectedGate => 3,
    });
    sha256(&buf)
}

/// One closed-loop step: validate, derive facts, propose, verify,
/// apply, audit. Rejection at any stage executes NoOp, records the
/// reason in the status, and still commits an audit event.
pub fn closed_loop_step(
    state: &LogicalState,
    obs: &Observation,
    now_ns: u64,
    ledger: &mut WormLedger,
) -> StepResult {
    let facts = encode_semantics(obs, state, now_ns);

    if !obs.checksum_valid() {
        return commit(
            state,
            obs,
            &facts,
            Action::NoOp,
            TransStatus::RejectedChecksum,
            ledger,
        );
    }
    if !obs.valid {
        return commit(
            state,
            obs,
            &facts,
            Action::NoOp,
            TransStatus::RejectedInvalidObservation,
            ledger,
        );
    }

    let proposed = reference_policy(state, &facts);
    let (executed, status) = match verify_transition(state, obs, &facts, proposed) {
        VerifyOutcome::Verified => (proposed, TransStatus::Accepted),
        VerifyOutcome::Rejected => (Action::NoOp, TransStatus::RejectedGate),
    };
    commit(state, obs, &facts, executed, status, ledger)
}

/// Apply the transition and append its audit event; the new event
/// hash becomes the next state's ledger head.
fn commit(
    state: &LogicalState,
    obs: &Observation,
    facts: &SemanticFacts,
    executed: Action,
    status: TransStatus,
    ledger: &mut WormLedger,
) -> StepResult {
    let digest = step_digest(state, obs, facts, executed, status);
    let mut next_state = apply_transition(state, executed, obs);
    next_state.ledger_head = ledger.append(digest);
    StepResult {
        next_state,
        executed,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid, elevated, in-band observation with a correct checksum.
    fn elevated_obs(idx: u64) -> Observation {
        let mut obs = Observation {
            timestamp_ns: idx * 3_906_250,
            sample_index: idx,
            sample: 0.0,
            alpha_power: Fixed::from_f64(0.9),
            quality: 95,
            valid: true,
            band_detected: true,
            checksum: 0,
        };
        obs.checksum = Observation::compute_checksum(
            obs.timestamp_ns,
            obs.sample_index,
            obs.alpha_power,
            obs.quality,
            obs.valid,
        );
        obs
    }

    // Lean obligation 1: invalid_obs_no_feedback.
    #[test]
    fn invalid_obs_no_feedback() {
        let state = LogicalState::initial(1);
        let mut ledger = WormLedger::new();
        let mut obs = elevated_obs(0);
        obs.valid = false;
        obs.checksum = Observation::compute_checksum(
            obs.timestamp_ns,
            obs.sample_index,
            obs.alpha_power,
            obs.quality,
            obs.valid,
        );
        let res = closed_loop_step(&state, &obs, obs.timestamp_ns, &mut ledger);
        assert_eq!(res.executed, Action::NoOp);
        assert_eq!(res.status, TransStatus::RejectedInvalidObservation);
        assert_eq!(ledger.len(), 1);
    }

    // Lean obligation 2: ineligible_noop (quality below the gate, and
    // separately, power below the elevation threshold).
    #[test]
    fn ineligible_noop() {
        let state = LogicalState::initial(1);
        let mut ledger = WormLedger::new();

        let mut low_q = elevated_obs(0);
        low_q.quality = MIN_QUALITY - 1;
        low_q.checksum = Observation::compute_checksum(
            low_q.timestamp_ns,
            low_q.sample_index,
            low_q.alpha_power,
            low_q.quality,
            low_q.valid,
        );
        let res = closed_loop_step(&state, &low_q, low_q.timestamp_ns, &mut ledger);
        assert_eq!(res.executed, Action::NoOp);
        assert_eq!(res.status, TransStatus::Accepted);

        let mut low_p = elevated_obs(1);
        low_p.alpha_power = Fixed(POWER_THRESHOLD.0 - 1);
        low_p.checksum = Observation::compute_checksum(
            low_p.timestamp_ns,
            low_p.sample_index,
            low_p.alpha_power,
            low_p.quality,
            low_p.valid,
        );
        let res = closed_loop_step(&state, &low_p, low_p.timestamp_ns, &mut ledger);
        assert_eq!(res.executed, Action::NoOp);
        assert_eq!(res.status, TransStatus::Accepted);
    }

    // Lean obligation 3: frequency_bound — over a long run of elevated
    // observations, feedback fires exactly MAX_FEEDBACK_WINDOW times,
    // and the cooldown suppresses the step right after each firing.
    #[test]
    fn frequency_bound_and_cooldown() {
        let mut state = LogicalState::initial(1);
        let mut ledger = WormLedger::new();
        let mut feedbacks = 0u64;
        let mut prev_was_feedback = false;
        for idx in 0..200u64 {
            let obs = elevated_obs(idx);
            let res = closed_loop_step(&state, &obs, obs.timestamp_ns, &mut ledger);
            if prev_was_feedback {
                assert_eq!(
                    res.executed,
                    Action::NoOp,
                    "cooldown must suppress the step after a feedback"
                );
            }
            if res.executed == Action::VirtualFeedback {
                feedbacks += 1;
            }
            prev_was_feedback = res.executed == Action::VirtualFeedback;
            state = res.next_state;
        }
        assert_eq!(feedbacks, MAX_FEEDBACK_WINDOW);
        assert_eq!(state.action_count, MAX_FEEDBACK_WINDOW);
        assert_eq!(ledger.len(), 200);
        assert!(ledger.verify_chain().is_ok());
    }

    // Lean obligation 5: clock_monotonic — every step advances the
    // logical clock by exactly one.
    #[test]
    fn clock_monotonic() {
        let mut state = LogicalState::initial(1);
        let mut ledger = WormLedger::new();
        for idx in 0..10u64 {
            let obs = elevated_obs(idx);
            let before = state.logical_clock;
            let res = closed_loop_step(&state, &obs, obs.timestamp_ns, &mut ledger);
            assert_eq!(res.next_state.logical_clock, before + 1);
            state = res.next_state;
        }
        assert_eq!(state.logical_clock, 10);
    }

    // Fail-closed gate: a tampered observation (checksum mismatch)
    // executes NoOp, is reported as a checksum rejection, and is
    // still written to the audit ledger.
    #[test]
    fn checksum_tamper_rejected() {
        let state = LogicalState::initial(1);
        let mut ledger = WormLedger::new();
        let mut obs = elevated_obs(0);
        obs.quality = 30; // tamper after the checksum was computed
        let res = closed_loop_step(&state, &obs, obs.timestamp_ns, &mut ledger);
        assert_eq!(res.executed, Action::NoOp);
        assert_eq!(res.status, TransStatus::RejectedChecksum);
        assert_eq!(ledger.len(), 1);
        assert!(ledger.verify_chain().is_ok());
    }

    // Deterministic replay: the full driver pipeline (acquirer,
    // processor, loop) run twice from one seed yields identical
    // ledger heads, and the chain verifies.
    #[test]
    fn deterministic_replay_same_seed_same_head() {
        fn run() -> ([u8; 32], usize) {
            let cfg = AcqConfig {
                seed: 42,
                sample_rate: DEFAULT_RATE,
                duration: 400,
                alpha_freq: 10.0,
                amplitude: 1.0,
                noise_std: 0.12,
                quality_base: 92,
                drop_permille: 2,
            };
            let mut acq = SyntheticAcquirer::new(cfg).expect("acquirer init");
            let mut proc = SignalProcessor::new(DEFAULT_RATE);
            let mut state = LogicalState::initial(1);
            let mut ledger = WormLedger::new();
            for _ in 0..WINDOW {
                if let Ok(obs) = acq.next() {
                    let _ = proc.push(&obs);
                }
            }
            for _ in 0..100 {
                let obs = acq.next().expect("stream");
                if let Ok(Some((power, quality, _band))) = proc.push(&obs) {
                    let mut o = obs.clone();
                    o.alpha_power = power;
                    o.quality = quality;
                    o.checksum = Observation::compute_checksum(
                        o.timestamp_ns,
                        o.sample_index,
                        o.alpha_power,
                        o.quality,
                        o.valid,
                    );
                    let res = closed_loop_step(&state, &o, o.timestamp_ns, &mut ledger);
                    state = res.next_state;
                }
            }
            assert!(ledger.verify_chain().is_ok());
            (state.ledger_head, ledger.len())
        }
        let (head_a, len_a) = run();
        let (head_b, len_b) = run();
        assert_eq!(head_a, head_b);
        assert_eq!(len_a, len_b);
        assert_eq!(len_a, 100);
    }
}
