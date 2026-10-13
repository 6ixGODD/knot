//! Exact values; no binary floating point is used for integers or decimals.
use crate::KnotError;
pub const SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Validate a bounded JSON number token as a mathematical safe integer.
pub fn exact_integer(token: &str) -> Result<i64, KnotError> {
    if token.len() > 128 {
        return Err(KnotError::limit("numberTokenBytes", 128, token.len()));
    }
    let invalid = || KnotError::new("BAD_SCHEMA", "");
    token.parse::<serde_json::Number>().map_err(|_| invalid())?;
    let (mantissa, exponent) = match token.find(['e', 'E']) {
        Some(i) => (
            &token[..i],
            token[i + 1..].parse::<i32>().map_err(|_| invalid())?,
        ),
        None => (token, 0),
    };
    if exponent.unsigned_abs() > 308 {
        return Err(KnotError::limit(
            "exponentMagnitude",
            308,
            exponent.unsigned_abs() as usize,
        ));
    }
    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.strip_prefix('-').unwrap_or(mantissa);
    let mut digits = String::new();
    let mut fractional = 0i32;
    let mut dot = false;
    for c in mantissa.chars() {
        if c == '.' && !dot {
            dot = true;
            continue;
        }
        if !c.is_ascii_digit() {
            return Err(invalid());
        }
        digits.push(c);
        if dot {
            fractional += 1;
        }
    }
    if digits.is_empty() {
        return Err(invalid());
    }
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return Ok(0);
    }
    let mut value = significant.to_owned();
    let power = exponent - fractional;
    if power < 0 {
        let count = (-power) as usize;
        if count >= value.len() || !value[value.len() - count..].bytes().all(|b| b == b'0') {
            return Err(invalid());
        }
        value.truncate(value.len() - count);
    } else {
        if value.len() + power as usize > 16 {
            return Err(invalid());
        }
        value.extend(std::iter::repeat_n('0', power as usize));
    }
    let value: i64 = value.parse().map_err(|_| invalid())?;
    if value > SAFE_INTEGER {
        return Err(invalid());
    }
    Ok(if negative { -value } else { value })
}

/// Canonical checked coefficient/scale pair, maximum scale 9 and safe magnitude.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decimal {
    coefficient: i128,
    scale: u32,
}

impl Decimal {
    pub fn parse(input: &str) -> Result<Self, KnotError> {
        let invalid = || KnotError::new("INVALID_ANSWER", "");
        if input.len() > 128 {
            return Err(KnotError::limit("decimalBytes", 128, input.len()));
        }
        let negative = input.starts_with('-');
        let raw = input.strip_prefix('-').unwrap_or(input);
        let (whole, fraction) = match raw.split_once('.') {
            Some(parts) => parts,
            None => (raw, ""),
        };
        if whole.is_empty()
            || !whole.bytes().all(|c| c.is_ascii_digit())
            || !fraction.bytes().all(|c| c.is_ascii_digit())
            || (raw.contains('.') && fraction.is_empty())
        {
            return Err(invalid());
        }
        let fraction = fraction.trim_end_matches('0');
        if fraction.len() > 9 {
            return Err(invalid());
        }
        let scale = fraction.len() as u32;
        let mut coefficient = 0i128;
        for b in whole.bytes().chain(fraction.bytes()) {
            coefficient = coefficient
                .checked_mul(10)
                .and_then(|v| v.checked_add(i128::from(b - b'0')))
                .ok_or_else(invalid)?;
        }
        if coefficient > i128::from(SAFE_INTEGER) * 10i128.pow(scale) {
            return Err(invalid());
        }
        Ok(Self {
            coefficient: if negative { -coefficient } else { coefficient },
            scale,
        })
    }

    pub fn coefficient(self) -> i128 {
        self.coefficient
    }
    pub fn scale(self) -> u32 {
        self.scale
    }

    /// Common scale 9 is exact for every valid protocol decimal (at most 25 digits).
    pub fn scaled(self) -> i128 {
        self.coefficient * 10i128.pow(9 - self.scale)
    }

    pub fn fits_step(self, anchor: Self, step: Self) -> bool {
        step.coefficient > 0 && (self.scaled() - anchor.scaled()) % step.scaled() == 0
    }

    /// Component-local multiplication with midpoint rounding away from zero.
    pub fn multiply_minor(self, unit: i64) -> Result<i64, KnotError> {
        let overflow = || KnotError::new("NUMERIC_OVERFLOW", "");
        if unit.unsigned_abs() > SAFE_INTEGER as u64 {
            return Err(overflow());
        }
        let raw = self
            .coefficient
            .checked_mul(i128::from(unit))
            .ok_or_else(overflow)?;
        let denominator = 10i128.pow(self.scale);
        let quotient = raw / denominator;
        let remainder = raw % denominator;
        let rounded = if remainder.abs() * 2 >= denominator {
            quotient.checked_add(raw.signum()).ok_or_else(overflow)?
        } else {
            quotient
        };
        if rounded.abs() > i128::from(SAFE_INTEGER) {
            return Err(overflow());
        }
        i64::try_from(rounded).map_err(|_| overflow())
    }
}
impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.scaled().cmp(&other.scaled())
    }
}
impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl std::fmt::Display for Decimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.coefficient == 0 {
            return f.write_str("0");
        }
        let sign = if self.coefficient < 0 { "-" } else { "" };
        let magnitude = self.coefficient.abs();
        if self.scale == 0 {
            return write!(f, "{sign}{magnitude}");
        }
        let base = 10i128.pow(self.scale);
        write!(
            f,
            "{sign}{}.{:0width$}",
            magnitude / base,
            magnitude % base,
            width = self.scale as usize
        )
    }
}
