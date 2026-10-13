use crate::{CompiledModel, ModelParts, digest, flow, lower, semantic};
use knot_rules::{CompileBudget, RuleCompiler};
use knot_schema::{
    KnotError, parse::PRODUCT_BYTES, parse_json, validation::Validator, wire::Product,
};
use serde_json::Value;
use std::sync::OnceLock;

static SCHEMA: OnceLock<Result<Validator, KnotError>> = OnceLock::new();

/// Compile one bounded Product document with an injected rule backend.
///
/// Parsing, schema, definition, graph and dependency checks precede backend
/// compilation. No partial model is returned. Defaults, decimals and colors are
/// normalized before hashing; declaration order and metadata are retained.
/// Optional unreachable questions are available through `CompiledModel::diagnostics`.
/// All resources are embedded; this operation performs no file or network I/O.
///
/// Storage is bounded by the 4 MiB input, 32,768 AST nodes, 4,096 roots and
/// 256 questions. Graph closure uses 8 KiB of temporary bitsets and at most
/// 65,536 stored successor indices. Backend expression and constant costs remain
/// owned by the injected compiler; ZEN uses the frozen prepaid template model.
pub fn compile_product(
    bytes: &[u8],
    backend: &dyn RuleCompiler,
) -> Result<CompiledModel, KnotError> {
    let value = parse_json(bytes, PRODUCT_BYTES)?;
    preflight(&value)?;
    let schema = SCHEMA
        .get_or_init(|| {
            let schema = serde_json::from_slice(include_bytes!(
                "../../../contracts/schemas/product.schema.json"
            ))
            .map_err(|_| KnotError::new("INTERNAL", ""))?;
            Validator::new(&schema)
        })
        .as_ref()
        .map_err(Clone::clone)?;
    schema.validate(&value)?;
    let mut product: Product =
        serde_json::from_value(value).map_err(|_| KnotError::new("BAD_SCHEMA", ""))?;
    let question_index = semantic::definitions(&mut product)?;
    let graph = flow::validate(&product, &question_index)?;
    semantic::outputs(&mut product, &question_index)?;
    let lowered = lower::product(&mut product, &question_index, &graph.successors)?;
    let digest = digest::digest(&product)?;
    let program = backend.compile(
        &lowered.fields,
        &lowered.conditions,
        &lowered.sources,
        &mut CompileBudget::default(),
    )?;
    let mut edges = graph.edges;
    for (i, edge) in edges.iter_mut().enumerate() {
        edge.condition = lowered.edge_conditions[i];
    }
    let mut target_order: Vec<_> = (0..product.visual.targets.len()).collect();
    target_order.sort_by(|&a, &b| {
        let a = &product.visual.targets[a];
        let b = &product.visual.targets[b];
        a.order
            .cmp(&b.order)
            .then_with(|| a.id.as_str().as_bytes().cmp(b.id.as_str().as_bytes()))
    });
    CompiledModel::new(ModelParts {
        product,
        question_index,
        edges,
        successors: graph.successors,
        target_order,
        sources: lowered.sources,
        program,
        digest,
    })
}

fn limit(value: &Value, maximum: usize, name: &str) -> Result<(), KnotError> {
    if let Some(items) = value.as_array() {
        check(items.len(), maximum, name)?;
    }
    Ok(())
}
pub(crate) fn check(observed: usize, maximum: usize, name: &str) -> Result<(), KnotError> {
    if observed > maximum {
        Err(KnotError::limit(name, maximum, observed))
    } else {
        Ok(())
    }
}

// Inspect raw values before recursive wire deserialization/Schema traversal or
// IR allocation. The shared parser has already bounded bytes and JSON depth.
fn preflight(value: &Value) -> Result<(), KnotError> {
    limit(&value["questions"], 256, "questions")?;
    limit(&value["flow"]["edges"], 2048, "flowEdges")?;
    limit(&value["pricing"]["components"], 1024, "priceComponents")?;
    limit(&value["visual"]["targets"], 256, "visualTargets")?;
    limit(&value["visual"]["slots"], 2048, "visualSlots")?;
    let mut roots = Vec::new();
    let mut options = 0;
    if let Some(questions) = value["questions"].as_array() {
        for q in questions {
            if let Some(items) = q["options"].as_array() {
                check(items.len(), 256, "optionsPerQuestion")?;
                options += items.len();
                check(options, 8192, "optionsTotal")?;
                for item in items {
                    if let Some(c) = item.get("enabledWhen") {
                        roots.push(c);
                    }
                }
            }
        }
    }
    for items in [&value["flow"]["edges"], &value["pricing"]["components"]] {
        if let Some(items) = items.as_array() {
            for item in items {
                if let Some(c) = item.get("when") {
                    roots.push(c);
                }
            }
        }
    }
    let mut cases = 0;
    if let Some(slots) = value["visual"]["slots"].as_array() {
        for slot in slots {
            if let Some(items) = slot["cases"].as_array() {
                cases += items.len();
                check(cases, 4096, "visualCases")?;
                for item in items {
                    if let Some(c) = item.get("when") {
                        roots.push(c);
                    }
                }
            }
        }
    }
    check(roots.len(), 4096, "conditionRoots")?;
    let mut budget = CompileBudget::default();
    let mut pending: Vec<_> = roots.into_iter().map(|c| (c, 1)).collect();
    while let Some((node, depth)) = pending.pop() {
        budget.node(depth)?;
        for key in ["all", "any"] {
            if let Some(items) = node.get(key).and_then(Value::as_array) {
                pending.extend(items.iter().map(|n| (n, depth + 1)));
            }
        }
        if let Some(n) = node.get("not") {
            pending.push((n, depth + 1));
        }
        if node["op"] == "in" {
            limit(&node["value"], 256, "inMembers")?;
        }
    }
    Ok(())
}
