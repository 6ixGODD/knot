//! Exact bounded completion search. All transitions share the caller's fuel.
mod domain;
use crate::evaluation::Evaluation;
use crate::{flow, output, snapshot, state, transition};
use knot_compiler::CompiledModel;
use knot_schema::{KnotError, quote::*, wire::*};
struct Search<'a> {
    model: &'a CompiledModel,
    budget: Evaluation,
    nodes: usize,
    exhausted: bool,
}
impl Search<'_> {
    fn tick(&mut self) -> Result<(), KnotError> {
        if self.nodes >= 10000 {
            self.exhausted = true;
            return Err(KnotError::limit("quoteSearch", 10000, self.nodes + 1));
        }
        self.nodes += 1;
        self.budget.charge(1)
    }
    fn empty(&self, status: QuoteStatus, reason: Option<QuoteReason>) -> PriceQuote {
        PriceQuote {
            status,
            currency: self.model.product().pricing.currency.clone(),
            min_minor: None,
            max_minor: None,
            reason,
        }
    }
    fn unavailable(&self, reason: QuoteReason) -> PriceQuote {
        self.empty(QuoteStatus::Unavailable, Some(reason))
    }
    fn complete(
        &mut self,
        state: &ConfigurationState,
        snapshot: &Snapshot,
    ) -> Result<PriceQuote, KnotError> {
        self.complete_root(state, snapshot, None)
    }
    fn complete_root(
        &mut self,
        state: &ConfigurationState,
        snapshot: &Snapshot,
        candidates: Option<&mut [QuoteCandidate]>,
    ) -> Result<PriceQuote, KnotError> {
        if snapshot.flow.status == FlowStatus::Complete {
            let amount = snapshot
                .price
                .total_minor
                .ok_or_else(|| KnotError::new("UNPRICED", ""))?;
            return Ok(PriceQuote {
                status: QuoteStatus::Final,
                currency: snapshot.price.currency.clone(),
                min_minor: Some(amount),
                max_minor: Some(amount),
                reason: None,
            });
        }
        if self.exhausted {
            return Ok(self.unavailable(QuoteReason::SearchLimit));
        }
        match self.walk(state, snapshot, candidates) {
            Ok(p) => Ok(p),
            Err(e) if e.code == "RESOURCE_LIMIT" => {
                self.exhausted = true;
                Ok(self.unavailable(QuoteReason::SearchLimit))
            }
            Err(e) => Err(e),
        }
    }
    fn walk(
        &mut self,
        old: &ConfigurationState,
        snap: &Snapshot,
        mut candidates: Option<&mut [QuoteCandidate]>,
    ) -> Result<PriceQuote, KnotError> {
        if snap.flow.status == FlowStatus::Complete {
            return self.complete(old, snap);
        }
        if snap.flow.status == FlowStatus::Blocked {
            return Ok(self.empty(QuoteStatus::NoCompletion, None));
        }
        let id = snap
            .flow
            .next_question_id
            .as_ref()
            .ok_or_else(|| KnotError::new("INTERNAL", ""))?;
        let index = *self
            .model
            .question_index()
            .get(id)
            .ok_or_else(|| KnotError::new("INVALID_MODEL", ""))?;
        let q = &self.model.product().questions[index];
        self.tick()?; // charge expansion before allocating its domain or facts
        let facts = state::facts(self.model, &old.answers, &mut self.budget)?;
        let options = flow::availability(self.model, index, &facts, None, &mut self.budget)?;
        let Some(mut domain) = domain::Domain::new(self.model, q, &options, &mut self.budget)?
        else {
            return Ok(self.unavailable(QuoteReason::DomainLimit));
        };
        let optional = !state::required(q);
        let mut skipped = false;
        let mut result = self.empty(QuoteStatus::NoCompletion, None);
        loop {
            self.tick()?;
            let event = if let Some(value) = domain.next()? {
                ConfigurationEvent::SetAnswer {
                    schema_version: Version::V1,
                    question_id: id.clone(),
                    expected_revision: old.revision,
                    value,
                }
            } else if optional && !skipped {
                skipped = true;
                ConfigurationEvent::SkipAnswer {
                    schema_version: Version::V1,
                    question_id: id.clone(),
                    expected_revision: old.revision,
                }
            } else {
                break;
            };
            let next = transition::apply_validated(
                self.model,
                old,
                snap,
                &event,
                &mut self.budget,
                false,
            )?;
            let child = self.walk(&next.state, &next.snapshot, None)?;
            if child.status == QuoteStatus::Unavailable {
                return Ok(child);
            }
            // Only this root's exact SetAnswer input and fully explored subtree
            // are reused. Existing bounded output slots own the result; no leaf
            // states or cross-request results are retained.
            if let (
                Some(entries),
                ConfigurationEvent::SetAnswer {
                    value: Answer::Text(value),
                    ..
                },
            ) = (candidates.as_deref_mut(), &event)
            {
                for candidate in entries {
                    self.budget
                        .charge((value.len() + candidate.option_id.as_str().len() + 1) as u64)?;
                    if candidate.enabled && candidate.option_id.as_str() == value {
                        self.budget.charge(
                            (child.currency.len() + std::mem::size_of::<PriceQuote>()) as u64,
                        )?;
                        candidate.price = Some(child.clone());
                        break;
                    }
                }
            }
            if let (Some(min), Some(max)) = (child.min_minor, child.max_minor) {
                result.status = QuoteStatus::Range;
                result.min_minor = Some(result.min_minor.map_or(min, |n| n.min(min)));
                result.max_minor = Some(result.max_minor.map_or(max, |n| n.max(max)));
            }
        }
        Ok(result)
    }
    fn candidate(
        &mut self,
        old: &ConfigurationState,
        id: &Id,
        value: &Answer,
        snapshot: &Snapshot,
    ) -> Result<(Option<PriceQuote>, Option<QuoteError>), KnotError> {
        let index = *self
            .model
            .question_index()
            .get(id)
            .ok_or_else(|| KnotError::new("INACTIVE_QUESTION", "/focusQuestionId"))?;
        if let Err(error) = state::normalize(&self.model.product().questions[index], value) {
            return Ok((None, Some(error.into())));
        }
        if self.exhausted {
            return Ok((Some(self.unavailable(QuoteReason::SearchLimit)), None));
        }
        let result = (|| {
            self.tick()?;
            transition::apply_validated(
                self.model,
                old,
                snapshot,
                &ConfigurationEvent::SetAnswer {
                    schema_version: Version::V1,
                    question_id: id.clone(),
                    expected_revision: old.revision,
                    value: value.clone(),
                },
                &mut self.budget,
                false,
            )
        })();
        match result {
            Ok(next) => Ok((Some(self.complete(&next.state, &next.snapshot)?), None)),
            Err(e)
                if matches!(e.code.as_str(), "INVALID_ANSWER" | "INVALID_OPTION")
                    && e.path == state::pointer(id) =>
            {
                Ok((None, Some(e.into())))
            }
            Err(e) if e.code == "RESOURCE_LIMIT" => {
                self.exhausted = true;
                Ok((Some(self.unavailable(QuoteReason::SearchLimit)), None))
            }
            Err(e) => Err(e),
        }
    }
}
/// Quote complete configurations reachable from this immutable state.
pub fn quote(model: &CompiledModel, request: &QuoteRequest) -> Result<QuoteResult, KnotError> {
    // Rust callers receive the same admission checks as JSON callers.
    let bytes = knot_schema::protocol::encode(request)?;
    knot_schema::protocol::quote(&bytes)?;
    let mut search = Search {
        model,
        budget: Evaluation::quoting(),
        nodes: 0,
        exhausted: false,
    };
    let old = &request.state;
    let base = snapshot(model, old, &mut search.budget)?;
    let focus_id = request
        .focus_question_id
        .as_ref()
        .or(base.flow.next_question_id.as_ref());
    let mut focus = if let Some(id) = focus_id {
        if !base.flow.path.contains(id) {
            return Err(KnotError::new("INACTIVE_QUESTION", "/focusQuestionId"));
        }
        let index = *model
            .question_index()
            .get(id)
            .ok_or_else(|| KnotError::new("INACTIVE_QUESTION", "/focusQuestionId"))?;
        let q = &model.product().questions[index];
        if request.candidate_value.is_some()
            && (request.focus_question_id.is_none()
                || matches!(q, Question::Single { .. } | Question::Multi { .. }))
        {
            return Err(KnotError::new("BAD_SCHEMA", "/candidateValue"));
        }
        let preceding = base
            .flow
            .path
            .iter()
            .take_while(|other| *other != id)
            .filter_map(|other| old.answers.get(other).map(|a| (other.clone(), a.clone())))
            .collect();
        let facts = state::facts(model, &preceding, &mut search.budget)?;
        let options = flow::availability(model, index, &facts, None, &mut search.budget)?;
        if options.len() > 256 {
            return Err(KnotError::limit("quoteCandidates", 256, options.len()));
        }
        let mut candidates = Vec::new();
        for o in &options {
            search.budget.charge((o.id.as_str().len() + 1) as u64)?;
            let value = if matches!(q, Question::Multi { .. }) {
                let selected = match old.answers.get(id) {
                    Some(Answer::Multi(v)) => v.as_slice(),
                    _ => &[],
                };
                Answer::Multi(
                    options
                        .iter()
                        .filter(|v| {
                            if v.id == o.id {
                                !selected.contains(&v.id)
                            } else {
                                selected.contains(&v.id)
                            }
                        })
                        .map(|v| v.id.clone())
                        .collect(),
                )
            } else {
                Answer::Text(o.id.as_str().into())
            };
            let rule_ids = if o.enabled {
                vec![]
            } else {
                flow::disabled(model, index, &Answer::Text(o.id.as_str().into()), &options)
            };
            candidates.push(QuoteCandidate {
                option_id: o.id.clone(),
                candidate_value: value,
                enabled: o.enabled,
                rule_ids,
                price: None,
                error: None,
            });
        }
        Some(QuoteFocus {
            question: flow::current(q, options),
            answer: old.answers.get(id).cloned(),
            skipped: old.skipped.contains(id),
            candidates,
            preview: request
                .candidate_value
                .clone()
                .map(|candidate_value| QuotePreview {
                    candidate_value,
                    price: None,
                    error: None,
                }),
        })
    } else {
        if request.candidate_value.is_some() {
            return Err(KnotError::new("BAD_SCHEMA", "/candidateValue"));
        }
        None
    };
    let root_candidates = focus
        .as_mut()
        .filter(|f| {
            matches!(f.question, CurrentQuestion::Single { .. })
                && focus_id == base.flow.next_question_id.as_ref()
                && f.answer.is_none()
                && !f.skipped
        })
        .map(|f| f.candidates.as_mut_slice());
    let mut price = search.complete_root(old, &base, root_candidates)?;
    if let (Some(id), Some(f)) = (focus_id, focus.as_mut()) {
        for c in &mut f.candidates {
            if c.enabled && c.price.is_none() {
                (c.price, c.error) = search.candidate(old, id, &c.candidate_value, &base)?;
            }
        }
        if let Some(p) = f.preview.as_mut() {
            (p.price, p.error) = search.candidate(old, id, &p.candidate_value, &base)?;
            if p.error.is_none() {
                let index = *model
                    .question_index()
                    .get(id)
                    .ok_or_else(|| KnotError::new("INACTIVE_QUESTION", "/focusQuestionId"))?;
                p.candidate_value =
                    state::normalize(&model.product().questions[index], &p.candidate_value)?;
            }
        }
    }
    if search.exhausted {
        if price.status != QuoteStatus::Final {
            price = search.unavailable(QuoteReason::SearchLimit);
        }
        if let Some(f) = focus.as_mut() {
            for c in &mut f.candidates {
                if c.price.is_some() {
                    c.price = Some(search.unavailable(QuoteReason::SearchLimit));
                }
            }
            if let Some(p) = f.preview.as_mut()
                && p.price.is_some()
            {
                p.price = Some(search.unavailable(QuoteReason::SearchLimit));
            }
        }
    }
    let health_facts = state::facts(model, &old.answers, &mut search.budget)?;
    search.budget.health(model, &health_facts)?;
    output::quote(
        QuoteResult {
            schema_version: Version::V1,
            model_digest: model.digest().into(),
            revision: old.revision,
            price,
            focus,
        },
        &mut search.budget,
    )
}
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
pub(crate) mod support;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[allow(clippy::unwrap_used)]
    fn optional_multi_search_visits_each_subset_and_skip_once() {
        use std::sync::{Arc, Mutex};
        let seen = Arc::new(Mutex::new(Vec::new()));
        let observed = seen.clone();
        let mut product = support::product(vec![Question::Multi {
            id: support::id("a"),
            label: "A".into(),
            required: Some(false),
            options: vec![support::option("x", false), support::option("y", false)],
        }]);
        product.pricing.components.push(PriceComponent::Fixed {
            id: support::id("observe"),
            category: PriceCategory::Adjustment,
            amount_minor: support::n(0),
            when: Some(support::condition()),
        });
        let model = support::controlled(
            product,
            move |_, facts| {
                let key = match &facts.values()[0] {
                    None => "skip".into(),
                    Some(knot_rules::FactValue::Multi(ids)) => {
                        ids.iter().map(Id::as_str).collect::<Vec<_>>().join(",")
                    }
                    _ => panic!("unexpected fact"),
                };
                observed.lock().unwrap().push(key);
                Ok(true)
            },
            1,
        );
        let initial = crate::initial(&model).unwrap();
        seen.lock().unwrap().clear();
        let mut search = Search {
            model: &model,
            budget: Evaluation::quoting(),
            nodes: 0,
            exhausted: false,
        };
        assert_eq!(
            search
                .complete(&initial.state, &initial.snapshot)
                .unwrap()
                .status,
            QuoteStatus::Range
        );
        let mut actual = seen.lock().unwrap().clone();
        actual.sort();
        assert_eq!(actual, ["", "skip", "x", "x,y", "y"]);
        assert_eq!(search.nodes, 7); // one expansion, five events, one termination
    }
    #[test]
    #[allow(clippy::unwrap_used)]
    fn root_candidates_reuse_only_the_same_fully_explored_subtrees() {
        let model = support::model(support::product(vec![
            support::single("q", true),
            support::single("r", true),
        ]));
        let initial = crate::initial(&model).unwrap();
        let expected = quote(
            &model,
            &QuoteRequest {
                schema_version: Version::V1,
                state: initial.state.clone(),
                focus_question_id: None,
                candidate_value: None,
            },
        )
        .unwrap();
        let mut candidates = expected.focus.unwrap().candidates;
        for c in &mut candidates {
            c.price = None;
        }
        let mut reused = Search {
            model: &model,
            budget: Evaluation::quoting(),
            nodes: 0,
            exhausted: false,
        };
        let main = reused
            .complete_root(&initial.state, &initial.snapshot, Some(&mut candidates))
            .unwrap();
        let mut independent = Search {
            model: &model,
            budget: Evaluation::quoting(),
            nodes: 0,
            exhausted: false,
        };
        assert_eq!(
            main,
            independent
                .complete(&initial.state, &initial.snapshot)
                .unwrap()
        );
        assert_eq!(reused.nodes, independent.nodes);
        let before = independent.nodes;
        for candidate in candidates {
            let (price, error) = independent
                .candidate(
                    &initial.state,
                    &support::id("q"),
                    &candidate.candidate_value,
                    &initial.snapshot,
                )
                .unwrap();
            assert_eq!(candidate.price, price);
            assert!(error.is_none());
        }
        assert!(independent.nodes > before);
    }
    #[test]
    fn node_ceiling_produces_an_encodable_unavailable_and_spent_fuel_cannot_encode() {
        let model = support::model(support::product(vec![support::single("q", true)]));
        let initial = crate::initial(&model).unwrap_or_else(|e| panic!("{e}"));
        let mut search = Search {
            model: &model,
            budget: Evaluation::new(),
            nodes: 10000,
            exhausted: false,
        };
        let price = search
            .complete(&initial.state, &initial.snapshot)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(price.status, QuoteStatus::Unavailable);
        assert_eq!(price.reason, Some(QuoteReason::SearchLimit));
        assert!(price.min_minor.is_none());
        assert!(price.max_minor.is_none());
        let result = QuoteResult {
            schema_version: Version::V1,
            model_digest: model.digest().into(),
            revision: initial.state.revision,
            price,
            focus: None,
        };
        assert!(output::quote(result.clone(), &mut search.budget).is_ok());
        let mut exhausted = Evaluation::new();
        assert!(exhausted.charge(2_000_000).is_ok());
        assert!(matches!(output::quote(result,&mut exhausted),Err(e) if e.code=="RESOURCE_LIMIT"));
    }
}
