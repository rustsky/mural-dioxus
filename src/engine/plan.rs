use std::collections::HashSet;

use super::learning::{LearnerState, RecurringError, WordState};
use super::models::{Assessment, EvidenceKind, Outcome};
use super::time::Date;

/// One teaching action for the next reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Move {
    /// Meaning broke down: recast it and lower the demand.
    Repair,
    /// An error the learner keeps making turned up again: recast it in passing.
    Recast { label: String, correction: String },
    /// Build a reason for the learner to use a word that is due again.
    Retrieve { lemma: String, meaning: String },
    /// Offer one new expression, then set up its later retrieval.
    Introduce,
    /// The learner has room to spare: ask for reasons or a short story.
    Stretch,
    /// Stay here and let the learner say more.
    Consolidate,
}

const REPAIR: u8 = 0;
const RECAST: u8 = 1;
const RETRIEVE: u8 = 2;
const INTRODUCE: u8 = 3;
const STRETCH: u8 = 4;
const CONSOLIDATE: u8 = 5;

impl Move {
    fn kind(&self) -> u8 {
        match self {
            Move::Repair => REPAIR,
            Move::Recast { .. } => RECAST,
            Move::Retrieve { .. } => RETRIEVE,
            Move::Introduce => INTRODUCE,
            Move::Stretch => STRETCH,
            Move::Consolidate => CONSOLIDATE,
        }
    }
}

/// Chooses one move per assessed turn. Nothing here is saved: a new conversation starts with a
/// fresh plan, while challenge and word strength keep coming from the learning record.
#[derive(Clone, Debug)]
pub struct TeachingPlan {
    quiet_turns: u32,
    since_retrieve: u32,
    since_recast: u32,
    prompted: HashSet<String>,
    last: Option<u8>,
}

impl Default for TeachingPlan {
    fn default() -> Self {
        // An error is only on the recurring list because it happened before, so the first time it
        // comes up in a conversation is already the right moment to recast it. Retrieval waits,
        // because opening a conversation by fishing for a word is drilling, not talking.
        Self { quiet_turns: 0, since_retrieve: 0, since_recast: 2, prompted: HashSet::new(), last: None }
    }
}

impl TeachingPlan {
    /// Call only after LearningEngine::validate has accepted the assessment.
    /// Returns None when the move would repeat the last one, so the conversation is not nagged.
    pub fn next(&mut self, assessment: &Assessment, learner: &LearnerState, language_id: &str, now: Date) -> Option<Move> {
        let independent = assessment.words.iter()
            .any(|w| w.language == language_id && w.kind == EvidenceKind::Independent);
        self.quiet_turns = if independent { 0 } else { self.quiet_turns.saturating_add(1) };
        self.since_retrieve = self.since_retrieve.saturating_add(1);
        self.since_recast = self.since_recast.saturating_add(1);

        // A breakdown is always worth addressing, even twice in a row.
        if assessment.outcome == Outcome::Breakdown {
            self.last = Some(REPAIR);
            return Some(Move::Repair);
        }
        // A recurring error earns one gentle recast, not a correction every time it appears.
        if self.since_recast >= 2 {
            if let Some(error) = Self::recurring_hit(assessment, learner, language_id) {
                self.since_recast = 0;
                self.last = Some(RECAST);
                return Some(Move::Recast { label: error.label.clone(), correction: error.correction.clone() });
            }
        }
        // Retrieval needs room to breathe; every turn would turn conversation into drilling.
        if self.since_retrieve >= 2 {
            if let Some(word) = self.due_word(learner, now) {
                let (lemma, meaning, id) = (word.lemma.clone(), word.meaning.clone(), word.id.clone());
                self.prompted.insert(id);
                self.since_retrieve = 0;
                self.last = Some(RETRIEVE);
                return Some(Move::Retrieve { lemma, meaning });
            }
        }
        let chosen = if self.quiet_turns >= 2 {
            Move::Introduce
        } else if assessment.outcome == Outcome::Success && assessment.suggested_level >= 3 {
            Move::Stretch
        } else {
            Move::Consolidate
        };
        if self.last == Some(chosen.kind()) { return None; }
        // New material resets the count, so the next turn does not introduce again immediately.
        if chosen == Move::Introduce { self.quiet_turns = 0; }
        self.last = Some(chosen.kind());
        Some(chosen)
    }

    /// An error from this turn that the learner has made before.
    fn recurring_hit<'a>(assessment: &Assessment, learner: &'a LearnerState, language_id: &str) -> Option<&'a RecurringError> {
        assessment.errors.iter().filter(|e| e.language == language_id).find_map(|seen| {
            learner.recurring_errors.iter().find(|known| {
                known.label.eq_ignore_ascii_case(&seen.label) && known.correction.eq_ignore_ascii_case(&seen.correction)
            })
        })
    }

    /// The weakest word that is due and has not been set up yet in this conversation.
    fn due_word<'a>(&self, learner: &'a LearnerState, now: Date) -> Option<&'a WordState> {
        learner.words.iter()
            .filter(|w| w.due_at < now && !self.prompted.contains(&w.id) && !w.lemma.is_empty() && !w.meaning.is_empty())
            .min_by(|a, b| a.bars.cmp(&b.bars).then(a.due_at.0.total_cmp(&b.due_at.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::models::{ErrorProposal, WordProposal};

    const NOW: Date = Date(1_000_000.0);

    fn learner(words: Vec<WordState>) -> LearnerState {
        LearnerState { challenge: 2, observation_count: 4, next_goal: "Fortell mer.".into(), capabilities: vec![], words, recurring_errors: vec![], recent_session: None }
    }

    fn word(id: &str, lemma: &str, bars: usize, due: f64) -> WordState {
        WordState {
            id: id.into(), lemma: lemma.into(), meaning: "a walk".into(), form: lemma.into(), example: "vi tok en tur".into(),
            bars, understanding_count: 0, independent_count: 1, last_seen: Date(NOW.0 - 86_400.0), due_at: Date(due),
        }
    }

    fn assessment(outcome: Outcome, level: i64, independent: bool) -> Assessment {
        let words = if independent {
            vec![WordProposal {
                lemma: "å gå".into(), meaning: "to go".into(), form: "gikk".into(), kind: EvidenceKind::Independent,
                confidence: 0.9, source_ids: vec!["f1".into()], quote: "jeg gikk".into(), language: "nb".into(),
            }]
        } else {
            vec![]
        };
        Assessment {
            passage_id: "p1".into(), revision_key: "k1".into(), outcome, suggested_level: level,
            next_goal: String::new(), capability: String::new(), words, errors: vec![], created_at: NOW, context: "free".into(),
        }
    }

    fn next(plan: &mut TeachingPlan, a: &Assessment, learner: &LearnerState) -> Option<Move> {
        plan.next(a, learner, "nb", NOW)
    }

    #[test]
    fn a_repeated_error_is_recast_once_then_left_alone() {
        let mut plan = TeachingPlan::default();
        let mut learner = learner(vec![]);
        learner.recurring_errors = vec![RecurringError { label: "noun gender".into(), correction: "en tur".into(), count: 3 }];
        let mut a = assessment(Outcome::Partial, 2, true);
        a.errors = vec![ErrorProposal { form: "et tur".into(), correction: "en tur".into(), label: "noun gender".into(), language: "nb".into() }];
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Recast { label: "noun gender".into(), correction: "en tur".into() }));
        // Not again on the very next turn, even though the error is still on the list.
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Consolidate));
    }

    #[test]
    fn a_first_time_error_is_not_corrected() {
        let mut plan = TeachingPlan::default();
        let learner = learner(vec![]);
        let mut a = assessment(Outcome::Partial, 2, true);
        a.errors = vec![ErrorProposal { form: "et tur".into(), correction: "en tur".into(), label: "noun gender".into(), language: "nb".into() }];
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Consolidate));
    }

    #[test]
    fn breakdown_repairs_every_time() {
        let mut plan = TeachingPlan::default();
        let learner = learner(vec![]);
        let a = assessment(Outcome::Breakdown, 1, false);
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Repair));
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Repair));
    }

    #[test]
    fn success_stretches_then_stays_quiet() {
        let mut plan = TeachingPlan::default();
        let learner = learner(vec![]);
        let a = assessment(Outcome::Success, 4, true);
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Stretch));
        // The same move twice in a row says nothing new.
        assert_eq!(next(&mut plan, &a, &learner), None);
    }

    #[test]
    fn low_level_success_consolidates() {
        let mut plan = TeachingPlan::default();
        let learner = learner(vec![]);
        assert_eq!(next(&mut plan, &assessment(Outcome::Success, 1, true), &learner), Some(Move::Consolidate));
    }

    #[test]
    fn due_word_is_set_up_once_weakest_first() {
        let mut plan = TeachingPlan::default();
        let learner = learner(vec![word("w-steady", "å gå", 3, NOW.0 - 10.0), word("w-fragile", "en tur", 1, NOW.0 - 5.0)]);
        let a = assessment(Outcome::Success, 2, true);
        // The first turn consolidates; retrieval waits a turn so it does not drill.
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Consolidate));
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Retrieve { lemma: "en tur".into(), meaning: "a walk".into() }));
        // Retrieval leaves a turn of ordinary conversation in between.
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Consolidate));
        // The next one picks a different word, never the one already set up.
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Retrieve { lemma: "å gå".into(), meaning: "a walk".into() }));
    }

    #[test]
    fn words_that_are_not_due_are_left_alone() {
        let mut plan = TeachingPlan::default();
        let learner = learner(vec![word("w1", "en tur", 1, NOW.0 + 86_400.0)]);
        let a = assessment(Outcome::Success, 2, true);
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Consolidate));
        assert_eq!(next(&mut plan, &a, &learner), None);
    }

    #[test]
    fn two_turns_without_independent_evidence_introduce_new_material() {
        let mut plan = TeachingPlan::default();
        let learner = learner(vec![]);
        let a = assessment(Outcome::Partial, 2, false);
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Consolidate));
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Introduce));
        // Introducing resets the count, so the next turn does not introduce again.
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Consolidate));
    }

    #[test]
    fn evidence_in_another_language_does_not_count() {
        let mut plan = TeachingPlan::default();
        let learner = learner(vec![]);
        let mut a = assessment(Outcome::Partial, 2, true);
        a.words[0].language = "en".into();
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Consolidate));
        assert_eq!(next(&mut plan, &a, &learner), Some(Move::Introduce));
    }
}
