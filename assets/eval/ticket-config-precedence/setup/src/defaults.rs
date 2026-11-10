/// Built-in value for a dotted settings key, if it has one.
pub fn default_for(key: &str) -> Option<&'static str> {
    match key {
        "db.port" => Some("5432"),
        "db.host" => Some("localhost"),
        "worker.threads" => Some("4"),
        _ => None,
    }
}
