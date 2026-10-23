# ALPHA-Ω Makefile
# Copyright (C) 2025–2026 ALPHA-Ω contributors
# SPDX-License-Identifier: AGPL-3.0-or-later

.PHONY: build test sim clean

build:
	cargo build --release

test:
	cargo test --release

# Overlay FS may be noexec; copy binary to /tmp
sim: build
	cp -f target/release/alpha-sim /tmp/alpha-sim
	chmod 755 /tmp/alpha-sim
	/tmp/alpha-sim --cycles 200 --seed 42

sim-verbose: build
	cp -f target/release/alpha-sim /tmp/alpha-sim
	chmod 755 /tmp/alpha-sim
	/tmp/alpha-sim --cycles 80 --seed 42 -v

clean:
	cargo clean
	rm -f /tmp/alpha-sim
