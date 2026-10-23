#!/usr/bin/env bash
# Reconstruct src/lib.rs from base64 fragments (AGPL-3.0)
set -euo pipefail
cd "$(dirname "$0")/.."
cat src/lib.rs.b64.0 src/lib.rs.b64.1 src/lib.rs.b64.2 src/lib.rs.b64.3 | base64 -d > src/lib.rs
echo "Restored src/lib.rs ($(wc -c < src/lib.rs) bytes)"
echo "Build with: cargo build --release"
