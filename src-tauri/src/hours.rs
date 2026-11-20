//! Berechnung der Tagesarbeitszeit aus Start, Ende und Pause.

#[derive(Debug, PartialEq, Eq)]
pub enum HoursError {
    InvalidTime,
    EndNotAfterStart,
    BreakTooLong,
    InvalidBreak,
}

/// Arbeitszeit eines Tages in Minuten. `start` und `end` im Format `HH:MM`.
pub fn worked_minutes(start: &str, end: &str, break_minutes: f64) -> Result<u32, HoursError> {
    if !break_minutes.is_finite() || break_minutes < 0.0 || break_minutes.fract() != 0.0 {
        return Err(HoursError::InvalidBreak);
    }
    let start = parse_time(start)?;
    let end = parse_time(end)?;
    if end <= start {
        return Err(HoursError::EndNotAfterStart);
    }
    (end - start)
        .checked_sub(break_minutes as u32)
        .ok_or(HoursError::BreakTooLong)
}

fn parse_time(value: &str) -> Result<u32, HoursError> {
    let (hours, minutes) = value.split_once(':').ok_or(HoursError::InvalidTime)?;
    let is_two_digits = |s: &str| s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit());
    if !is_two_digits(hours) || !is_two_digits(minutes) {
        return Err(HoursError::InvalidTime);
    }
    let hours: u32 = hours.parse().map_err(|_| HoursError::InvalidTime)?;
    let minutes: u32 = minutes.parse().map_err(|_| HoursError::InvalidTime)?;
    if hours > 23 || minutes > 59 {
        return Err(HoursError::InvalidTime);
    }
    Ok(hours * 60 + minutes)
}

impl std::fmt::Display for HoursError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            HoursError::InvalidTime => "Bitte Uhrzeiten im Format HH:MM eingeben.",
            HoursError::EndNotAfterStart => "Das Ende muss nach dem Start liegen.",
            HoursError::BreakTooLong => "Die Pause ist länger als die Arbeitszeit.",
            HoursError::InvalidBreak => "Die Pause muss eine ganze Zahl von mindestens 0 Minuten sein.",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_day_subtracts_break() {
        assert_eq!(worked_minutes("08:00", "16:30", 30.0), Ok(480));
    }

    #[test]
    fn day_without_break() {
        assert_eq!(worked_minutes("09:00", "17:00", 0.0), Ok(480));
    }

    #[test]
    fn break_equal_to_block_gives_zero() {
        assert_eq!(worked_minutes("08:00", "09:00", 60.0), Ok(0));
    }

    #[test]
    fn end_before_start_is_rejected() {
        assert_eq!(worked_minutes("16:00", "08:00", 0.0), Err(HoursError::EndNotAfterStart));
    }

    #[test]
    fn end_equal_to_start_is_rejected() {
        assert_eq!(worked_minutes("08:00", "08:00", 0.0), Err(HoursError::EndNotAfterStart));
    }

    #[test]
    fn break_longer_than_block_is_rejected() {
        assert_eq!(worked_minutes("08:00", "09:00", 61.0), Err(HoursError::BreakTooLong));
    }

    #[test]
    fn malformed_time_is_rejected() {
        assert_eq!(worked_minutes("8h", "16:00", 0.0), Err(HoursError::InvalidTime));
        assert_eq!(worked_minutes("08:00", "25:00", 0.0), Err(HoursError::InvalidTime));
        assert_eq!(worked_minutes("08:60", "16:00", 0.0), Err(HoursError::InvalidTime));
    }

    #[test]
    fn non_canonical_time_is_rejected() {
        assert_eq!(worked_minutes("+8:00", "16:00", 0.0), Err(HoursError::InvalidTime));
        assert_eq!(worked_minutes("8:00", "16:00", 0.0), Err(HoursError::InvalidTime));
        assert_eq!(worked_minutes("08:5", "16:00", 0.0), Err(HoursError::InvalidTime));
        assert_eq!(worked_minutes("008:00", "16:00", 0.0), Err(HoursError::InvalidTime));
    }

    #[test]
    fn negative_or_fractional_break_is_rejected() {
        assert_eq!(worked_minutes("08:00", "16:00", -5.0), Err(HoursError::InvalidBreak));
        assert_eq!(worked_minutes("08:00", "16:00", 1.5), Err(HoursError::InvalidBreak));
        assert_eq!(worked_minutes("08:00", "16:00", f64::NAN), Err(HoursError::InvalidBreak));
    }
}
