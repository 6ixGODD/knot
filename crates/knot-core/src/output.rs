use knot_rules::CallBudget;
use knot_schema::{KnotError, wire::TransitionResult};
use std::io::{self, Write};
const MAXIMUM: usize = 4 * 1024 * 1024;
// Serialization goes to a bounded counting sink, never an unbounded output Vec.
struct Counter<'a> {
    bytes: usize,
    exceeded: bool,
    budget: &'a mut CallBudget,
    failure: Option<KnotError>,
}
impl Write for Counter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAXIMUM - self.bytes {
            self.exceeded = true;
            return Err(io::Error::other("output limit"));
        }
        if let Err(error) = self.budget.charge(bytes.len() as u64) {
            self.failure = Some(error);
            return Err(io::Error::other("call budget"));
        }
        self.bytes += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(crate) fn finish(
    result: TransitionResult,
    budget: &mut CallBudget,
) -> Result<TransitionResult, KnotError> {
    let mut sink = Counter {
        bytes: 0,
        exceeded: false,
        budget,
        failure: None,
    };
    let serialized = serde_json::to_writer(&mut sink, &result);
    if sink.exceeded {
        return Err(KnotError::limit("outputBytes", MAXIMUM, MAXIMUM + 1));
    }
    if let Some(error) = sink.failure {
        return Err(error);
    }
    serialized.map_err(|_| KnotError::new("INTERNAL", ""))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_boundary_and_budget_are_checked_before_accepting_bytes() {
        let mut budget = CallBudget::new();
        let mut sink = Counter {
            bytes: MAXIMUM - 1,
            exceeded: false,
            budget: &mut budget,
            failure: None,
        };
        assert!(sink.write_all(&[0]).is_ok());
        assert_eq!(sink.bytes, MAXIMUM);
        assert!(sink.write_all(&[0]).is_err());
        assert_eq!(sink.bytes, MAXIMUM);
        assert!(sink.exceeded);
        let mut budget = CallBudget::new();
        assert!(budget.charge(1_999_999).is_ok());
        let mut sink = Counter {
            bytes: 0,
            exceeded: false,
            budget: &mut budget,
            failure: None,
        };
        assert!(sink.write_all(&[0, 0]).is_err());
        assert_eq!(sink.bytes, 0);
        assert_eq!(
            sink.failure.as_ref().map(|e| e.code.as_str()),
            Some("RESOURCE_LIMIT")
        );
    }
}
