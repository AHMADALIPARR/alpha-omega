// ALPHA-Ω semantics, reference policy, verification gate, transition.
// Copyright (C) 2025–2026 ALPHA-Ω contributors
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// RECONSTRUCTED 2026-10-23: see the note in domain_body.rs. The
// definitions below mirror verification/AlphaSafety.lean clause for
// clause (encodeSemantics / referencePolicy there, encode_semantics /
// reference_policy here) and the rules of semantics/predicates.dl.

/// Derive the typed semantic facts for one observation in context.
/// An invalid observation poisons every derived fact (fail-closed):
/// no fact that could authorize feedback may survive `valid = false`.
pub fn encode_semantics(obs: &Observation, state: &LogicalState, now_ns: u64) -> SemanticFacts {
    let valid = obs.valid;
    let age_ns = now_ns.saturating_sub(obs.timestamp_ns);

    let elevated = valid && obs.alpha_power >= POWER_THRESHOLD;
    let quality_ok = valid && obs.quality >= MIN_QUALITY;
    let fresh = valid && age_ns <= MAX_AGE_NS;
    let band = valid && obs.band_detected;
    // Lean `eligible`: valid, quality, freshness, cooldown elapsed,
    // and the lifetime feedback bound not yet reached; the Datalog
    // contract additionally requires AlphaBandDetected.
    let eligible = valid
        && band
        && quality_ok
        && fresh
        && state.feedback_cooldown == 0
        && state.action_count < MAX_FEEDBACK_WINDOW;
    let permitted = eligible && elevated;

    // Provenance: digest of (sample_index, timestamp, power).
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&obs.sample_index.to_le_bytes());
    buf[8..16].copy_from_slice(&obs.timestamp_ns.to_le_bytes());
    buf[16..24].copy_from_slice(&obs.alpha_power.0.to_le_bytes());
    let provenance_digest = sha256(&buf);

    SemanticFacts {
        observation_valid: valid,
        alpha_band_detected: band,
        alpha_power_elevated: elevated,
        signal_quality_acceptable: quality_ok,
        timestamp_fresh: fresh,
        feedback_eligible: eligible,
        feedback_permitted: permitted,
        provenance_digest,
    }
}

/// Reference policy pi(S): feedback is proposed only when the facts
/// say eligible AND permitted and the state's own bounds agree.
/// Mirrors Lean `referencePolicy` exactly.
pub fn reference_policy(state: &LogicalState, facts: &SemanticFacts) -> Action {
    if !facts.feedback_eligible || !facts.feedback_permitted {
        return Action::NoOp;
    }
    if state.feedback_cooldown > 0 || state.action_count >= MAX_FEEDBACK_WINDOW {
        return Action::NoOp;
    }
    Action::VirtualFeedback
}

/// Outcome of the verification gate for one proposed transition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VerifyOutcome {
    Verified,
    Rejected,
}

/// Fail-closed verification gate. The gate re-derives every condition
/// a feedback action depends on, independently of the policy, so a
/// policy defect cannot authorize feedback on its own (Lean
/// obligation 4, `verified_only`). NoOp is always verifiable.
pub fn verify_transition(
    state: &LogicalState,
    obs: &Observation,
    facts: &SemanticFacts,
    proposed: Action,
) -> VerifyOutcome {
    match proposed {
        Action::NoOp => VerifyOutcome::Verified,
        Action::VirtualFeedback => {
            let ok = obs.valid
                && obs.checksum_valid()
                && facts.observation_valid
                && facts.feedback_eligible
                && facts.feedback_permitted
                && facts.alpha_power_elevated
                && state.feedback_cooldown == 0
                && state.action_count < MAX_FEEDBACK_WINDOW;
            if ok {
                VerifyOutcome::Verified
            } else {
                VerifyOutcome::Rejected
            }
        }
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    fn obs(valid: bool, power: i64, quality: u8, band: bool, age_ns: u64) -> Observation {
        let mut o = Observation {
            timestamp_ns: 1_000_000_000,
            sample_index: 7,
            sample: 0.0,
            alpha_power: Fixed(power),
            quality,
            valid,
            band_detected: band,
            checksum: 0,
        };
        o.checksum = Observation::compute_checksum(
            o.timestamp_ns,
            o.sample_index,
            o.alpha_power,
            o.quality,
            o.valid,
        );
        // encode_semantics reads age as now - timestamp; emulate an
        // observation aged age_ns by passing a later "now".
        let _ = age_ns;
        o
    }

    /// Conformance with semantics/predicates.dl. The Datalog rules
    /// are transcribed literally:
    ///   FeedbackEligible(c)  :- ObservationValid(c), AlphaBandDetected(c),
    ///                           SignalQualityAcceptable(c), TimestampFresh(c)
    ///   FeedbackPermitted(c) :- FeedbackEligible(c), AlphaPowerElevated(c)
    ///   TransitionRejected(c):- FeedbackPermitted(c), !ObservationValid(c)
    /// The Rust encode_semantics additionally folds the state bounds
    /// (cooldown elapsed, lifetime bound not reached) into
    /// eligibility, exactly as verification/AlphaSafety.lean defines
    /// it; with a clear state the two must agree on every input, and
    /// with a blocked state Rust eligibility must be false while the
    /// state-free Datalog core stays as transcribed. The rejection
    /// rule's body is unsatisfiable by construction (permitted
    /// implies valid): assert that invariant over the whole grid.
    #[test]
    fn datalog_conformance() {
        for &valid in &[true, false] {
            for &power in &[0i64, 49_999, 50_000, 90_000] {
                for &quality in &[0u8, 9, 10, 95] {
                    for &band in &[true, false] {
                        for &age in &[0u64, MAX_AGE_NS, MAX_AGE_NS + 1] {
                            for &(cooldown, count) in &[(0u64, 0u64), (3, 0), (0, 3)] {
                                let o = obs(valid, power, quality, band, age);
                                let mut st = LogicalState::initial(1);
                                st.feedback_cooldown = cooldown;
                                st.action_count = count;
                                let now = o.timestamp_ns + age;
                                let f = encode_semantics(&o, &st, now);

                                // Datalog side, from raw inputs only.
                                let dl_valid = valid;
                                let dl_band = valid && band;
                                let dl_quality = valid && quality >= MIN_QUALITY;
                                let dl_fresh = valid && age <= MAX_AGE_NS;
                                let dl_elevated = valid && Fixed(power) >= POWER_THRESHOLD;
                                let dl_eligible =
                                    dl_valid && dl_band && dl_quality && dl_fresh;
                                let dl_permitted = dl_eligible && dl_elevated;

                                // Rust side = Datalog core AND state bounds.
                                let state_clear =
                                    cooldown == 0 && count < MAX_FEEDBACK_WINDOW;
                                assert_eq!(f.observation_valid, dl_valid);
                                assert_eq!(f.alpha_band_detected, dl_band);
                                assert_eq!(f.signal_quality_acceptable, dl_quality);
                                assert_eq!(f.timestamp_fresh, dl_fresh);
                                assert_eq!(f.alpha_power_elevated, dl_elevated);
                                assert_eq!(f.feedback_eligible, dl_eligible && state_clear);
                                assert_eq!(
                                    f.feedback_permitted,
                                    dl_permitted && state_clear
                                );
                                // The .dl rejection rule body
                                // (permitted AND NOT valid) never fires.
                                assert!(!(f.feedback_permitted && !f.observation_valid));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Deterministic transition F(S, O, A): advance the logical clock by
/// exactly one, mirror the observation's decision fields, and apply
/// the executed action's bookkeeping (feedback consumes one unit of
/// the lifetime bound and starts the cooldown; every other step ticks
/// an active cooldown down by one).
pub fn apply_transition(state: &LogicalState, executed: Action, obs: &Observation) -> LogicalState {
    let mut next = state.clone();
    next.logical_clock += 1;
    next.alpha_power = obs.alpha_power;
    next.quality = obs.quality;
    next.last_action = executed;
    match executed {
        Action::VirtualFeedback => {
            next.action_count += 1;
            next.feedback_cooldown = COOLDOWN_TICKS;
        }
        Action::NoOp => {
            if next.feedback_cooldown > 0 {
                next.feedback_cooldown -= 1;
            }
        }
    }
    next
}
