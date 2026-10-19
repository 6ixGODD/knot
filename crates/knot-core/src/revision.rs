//! Revision conflict checks precede no-op detection; failed calls consume no revision.
use knot_schema::{KnotError, SAFE_INTEGER};

pub fn check_revision(actual: i64, expected: i64) -> Result<(), KnotError> {
    if !(0..=SAFE_INTEGER).contains(&actual) {
        return Err(KnotError::new("INVALID_STATE", "/revision"));
    }
    if !(0..=SAFE_INTEGER).contains(&expected) {
        return Err(KnotError::new("BAD_SCHEMA", "/expectedRevision"));
    }
    if actual != expected {
        let mut error = KnotError::new("REVISION_CONFLICT", "/expectedRevision");
        error.details =
            Box::new(serde_json::json!({"expectedRevision":expected,"actualRevision":actual}));
        return Err(error);
    }
    Ok(())
}

pub fn next_revision(actual: i64, changed: bool) -> Result<i64, KnotError> {
    check_revision(actual, actual)?;
    if !changed {
        return Ok(actual);
    }
    actual
        .checked_add(1)
        .filter(|n| *n <= SAFE_INTEGER)
        .ok_or_else(|| KnotError::new("REVISION_OVERFLOW", "/revision"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_noop_and_conflict() {
        assert_eq!(next_revision(0, true), Ok(1));
        assert_eq!(next_revision(SAFE_INTEGER, false), Ok(SAFE_INTEGER));
        assert!(next_revision(SAFE_INTEGER, true).is_err());
        assert!(next_revision(-1, false).is_err());
        let error = check_revision(2, 1)
            .err()
            .unwrap_or_else(|| panic!("conflict required"));
        assert_eq!(error.status(), 1006);
        assert_eq!(error.details["actualRevision"], 2);
        assert!(check_revision(SAFE_INTEGER + 1, SAFE_INTEGER + 1).is_err());
    }
}
