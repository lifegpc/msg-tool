//! Functions for parsing numbers within a range.
/// Check if a value is within the specified range.
pub fn check_range<T>(val: T, min: T, max: T) -> Result<T, String>
where
    T: PartialOrd,
    T: std::fmt::Display,
{
    if val < min {
        return Err(format!("Value {} is less than minimum {}", val, min));
    } else if val > max {
        return Err(format!("Value {} is greater than maximum {}", val, max));
    }
    Ok(val)
}

/// Parse a number from a string and check if it is within the specified range.
pub fn number_range<T>(s: &str, min: T, max: T) -> Result<T, String>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
    T: PartialOrd,
    T: std::fmt::Display,
{
    debug_assert!(min <= max, "min should be less than or equal to max");
    let val = s.parse::<T>().map_err(|e| format!("{}", e))?;
    check_range(val, min, max)
}
