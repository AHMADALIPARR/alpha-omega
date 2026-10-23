//! ALPHA-Ω benchmark harness (pure std, no external crates).
//!
//! Copyright (C) 2025–2026 ALPHA-Ω contributors
//! SPDX-License-Identifier: AGPL-3.0-or-later
//!
//! Added 2026-10-23. Runs the full closed loop in-process at several
//! sizes and reports wall time, throughput, and the ledger head for
//! each. The head hashes are deterministic (seed 42), so two
//! machines can compare correctness even though timings differ.
//! Timings are measurements of the machine this runs on, nothing
//! more; they are printed next to the hashes so a slow run can never
//! masquerade as a correct one, or vice versa.
//!
//! Usage: alpha-bench [cycles ...]   (default: 100 1000 10000 100000)

use alpha_omega::sha256::hex_encode;
use alpha_omega::*;
use std::time::Instant;

fn run_once(cycles: u64) -> ([u8; 32], usize) {
    let cfg = AcqConfig {
        seed: 42,
        sample_rate: DEFAULT_RATE,
        duration: cycles + WINDOW as u64,
        alpha_freq: 10.0,
        amplitude: 1.0,
        noise_std: 0.12,
        quality_base: 92,
        drop_permille: 2,
    };
    let mut acquirer = SyntheticAcquirer::new(cfg).expect("acquirer init");
    let mut processor = SignalProcessor::new(DEFAULT_RATE);
    let mut state = LogicalState::initial(PROTOCOL_VERSION);
    let mut ledger = WormLedger::new();

    for _ in 0..WINDOW {
        if let Ok(obs) = acquirer.next() {
            let _ = processor.push(&obs);
        }
    }
    for _ in 0..cycles {
        let obs = acquirer.next().expect("stream length covers the run");
        if let Ok(Some((power, quality, _band))) = processor.push(&obs) {
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
    ledger.verify_chain().expect("ledger chain must verify");
    (state.ledger_head, ledger.len())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sizes: Vec<u64> = if args.is_empty() {
        vec![100, 1_000, 10_000, 100_000]
    } else {
        args.iter()
            .map(|a| a.parse().expect("cycle counts must be integers"))
            .collect()
    };

    println!("ALPHA-Ω benchmark (seed 42, in-process, chain verified per run)");
    println!(
        "{:>10}  {:>10}  {:>14}  {}",
        "cycles", "wall_ms", "events_per_sec", "head_hash"
    );
    for cycles in sizes {
        // One warm-up pass (also proves determinism), then the timed pass.
        let (head_warm, _) = run_once(cycles);
        let t0 = Instant::now();
        let (head, events) = run_once(cycles);
        let elapsed = t0.elapsed();
        assert_eq!(head, head_warm, "non-deterministic run at {cycles} cycles");
        let secs = elapsed.as_secs_f64();
        println!(
            "{:>10}  {:>10.2}  {:>14.0}  {}",
            cycles,
            secs * 1000.0,
            events as f64 / secs,
            hex_encode(&head)
        );
    }
}
