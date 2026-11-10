/// Environment variable that carries a dotted settings key: `db.port` -> `APP_DB_PORT`.
pub fn var_name(key: &str) -> String {
    format!("APP_{}", key.to_uppercase())
}
