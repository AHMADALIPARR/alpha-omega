# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule AlphaOmega.RunLog do
  @moduledoc """
  Bounded in-memory record of completed engine runs. The log holds
  at most #{64} entries; older entries are dropped in FIFO order.
  The bound is deliberate: no unbounded queues anywhere in the tree.
  """
  use Agent

  @max_entries 64

  def start_link(_opts) do
    Agent.start_link(fn -> [] end, name: __MODULE__)
  end

  @doc "Record one completed run summary (newest first)."
  def record(%AlphaOmega.Summary{} = summary) do
    Agent.update(__MODULE__, fn entries ->
      Enum.take([summary | entries], @max_entries)
    end)
  end

  @doc "All recorded summaries, newest first."
  def entries do
    Agent.get(__MODULE__, & &1)
  end

  @doc "Number of recorded runs."
  def count do
    Agent.get(__MODULE__, &length/1)
  end
end
