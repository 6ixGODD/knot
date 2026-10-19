use knot_schema::wire::*;
pub(crate) fn empty() -> Changes {
    Changes {
        answer_changes: Vec::new(),
        price_changed: false,
        visual_changed_targets: Vec::new(),
        flow_changed: false,
    }
}
pub(crate) fn derive(old: &Snapshot, new: &Snapshot, answer_changes: Vec<AnswerChange>) -> Changes {
    Changes {
        answer_changes,
        price_changed: old.price != new.price,
        flow_changed: old.flow != new.flow,
        visual_changed_targets: new
            .visual
            .targets
            .iter()
            .filter(|t| {
                old.visual
                    .targets
                    .iter()
                    .find(|o| o.id == t.id)
                    .is_none_or(|o| o.properties != t.properties)
            })
            .map(|t| t.id.clone())
            .collect(),
    }
}
