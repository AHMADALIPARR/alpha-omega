//! ALPHA-Ω closed-loop simulation driver (pure std, no external crates).
//!
//! Copyright (C) 2025–2026 ALPHA-Ω contributors
//! SPDX-License-Identifier: AGPL-3.0-or-later
//!
//! This program is free software under the GNU Affero General Public License v3.0 or later.
//! See LICENSE for details.

use alpha_omega::*;
use alpha_omega::sha256::hex_encode;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut cycles: u64 = 200;
    let mut seed: u64 = 42;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-c" | "--cycles" => {
                i += 1;
                cycles = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(200);
            }
            "-s" | "--seed" => {
                i += 1;
                seed = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(42);
            }
            "-v" | "--verbose" => verbose = true,
            "-h" | "--help" => {
                println!("alpha-sim [--cycles N] [--seed N] [--verbose]");
                process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    println!("=== ALPHA-Ω Closed-Loop Simulation ===");
    println!(
        "protocol_version={}  seed={}  cycles={}  rate={}Hz",
        PROTOCOL_VERSION, seed, cycles, DEFAULT_RATE
    );
    println!();

    let cfg = AcqConfig {
        seed,
        sample_rate: DEFAULT_RATE,
        duration: cycles + 128,
        alpha_freq: 10.0,
        amplitude: 1.0,
        noise_std: 0.12,
        quality_base: 92,
        drop_permille: 2,
    };

    let mut acquirer = match SyntheticAcquirer::new(cfg) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("FATAL: acquisition init: {}", e);
            process::exit(1);
        }
    };

    let mut processor = SignalProcessor::new(DEFAULT_RATE);
    let mut state = LogicalState::initial(1);
    let mut ledger = WormLedger::new();

    let mut feedback_count = 0u64;
    let mut noop_count = 0u64;
    let mut reject_count = 0u64;
    let mut processed = 0u64;

    for _ in 0..WINDOW {
        if let Ok(obs) = acquirer.next() {
            let _ = processor.push(&obs);
        }
    }

    for cycle in 0..cycles {
        let obs = match acquirer.next() {
            Ok(o) => o,
            Err(_) => {
                println!("EOS at cycle {}", cycle);
                break;
            }
        };

        let proc_result = processor.push(&obs);
        let decision_obs = match proc_result {
            Ok(Some((power, quality, _band))) => {
                processed += 1;
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
                o
            }
            Ok(None) => continue,
            Err(_) => {
                let res = closed_loop_step(&state, &obs, obs.timestamp_ns, &mut ledger);
                state = res.next_state;
                noop_count += 1;
                if res.status != TransStatus::Accepted {
                    reject_count += 1;
                }
                continue;
            }
        };

        let res = closed_loop_step(&state, &decision_obs, decision_obs.timestamp_ns, &mut ledger);
        state = res.next_state;

        match res.executed {
            Action::VirtualFeedback => feedback_count += 1,
            Action::NoOp => noop_count += 1,
        }
        if res.status != TransStatus::Accepted {
            reject_count += 1;
        }

        if verbose {
            println!(
                "cycle {:4}  power={:.5}  q={:3}  action={:?}  status={:?}  cd={}  clock={}",
                cycle,
                decision_obs.alpha_power.to_f64(),
                decision_obs.quality,
                res.executed,
                res.status,
                state.feedback_cooldown,
                state.logical_clock
            );
        }
    }

    match ledger.verify_chain() {
        Ok(()) => println!("LEDGER: chain integrity OK ({} events)", ledger.len()),
        Err(id) => {
            eprintln!("LEDGER: INTEGRITY FAILURE at event {}", id);
            process::exit(2);
        }
    }

    println!();
    println!("=== Summary ===");
    println!("processed_windows : {}", processed);
    println!("feedback_actions  : {}", feedback_count);
    println!("noop_actions      : {}", noop_count);
    println!("rejections        : {}", reject_count);
    println!("ledger_events     : {}", ledger.len());
    println!("final_clock       : {}", state.logical_clock);
    println!("final_head_hash   : {}", hex_encode(&state.ledger_head));
    println!("dropped_samples   : {}", acquirer.dropped());
    println!();
    println!("ALPHA-Ω core simulation completed successfully.");
}
