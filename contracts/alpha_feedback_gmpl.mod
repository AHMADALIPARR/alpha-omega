# ALPHA-Ω AMPL mathematical contract — GMPL translation for GLPK.
# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Added 2026-10-23. This file is the SAME model as
# contracts/alpha_feedback.mod (which remains the canonical AMPL
# source), translated for the GNU MathProg (GMPL) translator, whose
# abs() accepts only constant arguments. The switching term
# w_s * |a[t] - a[t-1]| is linearized with an auxiliary variable
# d[t] and the two constraints SwitchUp / SwitchDown below. For
# w_s >= 0 the linearization is exact at any optimum: minimization
# forces d[t] down to |a[t] - a[t-1]|.
#
# AMPL is NOT a formal proof assistant. Export solver results for
# independent verification.

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
var d {0..T} >= 0;

minimize TotalCost:
    sum {t in 0..T}
        ( w_e * (P_alpha[t] - P_star)^2
        + w_a * a[t]
        + w_s * d[t] );

subject to SwitchUp {t in 0..T}:
    d[t] >= a[t] - (if t = 0 then a_prev else a[t-1]);

subject to SwitchDown {t in 0..T}:
    d[t] >= (if t = 0 then a_prev else a[t-1]) - a[t];

subject to QualityGate {t in 0..T}:
    a[t] <= if quality[t] >= quality_min then 1 else 0;

subject to FreshnessGate {t in 0..T}:
    a[t] <= if age[t] <= age_max then 1 else 0;

subject to PowerEligibility {t in 0..T}:
    a[t] <= if P_alpha[t] >= P_star * 0.5 then 1 else 0;

subject to FrequencyBound:
    sum {t in 0..T} a[t] <= max_feedback;

# Infeasibility must be reported by the solver; host must treat as NoOp.
