# Source layout

The executable core is pure Rust (`std` only).

Local development tree uses:

- `src/lib.rs` — module root
- `src/sha256.rs` — FIPS 180-4 SHA-256
- `src/core_impl.rs` — `include!` of domain / acquisition / policy / ledger bodies
- `src/*_body.rs` — split sources for size-friendly publishing

License: AGPL-3.0-or-later

```bash
cargo build --release
cp target/release/alpha-sim /tmp/alpha-sim && chmod 755 /tmp/alpha-sim
/tmp/alpha-sim --cycles 200 --seed 42
```
