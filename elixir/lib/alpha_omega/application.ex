# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule AlphaOmega.Application do
  @moduledoc """
  OTP application entry point. Starts the ALPHA-Ω supervision tree:
  a bounded run log and the engine runner under a one-for-one
  supervisor with a bounded restart budget (no uncontrolled process
  spawning: at most 3 restarts in 10 seconds, then the supervisor
  itself gives up and the failure surfaces).
  """
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      AlphaOmega.RunLog,
      AlphaOmega.EngineRunner
    ]

    opts = [strategy: :one_for_one, max_restarts: 3, max_seconds: 10, name: AlphaOmega.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
