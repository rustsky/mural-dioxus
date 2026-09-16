/// An unanswered check-in never extends the session. Times use a monotonic clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { Wait, CheckIn, Warning(i64), End }

pub const IDLE_VOICE_SECONDS: f64 = 30.0;

#[derive(Clone, Debug)]
pub struct ConversationActivity {
    quiet_since: f64,
    output_deadline: f64,
    last_input: Option<f64>,
    typing_started: Option<f64>,
    last_typing: Option<f64>,
    busy_started: Option<f64>,
    checked_in: bool,
}

impl ConversationActivity {
    pub const QUIET_SECONDS: f64 = IDLE_VOICE_SECONDS;
    pub const CHECK_IN_SECONDS: f64 = 15.0;
    pub const SPEECH_GRACE_SECONDS: f64 = 15.0;
    pub const TYPING_GRACE_SECONDS: f64 = 60.0;
    pub const RESPONSE_GRACE_SECONDS: f64 = 45.0;

    pub fn new(now: f64) -> Self {
        Self { quiet_since: now, output_deadline: now + 60.0, last_input: None, typing_started: None, last_typing: None, busy_started: None, checked_in: false }
    }
    pub fn learner_engaged(&mut self, now: f64) {
        self.quiet_since = now;
        self.output_deadline = now + 60.0;
        self.checked_in = false;
        self.typing_started = None;
        self.last_typing = None;
        self.last_input = None;
        self.busy_started = None;
    }
    pub fn assistant_active(&mut self, now: f64) {
        if !self.checked_in && now <= self.output_deadline { self.quiet_since = now; }
    }
    pub fn input_active(&mut self, now: f64) { self.last_input = Some(now); }
    pub fn typing(&mut self, now: f64) {
        if self.typing_started.is_none() { self.typing_started = Some(now); }
        self.last_typing = Some(now);
    }
    pub fn tick(&mut self, now: f64, muted: bool, busy: bool) -> Action {
        if busy && self.busy_started.is_none() { self.busy_started = Some(now); }
        if !busy { self.busy_started = None; }
        let recent_input = !muted && self.last_input.map(|t| now - t < 1.5).unwrap_or(false);
        let editing = self.last_typing.map(|t| now - t < 10.0).unwrap_or(false);
        let mut deadline = self.quiet_since + Self::QUIET_SECONDS;
        // Audio levels provide bounded protection for delayed transcripts, never unlimited activity.
        if recent_input { deadline += Self::SPEECH_GRACE_SECONDS; }
        if editing { if let Some(started) = self.typing_started { deadline = deadline.max(started + Self::TYPING_GRACE_SECONDS); } }
        if busy { if let Some(started) = self.busy_started { deadline = deadline.max(started + Self::RESPONSE_GRACE_SECONDS); } }
        if now >= deadline { return Action::End; }
        if deadline - now <= 5.0 { return Action::Warning((deadline - now).ceil() as i64); }
        if !self.checked_in && !muted && !recent_input && !editing && !busy && now - self.quiet_since >= Self::CHECK_IN_SECONDS {
            self.checked_in = true;
            return Action::CheckIn;
        }
        Action::Wait
    }
}
