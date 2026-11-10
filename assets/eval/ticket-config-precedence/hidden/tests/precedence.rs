use std::collections::HashMap;

use appcfg::settings::{get, Sources};

fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn env_beats_config_file() {
    let (flags, env, file) = (map(&[]), map(&[("APP_DB_PORT", "5433")]), map(&[("db.port", "5432")]));
    let src = Sources { flags: &flags, env: &env, file: &file };
    assert_eq!(get(&src, "db.port").as_deref(), Some("5433"));
}

#[test]
fn flag_beats_env_and_file() {
    let (flags, env, file) = (map(&[("db.port", "6000")]), map(&[("APP_DB_PORT", "5433")]), map(&[("db.port", "5432")]));
    let src = Sources { flags: &flags, env: &env, file: &file };
    assert_eq!(get(&src, "db.port").as_deref(), Some("6000"));
}

#[test]
fn file_beats_default_and_default_is_last() {
    let (none, file) = (map(&[]), map(&[("db.host", "db.internal")]));
    let src = Sources { flags: &none, env: &none, file: &file };
    assert_eq!(get(&src, "db.host").as_deref(), Some("db.internal"));
    assert_eq!(get(&src, "db.port").as_deref(), Some("5432"));
}
