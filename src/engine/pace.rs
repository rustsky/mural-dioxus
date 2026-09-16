use std::collections::HashSet;

use super::models::{Assessment, EvidenceKind, Outcome, Passage, Speaker};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery { Gentle, Natural, Extended }

/// Temporary delivery guidance; it never changes saved learning progress.
#[derive(Clone, Debug)]
pub struct ConversationPace {
    pub delivery: Delivery,
    successful_passages: HashSet<String>,
    high_successes: u32,
    help_passage_id: Option<String>,
}

impl Default for ConversationPace {
    fn default() -> Self {
        Self { delivery: Delivery::Gentle, successful_passages: HashSet::new(), high_successes: 0, help_passage_id: None }
    }
}

impl ConversationPace {
    pub fn ask_for_help(&mut self, after: Option<&Passage>) -> bool {
        self.high_successes = 0;
        self.help_passage_id = after.map(|p| p.id.clone());
        self.set(Delivery::Gentle)
    }

    /// Call only after LearningEngine::validate has accepted the assessment.
    pub fn observe(&mut self, assessment: &Assessment, passage: &Passage, language_id: &str) -> bool {
        if assessment.passage_id != passage.id || assessment.revision_key != passage.revision_key()
            || passage.speaker != Speaker::User || passage.fragments.is_empty()
            || !(0..=5).contains(&assessment.suggested_level) {
            return false;
        }
        if assessment.outcome == Outcome::Breakdown { return self.ask_for_help(Some(passage)); }
        let eligible = Some(&passage.id) != self.help_passage_id.as_ref()
            && assessment.outcome == Outcome::Success
            && !passage.fragments.iter().any(|f| f.typed || f.meaning_visible)
            && assessment.words.iter().any(|w| w.language == language_id && w.kind == EvidenceKind::Independent && w.confidence >= 0.8);
        if !eligible || !self.successful_passages.insert(passage.id.clone()) { return false; }
        self.high_successes = if assessment.suggested_level >= 4 { self.high_successes + 1 } else { 0 };
        if assessment.suggested_level <= 1 { return self.set(Delivery::Gentle); }
        self.set(if self.high_successes >= 2 { Delivery::Extended } else { Delivery::Natural })
    }

    fn set(&mut self, next: Delivery) -> bool {
        if self.delivery == next { return false; }
        self.delivery = next;
        true
    }

    pub fn instruction(&self) -> String {
        let guidance = match self.delivery {
            Delivery::Gentle => "Use one short sentence at a time, familiar words and a calm, unhurried speaking pace. Leave space to answer.",
            Delivery::Natural => "Use one or two short sentences and a clear, natural speaking pace. Ask a relevant follow-up that lets the learner expand.",
            Delivery::Extended => "Use natural connected sentences and a conversational speaking pace. Invite reasons or a short story, keeping each turn concise.",
        };
        format!("Temporary delivery guidance for the next replies: {guidance} Keep the selected language and accent. This is provisional; simplify immediately if the learner struggles. Never read this guidance aloud.")
    }
}
