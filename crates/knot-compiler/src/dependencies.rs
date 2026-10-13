//! Reverse-topological bitsets; no path enumeration or dominance assumption.
pub(crate) fn successors(adjacency: &[Vec<usize>], topological: &[usize]) -> Vec<Vec<usize>> {
    let n = adjacency.len();
    let mut bits = vec![[0u64; 4]; n]; // at most 256 questions
    for &from in topological.iter().rev() {
        for &to in &adjacency[from] {
            bits[from][to / 64] |= 1 << (to % 64);
            let child = bits[to];
            for (word, value) in child.iter().enumerate() {
                bits[from][word] |= value;
            }
        }
    }
    bits.iter()
        .map(|set| {
            (0..n)
                .filter(|&i| set[i / 64] & (1 << (i % 64)) != 0)
                .collect()
        })
        .collect()
}
