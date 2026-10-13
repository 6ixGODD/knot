//! Monotonic budgets cannot be reset between coordination rounds.
use knot_schema::KnotError;
#[derive(Debug)]
pub struct CallBudget {
    remaining: u64,
}
impl CallBudget {
    pub fn new() -> Self {
        Self {
            remaining: 2_000_000,
        }
    }
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
    pub fn charge(&mut self, cost: u64) -> Result<(), KnotError> {
        self.remaining = self
            .remaining
            .checked_sub(cost)
            .ok_or_else(|| KnotError::limit("callCost", 2_000_000, 2_000_001))?;
        Ok(())
    }
}
impl Default for CallBudget {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Default)]
pub struct CompileBudget {
    roots: usize,
    nodes: usize,
    expressions: usize,
}
impl CompileBudget {
    pub fn root(&mut self) -> Result<(), KnotError> {
        Self::add(&mut self.roots, 1, 4096, "conditionRoots")
    }
    pub fn node(&mut self, depth: usize) -> Result<(), KnotError> {
        if depth > 32 {
            return Err(KnotError::limit("astDepth", 32, depth));
        }
        Self::add(&mut self.nodes, 1, 32768, "astNodes")
    }
    pub fn expression(&mut self, bytes: usize) -> Result<(), KnotError> {
        if bytes > 65536 {
            return Err(KnotError::limit("expressionBytes", 65536, bytes));
        }
        Self::add(
            &mut self.expressions,
            bytes,
            8 * 1024 * 1024,
            "expressionsTotalBytes",
        )
    }
    fn add(total: &mut usize, n: usize, max: usize, name: &str) -> Result<(), KnotError> {
        let next = total
            .checked_add(n)
            .ok_or_else(|| KnotError::limit(name, max, usize::MAX))?;
        if next > max {
            return Err(KnotError::limit(name, max, next));
        }
        *total = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cumulative_fuel_and_compile_caps() {
        let mut b = CallBudget::new();
        assert!(b.charge(1_999_999).is_ok());
        assert!(b.charge(2).is_err());
        assert_eq!(b.remaining(), 1);
        assert!(b.charge(1).is_ok());
        assert!(b.charge(1).is_err());
        let mut c = CompileBudget::default();
        assert!(c.node(33).is_err());
        assert!(c.expression(65537).is_err());
        for _ in 0..128 {
            assert!(c.expression(65536).is_ok());
        }
        assert!(c.expression(1).is_err());
    }
}
