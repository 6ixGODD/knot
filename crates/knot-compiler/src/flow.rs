use crate::{
    IndexedEdge, dependencies,
    semantic::{bad, required},
};
use knot_schema::{
    KnotError,
    wire::{Id, Product},
};
use std::collections::{BTreeMap, VecDeque};

pub(crate) struct Graph {
    pub edges: Vec<IndexedEdge>,
    pub successors: Vec<Vec<usize>>,
}
pub(crate) fn validate(product: &Product, index: &BTreeMap<Id, usize>) -> Result<Graph, KnotError> {
    let n = product.questions.len();
    let start = *index
        .get(&product.flow.start)
        .ok_or_else(|| bad("/flow/start"))?;
    let mut edges = Vec::new();
    let mut grouped = vec![vec![]; n];
    let mut adjacency = vec![vec![]; n];
    let mut indegree = vec![0; n];
    let mut reaches_end = vec![false; n];
    for (i, e) in product.flow.edges.iter().enumerate() {
        let from = *index
            .get(&e.from)
            .ok_or_else(|| bad(&format!("/flow/edges/{i}/from")))?;
        let to = if e.to.as_str() == "END" {
            None
        } else {
            Some(
                *index
                    .get(&e.to)
                    .ok_or_else(|| bad(&format!("/flow/edges/{i}/to")))?,
            )
        };
        grouped[from].push(i);
        if let Some(to) = to {
            adjacency[from].push(to);
            indegree[to] += 1;
        } else {
            reaches_end[from] = true;
        }
        edges.push(IndexedEdge {
            from,
            to,
            condition: None,
        });
    }
    for (q, group) in grouped.iter().enumerate() {
        if group.is_empty() {
            return Err(bad(&format!("/questions/{q}")));
        }
        if group.len() > 1 {
            for (pos, &i) in group.iter().enumerate() {
                if product.flow.edges[i].when.is_none() != (pos == group.len() - 1) {
                    return Err(bad(&format!("/flow/edges/{i}/when")));
                }
            }
        }
    }
    let mut ready: VecDeque<_> = (0..n).filter(|&i| indegree[i] == 0).collect();
    let mut topo = Vec::new();
    while let Some(from) = ready.pop_front() {
        topo.push(from);
        for &to in &adjacency[from] {
            indegree[to] -= 1;
            if indegree[to] == 0 {
                ready.push_back(to);
            }
        }
    }
    if topo.len() != n {
        return Err(bad("/flow"));
    }
    let successors = dependencies::successors(&adjacency, &topo);
    for &q in topo.iter().rev() {
        let via_successor = adjacency[q].iter().any(|&to| reaches_end[to]);
        reaches_end[q] |= via_successor;
    }
    for (q, question) in product.questions.iter().enumerate() {
        let reachable = q == start || successors[start].binary_search(&q).is_ok();
        if (!reachable && required(question)) || (reachable && !reaches_end[q]) {
            return Err(bad(&format!("/questions/{q}")));
        }
    }
    Ok(Graph { edges, successors })
}
