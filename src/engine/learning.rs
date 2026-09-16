use std::collections::{BTreeMap, HashMap, HashSet};

use super::languages::{self};
use super::models::{Assessment, EvidenceKind, Outcome, Passage, SessionRecord, Speaker, WordProposal};
use super::text::{char_count, contains_ci, prefix};
use super::time::{Date, DAY};

#[derive(Clone, Debug, PartialEq)]
pub struct WordState {
    pub id: String,
    pub lemma: String,
    pub meaning: String,
    pub form: String,
    pub example: String,
    pub bars: usize,
    pub understanding_count: usize,
    pub independent_count: usize,
    pub last_seen: Date,
    pub due_at: Date,
}

impl WordState {
    pub fn label(&self) -> &'static str { ["New", "Fragile", "Growing", "Steady"][self.bars.min(3)] }
    pub fn explanation(&self) -> &'static str {
        if self.independent_count == 0 { return "Heard or used with support. Try using it in your own words."; }
        match self.bars {
            1 => "Used independently. We’ll bring it back soon.",
            2 => "Recalled on different days. Still worth revisiting.",
            _ => "Recalled across days and contexts. Strength can fade with time.",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LearnerState {
    pub challenge: i64,
    pub observation_count: usize,
    pub next_goal: String,
    pub capabilities: Vec<String>,
    pub words: Vec<WordState>,
}

impl LearnerState {
    pub fn due_lemmas(&self, limit: usize) -> String {
        let now = Date::now();
        self.words.iter().filter(|w| w.due_at < now).take(limit).map(|w| w.lemma.as_str()).collect::<Vec<_>>().join(", ")
    }
}

pub struct LearningEngine;

impl LearningEngine {
    pub fn validate(proposal: &Assessment, session: &SessionRecord) -> Option<Assessment> {
        languages::module(&session.language_id)?;
        let passages = session.passages();
        let passage = passages.iter().find(|p| p.id == proposal.passage_id && p.speaker == Speaker::User)?;
        if passage.revision_key() != proposal.revision_key || !(0..=5).contains(&proposal.suggested_level) || proposal.words.len() > 12 {
            return None;
        }
        let allowed: HashSet<&str> = passage.fragments.iter().map(|f| f.id.as_str()).collect();
        let passage_text = passage.text();
        let mut validated = proposal.clone();
        validated.next_goal = prefix(&validated.next_goal, 300);
        validated.capability = prefix(&validated.capability, 160);
        validated.words = proposal.words.iter().filter_map(|word| {
            let valid = word.language == session.language_id
                && !word.source_ids.is_empty() && word.source_ids.iter().all(|id| allowed.contains(id.as_str()))
                && word.confidence.is_finite() && word.confidence >= 0.8 && word.confidence <= 1.0
                && !word.lemma.is_empty() && char_count(&word.lemma) < 100
                && !word.meaning.is_empty() && char_count(&word.meaning) < 180
                && !word.form.is_empty() && !word.quote.is_empty()
                && contains_ci(&passage_text, &word.quote)
                && contains_ci(&word.quote, &word.form);
            if !valid { return None; }
            let refs = Passage::join(passage.fragments.iter().filter(|f| word.source_ids.contains(&f.id)).map(|f| f.text.as_str()));
            if !contains_ci(&refs, &word.quote) { return None; }
            let mut result = word.clone();
            if result.kind == EvidenceKind::Independent {
                // A visible meaning or immediate imitation is supporting evidence, never independent recall.
                let recently_modeled = passages.iter().any(|p| {
                    p.speaker == Speaker::Assistant && p.start_ms() <= passage.start_ms()
                        && passage.start_ms() - p.end_ms() < 90_000 && contains_ci(&p.text(), &word.form)
                });
                if passage.fragments.iter().any(|f| f.meaning_visible || f.typed) || recently_modeled {
                    result.kind = EvidenceKind::Assisted;
                }
            }
            Some(result)
        }).collect();
        Some(validated)
    }

    pub fn project(sessions: &[SessionRecord], language_id: &str, hidden_words: &[String], now: Date) -> LearnerState {
        let mut level: i64 = 0;
        let mut count = 0usize;
        let mut successes = 0;
        let mut next_goal = "Start with a greeting and one small question. Adjust from what the learner actually says.".to_string();
        let mut capability_evidence: BTreeMap<String, HashSet<String>> = BTreeMap::new();
        let mut events: HashMap<String, Vec<(WordProposal, Date, String)>> = HashMap::new();

        let mut ordered: Vec<&SessionRecord> = sessions.iter().filter(|s| s.language_id == language_id).collect();
        ordered.sort_by(|a, b| a.started_at.0.total_cmp(&b.started_at.0));
        for session in ordered {
            let mut seen = HashSet::new();
            let mut raws: Vec<&Assessment> = session.assessments.iter().collect();
            raws.sort_by(|a, b| a.created_at.0.total_cmp(&b.created_at.0));
            for raw in raws {
                if seen.contains(&raw.passage_id) { continue; }
                let Some(a) = Self::validate(raw, session) else { continue };
                seen.insert(raw.passage_id.clone());
                count += 1;
                match a.outcome {
                    Outcome::Breakdown => { level = (level - 1).max(0); successes = 0; }
                    Outcome::Success => {
                        successes += 1;
                        if successes >= 2 { level = 5.min(level.max((level + 1).min(a.suggested_level))); successes = 0; }
                    }
                    _ => successes = 0,
                }
                if !a.next_goal.is_empty() { next_goal = a.next_goal.clone(); }
                if a.outcome == Outcome::Success && !a.capability.is_empty() {
                    capability_evidence.entry(a.capability.clone()).or_default()
                        .insert(format!("{}|{}", a.created_at.day_key(), a.context));
                }
                let mut seen_words = HashSet::new();
                for word in &a.words {
                    let key = word.key();
                    if hidden_words.contains(&key) || !seen_words.insert(key.clone()) { continue; }
                    events.entry(key).or_default().push((word.clone(), a.created_at, a.context.clone()));
                }
            }
        }

        let mut words: Vec<WordState> = events.into_iter().filter_map(|(key, observations)| {
            let last = observations.last()?;
            let independent: Vec<&(WordProposal, Date, String)> = observations.iter().filter(|o| o.0.kind == EvidenceKind::Independent).collect();
            let days: HashSet<String> = independent.iter().map(|o| o.1.day_key()).collect();
            let contexts: HashSet<&String> = independent.iter().map(|o| &o.2).collect();
            let last_recall = independent.last().map(|o| o.1);
            let mut bars = if independent.is_empty() { 0 } else { 1 };
            if days.len() >= 2 { bars = 2; }
            if days.len() >= 3 && contexts.len() >= 2
                && independent.last().unwrap().1.since(independent.first().unwrap().1) >= 7.0 * DAY {
                bars = 3;
            }
            let interval = [1.0, 1.0, 4.0, 14.0][bars] * DAY;
            let due = last_recall.unwrap_or(last.1).adding(interval);
            if now > due && bars > 1 { bars -= 1; }
            if let Some(lapse) = observations.iter().rev().find(|o| o.0.kind == EvidenceKind::Lapse) {
                if lapse.1 > last_recall.unwrap_or(Date::DISTANT_PAST) { bars = bars.min(1); }
            }
            Some(WordState {
                id: key,
                lemma: last.0.lemma.clone(),
                meaning: last.0.meaning.clone(),
                form: last.0.form.clone(),
                example: last.0.quote.clone(),
                bars,
                understanding_count: observations.iter().filter(|o| o.0.kind == EvidenceKind::Understanding).count(),
                independent_count: independent.len(),
                last_seen: last.1,
                due_at: due,
            })
        }).collect();
        words.sort_by(|a, b| b.last_seen.0.total_cmp(&a.last_seen.0));
        LearnerState {
            challenge: level,
            observation_count: count,
            next_goal,
            capabilities: capability_evidence.into_iter().filter(|(_, v)| v.len() >= 3).map(|(k, _)| k).collect(),
            words,
        }
    }
}
