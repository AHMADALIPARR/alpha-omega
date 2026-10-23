# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule AlphaOmega.EngineRunnerTest do
  use ExUnit.Case, async: false

  test "missing engine binary is an explicit error, never a fallback" do
    assert {:error, {:engine_not_found, "/nonexistent/alpha-sim"}} =
             AlphaOmega.EngineRunner.run(engine: "/nonexistent/alpha-sim")
  end

  test "bad arguments are rejected explicitly" do
    path = AlphaOmega.EngineRunner.engine_path()
    assert {:error, {:bad_cycles, 0}} = AlphaOmega.EngineRunner.run(engine: path, cycles: 0)
    assert {:error, {:bad_seed, -1}} = AlphaOmega.EngineRunner.run(engine: path, seed: -1)
  end

  test "runs the real engine and records the run" do
    path = AlphaOmega.EngineRunner.engine_path()

    if File.exists?(path) do
      before = AlphaOmega.RunLog.count()
      assert {:ok, s} = AlphaOmega.EngineRunner.run(engine: path, cycles: 100, seed: 42)
      assert s.feedback_actions == 3
      assert s.noop_actions == 97
      assert s.rejections == 0
      assert s.ledger_events == 100
      assert s.final_clock == 100
      assert AlphaOmega.RunLog.count() == before + 1
    else
      assert {:error, {:engine_not_found, ^path}} = AlphaOmega.EngineRunner.run(engine: path)
    end
  end
end
