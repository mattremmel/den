//! Date filter parsing for CLI commands.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

/// A date filter that matches timestamps on or after a threshold.
#[derive(Debug, Clone)]
pub struct DateFilter {
    threshold: DateTime<Utc>,
}

impl DateFilter {
    /// Parses a date filter from a string.
    ///
    /// Accepts:
    /// - Relative: "7d", "30d" (days ago from now)
    /// - Absolute: "2024-01-15" (YYYY-MM-DD format)
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();

        // Try relative format: Nd (e.g., "7d", "30d")
        if let Some(days_str) = s.strip_suffix('d') {
            let days: i64 = days_str
                .parse()
                .map_err(|_| format!("invalid relative date: {}", s))?;
            if days < 0 {
                return Err(format!("days must be non-negative: {}", s));
            }
            let threshold = Utc::now() - chrono::Duration::days(days);
            return Ok(Self { threshold });
        }

        // Try absolute format: YYYY-MM-DD
        let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| format!("invalid date format (expected YYYY-MM-DD or Nd): {}", s))?;

        let threshold = Utc
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .ok_or_else(|| format!("invalid date: {}", s))?;

        Ok(Self { threshold })
    }

    /// Returns true if the given timestamp matches this filter (is on or after threshold).
    pub fn matches(&self, timestamp: DateTime<Utc>) -> bool {
        timestamp >= self.threshold
    }

    /// Returns the threshold datetime.
    pub fn threshold(&self) -> DateTime<Utc> {
        self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_relative_days() {
        let filter = DateFilter::parse("7d").unwrap();
        let now = Utc::now();
        let seven_days_ago = now - chrono::Duration::days(7);

        // Threshold should be approximately 7 days ago
        let diff = (filter.threshold() - seven_days_ago).num_seconds().abs();
        assert!(diff < 2, "threshold should be ~7 days ago");
    }

    #[test]
    fn parse_absolute_date() {
        let filter = DateFilter::parse("2024-01-15").unwrap();
        let expected = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
        assert_eq!(filter.threshold(), expected);
    }

    #[test]
    fn parse_invalid_format() {
        assert!(DateFilter::parse("invalid").is_err());
        assert!(DateFilter::parse("2024/01/15").is_err());
        assert!(DateFilter::parse("-5d").is_err());
    }

    #[test]
    fn matches_after_threshold() {
        let filter = DateFilter::parse("2024-01-15").unwrap();
        let after = Utc.with_ymd_and_hms(2024, 1, 20, 12, 0, 0).unwrap();
        assert!(filter.matches(after));
    }

    #[test]
    fn matches_on_threshold() {
        let filter = DateFilter::parse("2024-01-15").unwrap();
        let on = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
        assert!(filter.matches(on));
    }

    #[test]
    fn does_not_match_before_threshold() {
        let filter = DateFilter::parse("2024-01-15").unwrap();
        let before = Utc.with_ymd_and_hms(2024, 1, 14, 23, 59, 59).unwrap();
        assert!(!filter.matches(before));
    }
}
