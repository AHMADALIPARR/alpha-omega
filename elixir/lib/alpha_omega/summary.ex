# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule AlphaOmega.Summary do
  @moduledoc """
  Explicit parser for the Rust engine's summary block.

  The Port boundary is text. Numbers are converted only here, only
  with `Integer.parse/1`, and a conversion counts only when the whole
  token is consumed — no implicit numerical conversions across the
  FFI boundary. The head hash is carried as a string and validated
  as 64 lowercase hex characters; it is never parsed as a number.
  """

  defstruct processed_windows: 0,
            feedback_actions: 0,
            noop_actions: 0,
            rejections: 0,
            ledger_events: 0,
            final_clock: 0,
            final_head_hash: "",
            dropped_samples: 0

  @int_fields [
    {"processed_windows", :processed_windows},
    {"feedback_actions", :feedback_actions},
    {"noop_actions", :noop_actions},
    {"rejections", :rejections},
    {"ledger_events", :ledger_events},
    {"final_clock", :final_clock},
    {"dropped_samples", :dropped_samples}
  ]

  @doc """
  Parse a full engine stdout transcript. Returns {:ok, summary} only
  when the transcript carries the chain-integrity line and every
  summary field; {:error, reason} otherwise.
  """
  def parse(text) when is_binary(text) do
    if String.contains?(text, "LEDGER: chain integrity OK") do
      fields =
        text
        |> String.split("\n")
        |> Enum.reduce(%{}, fn line, acc ->
          case Regex.run(~r/^([a-z_]+)\s+:\s+(\S+)\s*$/, String.trim(line)) do
            [_, key, value] -> Map.put(acc, key, value)
            _ -> acc
          end
        end)

      with {:ok, ints} <- parse_ints(fields, @int_fields),
           {:ok, hash} <- parse_hash(fields) do
        {:ok, struct(__MODULE__, Map.put(ints, :final_head_hash, hash))}
      end
    else
      {:error, :missing_integrity_line}
    end
  end

  defp parse_ints(fields, spec) do
    Enum.reduce_while(spec, {:ok, %{}}, fn {key, atom}, {:ok, acc} ->
      case Map.fetch(fields, key) do
        :error ->
          {:halt, {:error, {:missing_field, key}}}

        {:ok, token} ->
          case Integer.parse(token) do
            {n, ""} -> {:cont, {:ok, Map.put(acc, atom, n)}}
            _ -> {:halt, {:error, {:bad_integer, key, token}}}
          end
      end
    end)
  end

  defp parse_hash(fields) do
    case Map.fetch(fields, "final_head_hash") do
      :error ->
        {:error, {:missing_field, "final_head_hash"}}

      {:ok, token} ->
        if Regex.match?(~r/^[0-9a-f]{64}$/, token) do
          {:ok, token}
        else
          {:error, {:bad_hash, token}}
        end
    end
  end
end
