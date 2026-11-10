#!/bin/sh
# $1 = omp stdout capture (unused). Hidden acceptance test + the crate's own tests.
cargo test --test pagination >/dev/null 2>&1 && cargo test --lib >/dev/null 2>&1
