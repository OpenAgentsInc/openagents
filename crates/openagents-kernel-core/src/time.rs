pub fn floor_to_minute_utc(value_ms: i64) -> i64 {
    value_ms.div_euclid(60_000) * 60_000
}

pub fn snapshot_id_for_minute(as_of_ms: i64) -> String {
    format!("snapshot.economy:{as_of_ms}")
}
