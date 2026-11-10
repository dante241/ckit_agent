use std::collections::HashMap;

use crate::{defaults, env};

/// Where a value can come from. `flags` and `file` are keyed by the dotted key
/// (`db.port`); `env` is the process environment (`APP_DB_PORT`).
pub struct Sources<'a> {
    pub flags: &'a HashMap<String, String>,
    pub env: &'a HashMap<String, String>,
    pub file: &'a HashMap<String, String>,
}

pub fn get(src: &Sources, key: &str) -> Option<String> {
    if let Some(v) = src.flags.get(key) {
        return Some(v.clone());
    }
    if let Some(v) = src.file.get(key) {
        return Some(v.clone());
    }
    if let Some(v) = src.env.get(&env::var_name(key)) {
        return Some(v.clone());
    }
    defaults::default_for(key).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_default() {
        let none = HashMap::new();
        let src = Sources { flags: &none, env: &none, file: &none };
        assert_eq!(get(&src, "worker.threads").as_deref(), Some("4"));
    }
}
