# ALPHA-Ω AMPL mathematical contract
# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# AMPL is NOT a formal proof assistant. Export solver results for independent verification.

# Mixed-integer model for virtual feedback eligibility and cost.
# AMPL is NOT a formal proof assistant. Export solver results for independent check.

param T > 0;
param P_star;
param w_e >= 0;
param w_a >= 0;
param w_s >= 0;
param max_feedback integer >= 0;
param quality_min;
param age_max;

param P_alpha {0..T};
param quality {0..T};
param age {0..T};
param a_prev binary;

var a {0..T} binary;

minimize TotalCost:
    sum {t in 0..T}
        ( w_e * (P_alpha[t] - P_star)^2
        + w_a * a[t]
        + w_s * abs(a[t] - (if t = 0 then a_prev else a[t-1])) );

subject to QualityGate {t in 0..T}:
    a[t] <= if quality[t] >= quality_min then 1 else 0;

subject to FreshnessGate {t in 0..T}:
    a[t] <= if age[t] <= age_max then 1 else 0;

subject to PowerEligibility {t in 0..T}:
    a[t] <= if P_alpha[t] >= P_star * 0.5 then 1 else 0;

subject to FrequencyBound:
    sum {t in 0..T} a[t] <= max_feedback;

# Infeasibility must be reported by the solver; host must treat as NoOp.
