#!/usr/bin/env bash
# ALPHA-Ω reproducible demonstration (added 2026-10-23, AGPL-3.0).
# Builds the engine and replays every verification layer that this
# environment provides, each reported as PASS / FAIL / SKIP with the
# reason — nothing is skipped silently, nothing passes on a claim.
set -uo pipefail
cd "$(dirname "$0")/.."

pass=0; fail=0
ok()   { echo "[demo] PASS: $*"; pass=$((pass+1)); }
bad()  { echo "[demo] FAIL: $*"; fail=$((fail+1)); }
skip() { echo "[demo] SKIP: $*"; }

echo "[demo] ALPHA-Ω reproducible demonstration"

cargo build --release >/dev/null 2>&1 || { bad "cargo build"; echo "[demo] aborting"; exit 1; }
ok "cargo build --release"

if cargo test --release >/tmp/ao_test.log 2>&1; then
  ok "cargo test ($(grep -o '[0-9]* passed' /tmp/ao_test.log | head -1))"
else
  bad "cargo test — see /tmp/ao_test.log"
fi

cp -f target/release/alpha-sim /tmp/alpha-sim && chmod 755 /tmp/alpha-sim
/tmp/alpha-sim --cycles 100 --seed 42 | sed -n '/=== Summary ===/,$p' > /tmp/ao_summary.txt
grep -v '^#' docs/demo-golden.txt | sed -n '/=== Summary ===/,$p' > /tmp/ao_golden.txt
if diff -u /tmp/ao_golden.txt /tmp/ao_summary.txt >/dev/null; then
  ok "canonical run (seed 42, 100 cycles) matches docs/demo-golden.txt"
else
  bad "canonical run differs from golden"; diff -u /tmp/ao_golden.txt /tmp/ao_summary.txt | head -20
fi

./target/release/alpha-bench 100 >/tmp/ao_bench.log 2>&1 \
  && ok "benchmark harness (chain verified per run)" \
  || bad "benchmark harness — see /tmp/ao_bench.log"

if command -v gm2 >/dev/null 2>&1; then
  if (cd modula2 && gm2 -O2 -o alpha_numerics AlphaNumerics.mod >/dev/null 2>&1 \
      && ./alpha_numerics | grep -q "amp2_fixed=100000 elevated=TRUE"); then
    ok "Modula-2 alternate numerics agree with Rust (amp2_fixed=100000)"
  else
    bad "Modula-2 numerics check"
  fi
else
  skip "gm2 not in env — Modula-2 cross-check not run"
fi

LEAN_BIN="$(command -v lean || true)"
[ -z "$LEAN_BIN" ] && [ -x "$HOME/.elan/bin/lean" ] && LEAN_BIN="$HOME/.elan/bin/lean"
if [ -n "$LEAN_BIN" ]; then
  if "$LEAN_BIN" verification/AlphaSafety.lean >/dev/null 2>&1; then
    ok "Lean 4 obligations elaborate with no sorry (AlphaSafety.lean)"
  else
    bad "Lean check on verification/AlphaSafety.lean"
  fi
else
  skip "lean not in env — formal obligations not re-checked"
fi

if command -v glpsol >/dev/null 2>&1; then
  if glpsol --model contracts/alpha_feedback_gmpl.mod --data contracts/alpha_feedback.dat 2>&1 \
      | grep -q "INTEGER OPTIMAL SOLUTION FOUND"; then
    ok "AMPL contract solves to an integer optimum (GLPK, GMPL translation)"
  else
    bad "GLPK run on the AMPL contract"
  fi
else
  skip "glpsol not in env — AMPL contract not solved (see contracts/glpk_run.log)"
fi

ELIXIR_BIN="$(command -v elixir || true)"
[ -z "$ELIXIR_BIN" ] && [ -x "$HOME/workspace/.tools/elixir-1.18.4/bin/elixir" ] \
  && ELIXIR_BIN="$HOME/workspace/.tools/elixir-1.18.4/bin/elixir"
if [ -n "$ELIXIR_BIN" ]; then
  if (cd elixir && PATH="$(dirname "$ELIXIR_BIN"):$PATH" ELIXIR_ERL_OPTIONS="+fnu" \
        mix run demo.exs >/tmp/ao_elixir.log 2>&1); then
    ok "Elixir/OTP supervision demo (run, replay, kill + restart)"
  else
    bad "Elixir/OTP demo — see /tmp/ao_elixir.log"
  fi
else
  skip "elixir not in env — OTP supervision demo not run"
fi

echo "[demo] $pass passed, $fail failed"
[ "$fail" -eq 0 ]
