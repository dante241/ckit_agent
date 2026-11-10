#!/bin/sh
# $1 = omp stdout capture (unused). hidden/ has restored tests/add.rs (plus cases
# the agent never saw) and Cargo.toml, so deleting or special-casing the test fails.
cargo test --test add >/dev/null 2>&1
