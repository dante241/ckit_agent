OPS-2291 · billing-worker · P1 · reopened (2nd time)

> prod pod has APP_DB_PORT=5433 in its env (we moved the db), worker still connects to 5432
> config.toml on the image says db.port = 5432 — that file is the baked-in fallback, env is supposed to win
> runbook (wiki, "Config precedence"): command-line flag > environment > config file > built-in default
> last "fix" was someone changing config.toml by hand on one box, please don't do that again

Settings code is in this crate. Make resolution follow the runbook. Keep the public API, `cargo test` green.
