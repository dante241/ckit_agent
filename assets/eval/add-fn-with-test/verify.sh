#!/bin/sh
# $1 = omp stdout capture (unused). The assertions live in hidden/tests/slugify.rs,
# copied in only after omp exits, so the agent cannot tune its code to them.
cargo test --test slugify >/dev/null 2>&1
