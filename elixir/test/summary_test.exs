# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule AlphaOmega.SummaryTest do
  use ExUnit.Case, async: true

  @transcript """
  === ALPHA-Ω Closed-Loop Simulation ===
  protocol_version=1  seed=42  cycles=100  rate=256Hz

  LEDGER: chain integrity OK (100 events)

  === Summary ===
  processed_windows : 100
  feedback_actions  : 3
  noop_actions      : 97
  rejections        : 0
  ledger_events     : 100
  final_clock       : 100
  final_head_hash   : 447a4b918ddffedabe1ec82d75f207c437b82a20ce19b3bdf38c4e5615c48055
  dropped_samples   : 0
  """

  test "parses a well-formed transcript exactly" do
    assert {:ok, s} = AlphaOmega.Summary.parse(@transcript)
    assert s.processed_windows == 100
    assert s.feedback_actions == 3
    assert s.noop_actions == 97
    assert s.rejections == 0
    assert s.ledger_events == 100
    assert s.final_clock == 100
    assert s.dropped_samples == 0

    assert s.final_head_hash ==
             "447a4b918ddffedabe1ec82d75f207c437b82a20ce19b3bdf38c4e5615c48055"
  end

  test "rejects a transcript without the integrity line" do
    text = String.replace(@transcript, "LEDGER: chain integrity OK", "LEDGER: unknown")
    assert {:error, :missing_integrity_line} = AlphaOmega.Summary.parse(text)
  end

  test "rejects a non-integer field instead of converting loosely" do
    text = String.replace(@transcript, "feedback_actions  : 3", "feedback_actions  : three")
    assert {:error, {:bad_integer, "feedback_actions", "three"}} = AlphaOmega.Summary.parse(text)
  end

  test "rejects a malformed head hash" do
    text = String.replace(@transcript, "447a4b918ddffedabe", "ZZZZ4b918ddffedabe")
    assert {:error, {:bad_hash, _}} = AlphaOmega.Summary.parse(text)
  end

  test "rejects a transcript with a missing field" do
    text = String.replace(@transcript, "rejections        : 0\n", "")
    assert {:error, {:missing_field, "rejections"}} = AlphaOmega.Summary.parse(text)
  end
end
