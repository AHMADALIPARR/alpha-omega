# ALPHA-Ω OTP supervision demonstration.
# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Run from this directory (the Rust engine must be built first):
#   mix run demo.exs
#
# Shows: (1) a supervised engine run with the canonical seed-42
# summary, (2) deterministic replay under supervision (identical
# head hash), (3) the runner being killed mid-flight and the
# supervisor restarting it, after which runs succeed again.

defmodule Demo do
  def check(label, true), do: IO.puts("  PASS  #{label}")
  def check(label, false), do: raise("DEMO FAILED: #{label}")

  def wait_for_new_runner(old_pid, attempts \\ 50)
  def wait_for_new_runner(_old_pid, 0), do: raise("DEMO FAILED: runner was not restarted")

  def wait_for_new_runner(old_pid, attempts) do
    case Process.whereis(AlphaOmega.EngineRunner) do
      pid when is_pid(pid) and pid != old_pid ->
        pid

      _ ->
        Process.sleep(100)
        wait_for_new_runner(old_pid, attempts - 1)
    end
  end

  def main do
    IO.puts("== ALPHA-Ω OTP supervision demo ==")
    IO.puts("engine: #{AlphaOmega.EngineRunner.engine_path()}")

    IO.puts("[1] supervised run, seed 42, 100 cycles")
    {:ok, s1} = AlphaOmega.EngineRunner.run(cycles: 100, seed: 42)
    check("feedback_actions == 3", s1.feedback_actions == 3)
    check("noop_actions == 97", s1.noop_actions == 97)
    check("rejections == 0", s1.rejections == 0)
    check("ledger_events == 100", s1.ledger_events == 100)
    check("final_clock == 100", s1.final_clock == 100)
    IO.puts("  head: #{s1.final_head_hash}")

    IO.puts("[2] deterministic replay under supervision")
    {:ok, s2} = AlphaOmega.EngineRunner.run(cycles: 100, seed: 42)
    check("identical head hash", s2.final_head_hash == s1.final_head_hash)

    IO.puts("[3] kill the runner; the supervisor restarts it")
    old_pid = Process.whereis(AlphaOmega.EngineRunner)
    Process.exit(old_pid, :kill)
    new_pid = wait_for_new_runner(old_pid)
    check("runner restarted with a new pid", new_pid != old_pid)
    {:ok, s3} = AlphaOmega.EngineRunner.run(cycles: 100, seed: 42)
    check("post-restart run matches", s3.final_head_hash == s1.final_head_hash)

    check("run log holds all three runs", AlphaOmega.RunLog.count() >= 3)
    IO.puts("== demo complete: supervision, determinism, recovery verified ==")
  end
end

Demo.main()
