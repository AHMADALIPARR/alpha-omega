# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule AlphaOmega.EngineRunner do
  @moduledoc """
  Runs the Rust engine (`alpha-sim`) as a supervised external
  process over an Erlang Port and returns its parsed summary.

  Design rules honored here:
  * one run at a time per runner (calls are serialized by the
    GenServer), each with a hard timeout — a runaway engine process
    is killed, never left behind;
  * a nonzero exit, a timeout, or an unparseable transcript is an
    explicit {:error, reason}, never a silent fallback;
  * every successful run is written to AlphaOmega.RunLog.
  """
  use GenServer

  @default_timeout_ms 30_000
  @call_timeout_ms 60_000

  # --- client API ---

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, %{}, name: __MODULE__)
  end

  @doc """
  Run the engine. Options: :cycles (default 100), :seed (default
  42), :engine (path to alpha-sim; default: ALPHA_SIM_BIN env, else
  the repo's target/release/alpha-sim, else /tmp/alpha-sim),
  :timeout_ms (default #{@default_timeout_ms}).
  """
  def run(opts \\ []) do
    GenServer.call(__MODULE__, {:run, opts}, @call_timeout_ms)
  end

  @doc "Path the runner will execute for a default run."
  def engine_path(opts \\ []) do
    Keyword.get(opts, :engine) ||
      System.get_env("ALPHA_SIM_BIN") ||
      default_engine_path()
  end

  defp default_engine_path do
    repo_bin = Path.expand("../../../target/release/alpha-sim", __DIR__)
    if File.exists?(repo_bin), do: repo_bin, else: "/tmp/alpha-sim"
  end

  # --- server ---

  @impl true
  def init(state), do: {:ok, state}

  @impl true
  def handle_call({:run, opts}, _from, state) do
    {:reply, execute(opts), state}
  end

  defp execute(opts) do
    path = engine_path(opts)
    cycles = Keyword.get(opts, :cycles, 100)
    seed = Keyword.get(opts, :seed, 42)
    timeout = Keyword.get(opts, :timeout_ms, @default_timeout_ms)

    cond do
      not File.exists?(path) ->
        {:error, {:engine_not_found, path}}

      not (is_integer(cycles) and cycles > 0) ->
        {:error, {:bad_cycles, cycles}}

      not (is_integer(seed) and seed >= 0) ->
        {:error, {:bad_seed, seed}}

      true ->
        args = ["--cycles", Integer.to_string(cycles), "--seed", Integer.to_string(seed)]

        case run_port(path, args, timeout) do
          {:ok, text} ->
            case AlphaOmega.Summary.parse(text) do
              {:ok, summary} ->
                AlphaOmega.RunLog.record(summary)
                {:ok, summary}

              {:error, reason} ->
                {:error, {:unparseable_output, reason}}
            end

          {:error, reason} ->
            {:error, reason}
        end
    end
  end

  defp run_port(path, args, timeout) do
    port =
      Port.open({:spawn_executable, String.to_charlist(path)}, [
        :binary,
        :exit_status,
        {:args, args}
      ])

    collect(port, "", timeout)
  end

  defp collect(port, acc, timeout) do
    receive do
      {^port, {:data, chunk}} ->
        collect(port, acc <> chunk, timeout)

      {^port, {:exit_status, 0}} ->
        {:ok, acc}

      {^port, {:exit_status, status}} ->
        {:error, {:engine_exit, status, acc}}
    after
      timeout ->
        Port.close(port)
        {:error, :engine_timeout}
    end
  end
end
