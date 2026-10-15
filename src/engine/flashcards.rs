//! Flash cards for the word list: the essentials of Anki's scheduler and nothing more.
//!
//! A new card goes through two short learning steps (1 and 10 minutes) before it graduates to a
//! day. After that each Good multiplies the interval by the card's ease (250% to start), Hard
//! grows it a little and lowers the ease, Easy grows it more and raises the ease, and Again sends
//! the card back to relearn with a lower ease. A late review counts the extra time it held.
//!
//! Cards measure recognition: seeing the word and remembering its meaning. The bars on a word
//! measure spoken recall in conversation and are kept apart.

use serde::{Deserialize, Serialize};

use super::time::{Date, DAY};

const MINUTE: f64 = 60.0;
const LEARNING_STEPS: [f64; 2] = [MINUTE, 10.0 * MINUTE];
/// A forgotten card is seen again after this, then returns to day intervals.
const RELEARNING_STEP: f64 = 10.0 * MINUTE;
const GRADUATING_DAYS: f64 = 1.0;
const EASY_DAYS: f64 = 4.0;
const START_EASE: f64 = 2.5;
const MIN_EASE: f64 = 1.3;
const MAX_DAYS: f64 = 3650.0;
/// New cards introduced per day, so a long list doesn't become a wall of reviews tomorrow.
pub const NEW_PER_DAY: usize = 20;
/// A learning card due within this long is shown now rather than leaving a gap in the session.
const LEARN_AHEAD: f64 = 20.0 * MINUTE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rating { Again, Hard, Good, Easy }

impl Rating {
    pub const ALL: [Rating; 4] = [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy];
    pub fn label(self) -> &'static str {
        match self { Rating::Again => "Again", Rating::Hard => "Hard", Rating::Good => "Good", Rating::Easy => "Easy" }
    }
    /// Number keys 1–4, as in Anki.
    pub fn key(self) -> &'static str {
        match self { Rating::Again => "1", Rating::Hard => "2", Rating::Good => "3", Rating::Easy => "4" }
    }
}

/// Where a card stands. A card with no schedule is new.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    /// Days until the next review once graduated; while relearning, the interval it returns to.
    pub interval: f64,
    pub ease: f64,
    /// The learning step the card is on, or `None` once it reviews in days.
    pub step: Option<usize>,
    pub due: Date,
    pub reviews: u32,
    pub lapses: u32,
    pub first_reviewed: Date,
    pub last_reviewed: Date,
}

impl Schedule {
    pub fn is_learning(&self) -> bool { self.step.is_some() }
}

/// The schedule after answering a card with `rating` at `now`.
pub fn review(previous: Option<&Schedule>, rating: Rating, now: Date) -> Schedule {
    let mut card = previous.cloned().unwrap_or(Schedule {
        interval: 0.0, ease: START_EASE, step: Some(0), due: now, reviews: 0, lapses: 0,
        first_reviewed: now, last_reviewed: now,
    });
    card.reviews += 1;
    card.last_reviewed = now;
    match card.step {
        Some(step) => {
            let relearning = card.interval > 0.0;
            let steps: &[f64] = if relearning { &[RELEARNING_STEP] } else { &LEARNING_STEPS };
            let step = step.min(steps.len() - 1);
            let wait = match rating {
                Rating::Again => { card.step = Some(0); Some(steps[0]) }
                // Between this step and the next, or this step again if it is the last.
                Rating::Hard => Some(steps.get(step + 1).map(|next| (steps[step] + next) / 2.0).unwrap_or(steps[step])),
                Rating::Good if step + 1 < steps.len() => { card.step = Some(step + 1); Some(steps[step + 1]) }
                Rating::Good => None,
                Rating::Easy => None,
            };
            match wait {
                Some(seconds) => card.due = now.adding(seconds),
                None => {
                    card.interval = match (rating, relearning) {
                        (Rating::Easy, false) => EASY_DAYS,
                        (Rating::Easy, true) => card.interval + 1.0,
                        (_, false) => GRADUATING_DAYS,
                        (_, true) => card.interval,
                    };
                    card.step = None;
                    card.due = now.adding(card.interval * DAY);
                }
            }
        }
        None => {
            let late = (now.since(card.due) / DAY).max(0.0);
            let hard = (card.interval * 1.2).max(card.interval + 1.0);
            match rating {
                Rating::Again => {
                    card.lapses += 1;
                    card.ease = (card.ease - 0.2).max(MIN_EASE);
                    // Back to a day once relearnt; the card has to earn its interval again.
                    card.interval = 1.0;
                    card.step = Some(0);
                    card.due = now.adding(RELEARNING_STEP);
                    return card;
                }
                Rating::Hard => {
                    card.ease = (card.ease - 0.15).max(MIN_EASE);
                    card.interval = hard;
                }
                Rating::Good => card.interval = ((card.interval + late / 2.0) * card.ease).max(hard + 1.0),
                Rating::Easy => {
                    let good = ((card.interval + late / 2.0) * card.ease).max(hard + 1.0);
                    card.interval = ((card.interval + late) * card.ease * 1.3).max(good + 1.0);
                    card.ease += 0.15;
                }
            }
            card.interval = card.interval.min(MAX_DAYS).round();
            card.due = now.adding(card.interval * DAY);
        }
    }
    card
}

/// How long until the card would come back, written as on Anki's buttons: 1m, 10m, 3d, 2mo, 1.5y.
pub fn wait_label(seconds: f64) -> String {
    let minutes = seconds / MINUTE;
    let days = seconds / DAY;
    if minutes < 60.0 { format!("{}m", minutes.round().max(1.0)) }
    else if days < 1.0 { format!("{}h", (minutes / 60.0).round()) }
    else if days < 30.0 { format!("{}d", days.round()) }
    else if days < 365.0 { format!("{}mo", (days / 30.0).round()) }
    else { format!("{:.1}y", days / 365.0).replace(".0y", "y") }
}

/// What a session shows, in order: cards due now (learning first, then reviews, oldest due first),
/// then new cards up to today's allowance, then learning cards due within the next few minutes.
/// `cards` lists every card in the order new ones should be introduced.
pub fn queue<'a>(cards: &[(&'a str, Option<&Schedule>)], now: Date) -> Vec<&'a str> {
    let today = now.day_key();
    let introduced_today = cards.iter()
        .filter(|(_, s)| s.is_some_and(|s| s.first_reviewed.day_key() == today))
        .count();
    let mut due: Vec<(&str, &Schedule)> = cards.iter()
        .filter_map(|(id, s)| s.filter(|s| s.due <= now).map(|s| (*id, s)))
        .collect();
    due.sort_by(|a, b| b.1.is_learning().cmp(&a.1.is_learning()).then(a.1.due.0.total_cmp(&b.1.due.0)));
    let new = cards.iter().filter(|(_, s)| s.is_none()).map(|(id, _)| *id)
        .take(NEW_PER_DAY.saturating_sub(introduced_today));
    let mut ahead: Vec<(&str, &Schedule)> = cards.iter()
        .filter_map(|(id, s)| s.filter(|s| s.is_learning() && s.due > now && s.due <= now.adding(LEARN_AHEAD)).map(|s| (*id, s)))
        .collect();
    ahead.sort_by(|a, b| a.1.due.0.total_cmp(&b.1.due.0));
    due.into_iter().map(|(id, _)| id).chain(new).chain(ahead.into_iter().map(|(id, _)| id)).collect()
}

/// When the next card after the queue becomes due, if any.
pub fn next_due(cards: &[(&str, Option<&Schedule>)]) -> Option<Date> {
    cards.iter().filter_map(|(_, s)| s.map(|s| s.due)).min_by(|a, b| a.0.total_cmp(&b.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(days: f64) -> Date { Date(800_000_000.0 + days * DAY) }

    fn wait(card: &Schedule, now: Date) -> f64 { card.due.since(now) }

    #[test]
    fn a_new_card_steps_through_learning_then_graduates_to_a_day() {
        let now = at(0.0);
        let first = review(None, Rating::Good, now);
        assert_eq!(first.step, Some(1));
        assert_eq!(wait(&first, now), 10.0 * MINUTE);
        let graduated = review(Some(&first), Rating::Good, first.due);
        assert_eq!(graduated.step, None);
        assert_eq!(graduated.interval, 1.0);
        assert_eq!(wait(&graduated, first.due), DAY);
    }

    #[test]
    fn again_and_hard_keep_a_new_card_in_minutes() {
        let now = at(0.0);
        assert_eq!(wait(&review(None, Rating::Again, now), now), MINUTE);
        assert_eq!(wait(&review(None, Rating::Hard, now), now), 5.5 * MINUTE);
        let second = review(None, Rating::Good, now);
        assert_eq!(wait(&review(Some(&second), Rating::Hard, now), now), 10.0 * MINUTE, "the last step repeats");
    }

    #[test]
    fn easy_skips_learning() {
        let card = review(None, Rating::Easy, at(0.0));
        assert_eq!((card.step, card.interval), (None, 4.0));
    }

    #[test]
    fn good_multiplies_by_ease_and_hard_and_easy_move_it() {
        let mut card = review(None, Rating::Easy, at(0.0)); // 4 days
        card = review(Some(&card), Rating::Good, card.due);
        assert_eq!(card.interval, 10.0, "4 × 2.5");
        let hard = review(Some(&card), Rating::Hard, card.due);
        assert_eq!(hard.interval, 12.0);
        assert!((hard.ease - 2.35).abs() < 1e-9);
        let easy = review(Some(&card), Rating::Easy, card.due);
        assert_eq!(easy.interval, 33.0, "10 × 2.5 × 1.3, rounded");
        assert!((easy.ease - 2.65).abs() < 1e-9);
    }

    #[test]
    fn forgetting_a_card_relearns_it_and_starts_over_at_a_day() {
        let mut card = review(None, Rating::Easy, at(0.0));
        card = review(Some(&card), Rating::Good, card.due);
        let forgotten = review(Some(&card), Rating::Again, card.due);
        assert_eq!(forgotten.lapses, 1);
        assert!((forgotten.ease - 2.3).abs() < 1e-9);
        assert_eq!(wait(&forgotten, card.due), RELEARNING_STEP);
        let relearnt = review(Some(&forgotten), Rating::Good, forgotten.due);
        assert_eq!((relearnt.step, relearnt.interval), (None, 1.0));
    }

    #[test]
    fn ease_never_drops_below_its_floor() {
        let mut card = review(None, Rating::Easy, at(0.0));
        for _ in 0..20 { card = review(Some(&card), Rating::Hard, card.due); }
        assert_eq!(card.ease, MIN_EASE);
    }

    #[test]
    fn a_late_good_review_counts_the_time_it_held() {
        let card = review(None, Rating::Easy, at(0.0)); // 4 days, due at day 4
        let on_time = review(Some(&card), Rating::Good, at(4.0));
        let late = review(Some(&card), Rating::Good, at(10.0));
        assert!(late.interval > on_time.interval);
    }

    #[test]
    fn the_queue_puts_due_cards_first_and_limits_new_ones() {
        let now = at(10.0);
        let learning = Schedule { interval: 0.0, ease: 2.5, step: Some(1), due: at(9.99), reviews: 1, lapses: 0, first_reviewed: at(9.0), last_reviewed: at(9.0) };
        let review_due = Schedule { step: None, interval: 3.0, due: at(9.0), ..learning.clone() };
        let later = Schedule { due: at(12.0), ..review_due.clone() };
        let ids: Vec<String> = (0..30).map(|i| format!("new{i}")).collect();
        let mut cards: Vec<(&str, Option<&Schedule>)> = vec![("later", Some(&later)), ("review", Some(&review_due)), ("learning", Some(&learning))];
        cards.extend(ids.iter().map(|id| (id.as_str(), None)));
        let q = queue(&cards, now);
        assert_eq!(&q[..3], &["learning", "review", "new0"]);
        assert!(!q.contains(&"later"));
        assert_eq!(q.iter().filter(|id| id.starts_with("new")).count(), NEW_PER_DAY);
    }

    #[test]
    fn new_cards_already_started_today_count_against_the_allowance() {
        let now = at(10.0);
        let started = Schedule { interval: 1.0, ease: 2.5, step: None, due: at(11.0), reviews: 2, lapses: 0, first_reviewed: now, last_reviewed: now };
        let ids: Vec<String> = (0..30).map(|i| format!("new{i}")).collect();
        let mut cards: Vec<(&str, Option<&Schedule>)> = (0..5).map(|_| ("started", Some(&started))).collect();
        cards.extend(ids.iter().map(|id| (id.as_str(), None)));
        assert_eq!(queue(&cards, now).len(), NEW_PER_DAY - 5);
    }

    #[test]
    fn a_learning_card_due_in_a_few_minutes_is_shown_last_instead_of_waiting() {
        let now = at(10.0);
        let soon = Schedule { interval: 0.0, ease: 2.5, step: Some(0), due: now.adding(MINUTE), reviews: 1, lapses: 0, first_reviewed: now, last_reviewed: now };
        assert_eq!(queue(&[("soon", Some(&soon)), ("new", None)], now), vec!["new", "soon"]);
    }

    #[test]
    fn waits_read_like_anki() {
        assert_eq!(wait_label(MINUTE), "1m");
        assert_eq!(wait_label(10.0 * MINUTE), "10m");
        assert_eq!(wait_label(3.0 * 3600.0), "3h");
        assert_eq!(wait_label(4.0 * DAY), "4d");
        assert_eq!(wait_label(60.0 * DAY), "2mo");
        assert_eq!(wait_label(365.0 * DAY), "1y");
        assert_eq!(wait_label(1.5 * 365.0 * DAY), "1.5y");
    }
}
