/// The one way this project renders a timestamp for storage.
///
/// Fixed-width milliseconds and a `Z` suffix, so lexicographic order
/// always agrees with chronological order — which the audit log's index
/// `idx_audit_logs_created_at` depends on. Plain `to_rfc3339()` gives
/// neither: its subsecond precision varies and it writes `+00:00`.
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn timestamps_are_fixed_width_utc() {
        let now = now_iso();
        // 1970-01-01T00:00:00.000Z
        assert_eq!(now.len(), 24, "{now}");
        assert!(now.ends_with('Z'), "{now}");
        assert_eq!(&now[10..11], "T", "{now}");
        assert_eq!(&now[19..20], ".", "{now}");
        // Round-trips, so it is still a valid RFC 3339 instant.
        chrono::DateTime::parse_from_rfc3339(&now).unwrap();
    }

    #[test]
    fn lexicographic_order_matches_chronological_order() {
        // The property the audit log's index relies on. `to_rfc3339()`
        // breaks it: a whole-second instant renders shorter than a
        // sub-second one, so string comparison can invert.
        let earlier = chrono::Utc
            .timestamp_opt(1_000_000_000, 0)
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let later = chrono::Utc
            .timestamp_opt(1_000_000_000, 1_000_000)
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert!(earlier < later, "{earlier} !< {later}");
        assert_eq!(earlier.len(), later.len());
    }
}
