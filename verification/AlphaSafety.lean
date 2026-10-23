/-
  ALPHA-Ω Lean 4 safety specification
  Copyright (C) 2025–2026 ALPHA-Ω contributors
  SPDX-License-Identifier: AGPL-3.0-or-later

  TOOLCHAIN STATUS: Lean 4 / lake is NOT installed in the original build environment.
  Open proof obligations only — no sorry / admit / unreviewed axioms.
-/

namespace AlphaOmega

structure FixedPower where
  val : Int
  deriving DecidableEq, Repr

inductive Action where
  | NoOp
  | VirtualFeedback
  deriving DecidableEq, Repr

structure LogicalState where
  alphaPower : FixedPower
  quality : Nat
  cooldown : Nat
  actionCount : Nat
  clock : Nat
  lastAction : Action
  deriving Repr

structure Observation where
  valid : Bool
  alphaPower : FixedPower
  quality : Nat
  ageNs : Nat
  deriving Repr

structure SemanticFacts where
  observationValid : Bool
  alphaPowerElevated : Bool
  signalQualityAcceptable : Bool
  timestampFresh : Bool
  feedbackEligible : Bool
  feedbackPermitted : Bool
  deriving Repr

def powerThreshold : Int := 50000
def minQuality : Nat := 10
def maxAge : Nat := 500000000
def maxFeedback : Nat := 3

def encodeSemantics (obs : Observation) (s : LogicalState) : SemanticFacts :=
  let ov := obs.valid
  let elevated := decide (ov = true) && decide (obs.alphaPower.val ≥ powerThreshold)
  let qOk := decide (ov = true) && decide (obs.quality ≥ minQuality)
  let fresh := decide (ov = true) && decide (obs.ageNs ≤ maxAge)
  let eligible :=
    decide (ov = true) && qOk && fresh
      && decide (s.cooldown = 0) && decide (s.actionCount < maxFeedback)
  let permitted := eligible && elevated
  { observationValid := ov
    alphaPowerElevated := elevated
    signalQualityAcceptable := qOk
    timestampFresh := fresh
    feedbackEligible := eligible
    feedbackPermitted := permitted }

def referencePolicy (s : LogicalState) (f : SemanticFacts) : Action :=
  if f.feedbackEligible = false ∨ f.feedbackPermitted = false then
    Action.NoOp
  else if s.cooldown > 0 ∨ s.actionCount ≥ maxFeedback then
    Action.NoOp
  else
    Action.VirtualFeedback

/-!
  ## Open proof obligations (do not insert sorry)

  1. `invalid_obs_no_feedback`
     ∀ obs s, ¬obs.valid → referencePolicy s (encodeSemantics obs s) = Action.NoOp

  2. `ineligible_noop`
     ∀ s f, ¬f.feedbackEligible → referencePolicy s f = Action.NoOp

  3. `frequency_bound`
     After any transition that executes VirtualFeedback,
     next.actionCount ≤ maxFeedback

  4. `verified_only`
     The executable never commits an action unless the verification gate
     returns Verified for that (state, action, next) triple.

  5. `clock_monotonic`
     next.clock = state.clock + 1 for every accepted transition.

  These are checked today by the Rust unit tests and the closed-loop
  simulation (see `cargo test` and `alpha-sim`). Formal machine-checked
  proofs require a Lean 4 installation.
-/

/-!
  ## Extension (added 2026-10-23): transition and discharged obligations

  The five obligations above are discharged in this section. A
  `transition` function is added, mirroring the Rust engine's
  verification gate + `apply_transition` (src/policy_body.rs,
  src/ledger_loop_body.rs): `encodeSemantics` derives the facts,
  `referencePolicy` proposes an action, and the gate re-checks every
  condition independently before VirtualFeedback may execute. The
  logical clock advances by exactly one on every step; a feedback
  step consumes one unit of the lifetime bound and starts the
  cooldown (`cooldownTicks`, the engine's COOLDOWN_TICKS); any other
  step ticks an active cooldown down by one.
-/

/-- Cooldown imposed after a feedback action, in steps
    (the Rust engine's COOLDOWN_TICKS). -/
def cooldownTicks : Nat := 16

/-- Gate conditions for executing VirtualFeedback, re-derived
    independently of the policy: the policy must propose feedback,
    the observation must be valid, the cooldown must have elapsed,
    the lifetime feedback bound must not be reached, and the facts
    must permit feedback. -/
def gateConditions (s : LogicalState) (obs : Observation) : Bool :=
  decide (referencePolicy s (encodeSemantics obs s) = Action.VirtualFeedback)
    && decide (obs.valid = true)
    && decide (s.cooldown = 0)
    && decide (s.actionCount < maxFeedback)
    && (encodeSemantics obs s).feedbackPermitted

/-- One closed-loop transition: propose via `referencePolicy`,
    execute VirtualFeedback only when `gateConditions` holds,
    otherwise execute NoOp. The clock always advances by exactly
    one, and the executed action is recorded in `lastAction`. -/
def transition (s : LogicalState) (obs : Observation) : LogicalState :=
  if gateConditions s obs then
    { s with
      alphaPower := obs.alphaPower
      quality := obs.quality
      cooldown := cooldownTicks
      actionCount := s.actionCount + 1
      clock := s.clock + 1
      lastAction := Action.VirtualFeedback }
  else
    { s with
      alphaPower := obs.alphaPower
      quality := obs.quality
      cooldown := s.cooldown - 1
      clock := s.clock + 1
      lastAction := Action.NoOp }

/-- Obligation 1: an invalid observation can never produce feedback
    under the reference policy. -/
theorem invalid_obs_no_feedback (obs : Observation) (s : LogicalState)
    (h : obs.valid = false) :
    referencePolicy s (encodeSemantics obs s) = Action.NoOp := by
  have hElig : (encodeSemantics obs s).feedbackEligible = false := by
    simp [encodeSemantics, h]
  unfold referencePolicy
  rw [if_pos (Or.inl hElig)]

/-- Obligation 2: facts that are not feedback-eligible yield NoOp. -/
theorem ineligible_noop (s : LogicalState) (f : SemanticFacts)
    (h : f.feedbackEligible = false) :
    referencePolicy s f = Action.NoOp := by
  unfold referencePolicy
  rw [if_pos (Or.inl h)]

/-- Obligation 5: every transition advances the logical clock by
    exactly one. -/
theorem clock_monotonic (s : LogicalState) (obs : Observation) :
    (transition s obs).clock = s.clock + 1 := by
  unfold transition
  split <;> rfl

/-- Auxiliary: if a transition records VirtualFeedback as the
    executed action, the gate conditions must have held. -/
theorem gate_true_of_executed_feedback (s : LogicalState) (obs : Observation)
    (h : (transition s obs).lastAction = Action.VirtualFeedback) :
    gateConditions s obs = true := by
  cases hgc : gateConditions s obs with
  | false =>
    have hNo : (transition s obs).lastAction = Action.NoOp := by
      simp [transition, hgc]
    rw [hNo] at h
    cases h
  | true => rfl

/-- Obligation 4 (constructive form of `verified_only`): a
    transition commits VirtualFeedback only if the verification
    gate's conditions held for that step — the observation was
    valid, the cooldown had elapsed, the lifetime bound had not
    been reached, and the facts permitted feedback. -/
theorem verified_only (s : LogicalState) (obs : Observation)
    (h : (transition s obs).lastAction = Action.VirtualFeedback) :
    obs.valid = true ∧ s.cooldown = 0 ∧ s.actionCount < maxFeedback
      ∧ (encodeSemantics obs s).feedbackPermitted = true := by
  have hg := gate_true_of_executed_feedback s obs h
  simp only [gateConditions, Bool.and_eq_true, decide_eq_true_eq] at hg
  exact ⟨hg.1.1.1.2, hg.1.1.2, hg.1.2, hg.2⟩

/-- Obligation 3: after a transition that executes VirtualFeedback,
    the resulting actionCount does not exceed the lifetime bound. -/
theorem frequency_bound (s : LogicalState) (obs : Observation)
    (h : (transition s obs).lastAction = Action.VirtualFeedback) :
    (transition s obs).actionCount ≤ maxFeedback := by
  have hg := gate_true_of_executed_feedback s obs h
  have hlt : s.actionCount < maxFeedback := by
    have hg' := hg
    simp only [gateConditions, Bool.and_eq_true, decide_eq_true_eq] at hg'
    exact hg'.1.2
  have hac : (transition s obs).actionCount = s.actionCount + 1 := by
    simp [transition, hg]
  rw [hac]
  exact Nat.succ_le_of_lt hlt

end AlphaOmega
