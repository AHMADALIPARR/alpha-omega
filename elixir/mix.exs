# ALPHA-Ω Elixir/OTP supervision layer.
# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Added 2026-10-23: the OTP supervision tree the README lists as a
# subsystem. Zero external dependencies (no hex packages): the tree
# supervises runs of the Rust engine binary over a Port.
defmodule AlphaOmega.MixProject do
  use Mix.Project

  def project do
    [
      app: :alpha_omega_sup,
      version: "0.1.0",
      elixir: "~> 1.18",
      start_permanent: Mix.env() == :prod,
      deps: []
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {AlphaOmega.Application, []}
    ]
  end
end
