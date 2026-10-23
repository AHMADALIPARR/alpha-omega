//! ALPHA-Ω core implementation
//! Copyright (C) 2025–2026 ALPHA-Ω contributors
//! SPDX-License-Identifier: AGPL-3.0-or-later

use crate::sha256::sha256;

include!("domain_body.rs");
include!("acq_sig_body.rs");
include!("policy_body.rs");
include!("ledger_loop_body.rs");
