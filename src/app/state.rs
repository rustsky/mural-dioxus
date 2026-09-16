use std::collections::HashMap;
use std::time::Duration;

use dioxus::core::{spawn_forever, Task};
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::engine::activity::{Action, ConversationActivity};
use crate::engine::languages::{self, ConversationTheme, LanguageModule, MEANING_LANGUAGES};
use crate::engine::learning::LearningEngine;
use crate::engine::models::{Assessment, Fragment, Outcome, Passage, SessionRecord, Speaker, TopicBrief, WordProposal};
use crate::engine::pace::ConversationPace;
use crate::engine::text::{char_count, prefix};
use crate::engine::time::{new_id, uptime, Date};
use crate::engine::{langdetect, meaning_cache_key, teaching, AI_CONSENT_REQUIRED, AI_CONSENT_VERSION};
use crate::services::api::{self, ApiError, ApiResult, Usage};
use crate::services::credentials;
use crate::services::store::LearningStore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState { Idle, Connecting, Active, Closing, Ended, Failed }

#[derive(Clone, Debug, PartialEq)]
pub struct MeaningRequest {
    pub session_id: String,
    pub passage_id: String,
    pub revision_key: String,
    pub text: String,
    pub learning_language_id: String,
    pub meaning_language: String,
}

impl MeaningRequest {
    fn new(session_id: &str, passage: &Passage, learning_language_id: &str, meaning_language: &str) -> Self {
        Self {
            session_id: session_id.into(), passage_id: passage.id.clone(), revision_key: passage.revision_key(),
            text: passage.text(), learning_language_id: learning_language_id.into(), meaning_language: meaning_language.into(),
        }
    }
    pub fn cache_key(&self) -> String { meaning_cache_key(&self.revision_key, &self.meaning_language) }
    fn shares_context(&self, other: &Self) -> bool {
        self.session_id == other.session_id && self.passage_id == other.passage_id
            && self.learning_language_id == other.learning_language_id && self.meaning_language == other.meaning_language
    }
}

/// Keeps one translation in flight while coalescing growing transcript fragments.
#[derive(Default)]
pub struct Meaning {
    pub text: String,
    pub loading: bool,
    pub error: Option<String>,
    desired: Option<MeaningRequest>,
    rendered: Option<MeaningRequest>,
    worker: Option<Task>,
    generation: u64,
}

struct FinalJob { token: String, deadline: Date, request: Task, timer: Task }

pub struct Conv {
    pub state: ConnectionState,
    pub session: Option<SessionRecord>,
    pub selected_theme: Option<ConversationTheme>,
    pub input_level: f64,
    pub output_level: f64,
    pub is_muted: bool,
    pub working: bool,
    pub error: Option<String>,
    pub typed_reply_error: Option<String>,
    pub notice: Option<String>,
    pub inactivity_seconds: Option<i64>,
    pub meaning: Meaning,
    start_after_consent: bool,
    // Transport
    attempt: Option<String>,
    instructions: String,
    channel_open: bool,
    started: bool,
    transport_closing: bool,
    recovery_task: Option<Task>,
    ready_task: Option<Task>,
    // Coordinator tasks
    connection_task: Option<Task>,
    assessment_task: Option<Task>,
    delegation_tasks: HashMap<String, Task>,
    close_task: Option<Task>,
    duration_task: Option<Task>,
    save_task: Option<Task>,
    reset_task: Option<Task>,
    reset_deadline: Option<f64>,
    activity: ConversationActivity,
    pace: ConversationPace,
    last_language_check: String,
    pending_commands: HashMap<String, Date>,
    last_assessment_key: String,
    pending_topic: Option<TopicBrief>,
    language_generation: u64,
    final_jobs: HashMap<String, FinalJob>,
}

impl Default for Conv {
    fn default() -> Self {
        Self {
            state: ConnectionState::Idle, session: None, selected_theme: None, input_level: 0.0, output_level: 0.0,
            is_muted: false, working: false, error: None, typed_reply_error: None, notice: None, inactivity_seconds: None,
            meaning: Meaning::default(), start_after_consent: false,
            attempt: None, instructions: String::new(), channel_open: false, started: false, transport_closing: false,
            recovery_task: None, ready_task: None,
            connection_task: None, assessment_task: None, delegation_tasks: HashMap::new(), close_task: None,
            duration_task: None, save_task: None, reset_task: None, reset_deadline: None,
            activity: ConversationActivity::new(0.0), pace: ConversationPace::default(),
            last_language_check: String::new(), pending_commands: HashMap::new(), last_assessment_key: String::new(),
            pending_topic: None, language_generation: 0, final_jobs: HashMap::new(),
        }
    }
}

impl Conv {
    pub fn is_running(&self) -> bool {
        matches!(self.state, ConnectionState::Active | ConnectionState::Connecting | ConnectionState::Closing)
    }
    pub fn assistant_passage(&self) -> Option<Passage> {
        self.session.as_ref()?.passages().into_iter().rev().find(|p| p.speaker == Speaker::Assistant)
    }
    pub fn user_passage(&self) -> Option<Passage> {
        self.session.as_ref()?.passages().into_iter().rev().find(|p| p.speaker == Speaker::User)
    }
    pub fn caption(&self, language: &LanguageModule) -> String {
        self.assistant_passage().map(|p| p.text()).unwrap_or_else(|| language.greeting.to_string())
    }
    pub fn status(&self) -> String {
        if self.state == ConnectionState::Active {
            if let Some(s) = self.inactivity_seconds { return format!("Ending in {s}s\nReply to continue"); }
        }
        match self.state {
            ConnectionState::Idle => "Ready when you are",
            ConnectionState::Connecting => "Getting comfortable…",
            ConnectionState::Active if self.output_level > 0.02 => "Mural is speaking",
            ConnectionState::Active if self.input_level > 0.02 => "I’m listening",
            ConnectionState::Active => "Take your time",
            ConnectionState::Closing => "Saving our conversation…",
            ConnectionState::Ended => "Until next time",
            ConnectionState::Failed => "Let’s try again",
        }.to_string()
    }
    pub fn microphone_label(&self) -> &'static str {
        match self.state {
            ConnectionState::Active if self.is_muted => "Microphone muted",
            ConnectionState::Active => "Microphone on",
            ConnectionState::Connecting => "Connecting microphone",
            _ => "Microphone off",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab { Talk, Themes, Words }

#[derive(Clone, Debug, PartialEq)]
pub enum Sheet {
    Settings,
    AiConsent,
    TypedReply,
    Transcript,
    Lookup { word: String, sentence: String },
    CurrentTopic,
    WordDetail(String),
    History,
    EditableTranscript(String),
    Notices,
}

pub struct Ui {
    pub tab: Tab,
    pub sheets: Vec<Sheet>,
    pub onboarding: bool,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum BridgeMessage {
    Ready,
    Offer { attempt: String, sdp: String },
    Error { attempt: String, code: String },
    Channel { attempt: String, open: bool },
    Event { attempt: String, data: String },
    Ice { attempt: String, state: String },
    Levels { input: f64, output: f64 },
}

const NETWORK_LOST: &str = "The network connection was lost. Click the microphone to start a new conversation.";
const TIME_LIMIT_NOTICE: &str = "You’ve reached your conversation time limit.";
const QUIET_NOTICE: &str = "Mural ended this quiet session to avoid running up usage.";

async fn sleep(seconds: f64) { tokio::time::sleep(Duration::from_secs_f64(seconds)).await }

/// The app's single controller: a copyable handle over reactive state, like ConversationCoordinator on iPhone.
#[derive(Clone, Copy, PartialEq)]
pub struct Mural {
    pub conv: Signal<Conv>,
    pub store: Signal<LearningStore>,
    pub ui: Signal<Ui>,
    bridge: Signal<Option<document::Eval>>,
}

impl Mural {
    pub fn new(store: LearningStore) -> Self {
        let onboarding = !store.preferences().has_onboarded;
        // Owned by the root scope: background tasks run there too.
        let root = dioxus::core::ScopeId::ROOT;
        Self {
            conv: Signal::new_in_scope(Conv::default(), root),
            store: Signal::new_in_scope(store, root),
            ui: Signal::new_in_scope(Ui { tab: Tab::Talk, sheets: vec![], onboarding }, root),
            bridge: Signal::new_in_scope(None, root),
        }
    }

    // ---------- Bridge ----------

    pub fn attach_bridge(self) {
        let mut eval = document::eval(include_str!("../bridge.js"));
        let bridge = self.bridge;
        *bridge.write_unchecked() = Some(eval);
        #[cfg(debug_assertions)]
        if std::env::var("MURAL_FAKE_MICROPHONE").is_ok() {
            let _ = eval.send(json!({"cmd": "config", "fakeMicrophone": true}));
        }
        spawn_forever(async move {
            loop {
                match eval.recv::<Value>().await {
                    Ok(value) => {
                        #[cfg(debug_assertions)]
                        if std::env::var("MURAL_TRACE").is_ok() && value["kind"] != "levels" {
                            eprintln!("bridge <- {}", crate::engine::text::prefix(&value.to_string(), 300));
                        }
                        match serde_json::from_value::<BridgeMessage>(value) {
                            Ok(message) => self.on_bridge(message),
                            Err(error) => eprintln!("Unrecognized bridge message: {error}"),
                        }
                    }
                    Err(dioxus::document::EvalError::Serialization(_)) => continue,
                    Err(_) => break,
                }
            }
        });
    }

    fn bridge_send(self, value: Value) {
        if let Some(eval) = *self.bridge.peek() { let _ = eval.send(value); }
    }

    fn current_attempt(self, attempt: &str) -> bool {
        self.conv.peek().attempt.as_deref() == Some(attempt)
    }

    fn on_bridge(self, message: BridgeMessage) {
        let conv = self.conv;
        match message {
            BridgeMessage::Ready => {}
            BridgeMessage::Offer { attempt, sdp } => {
                if !self.current_attempt(&attempt) || conv.peek().state != ConnectionState::Connecting { return; }
                let instructions = conv.peek().instructions.clone();
                let task = spawn_forever(async move {
                    let result = api::create_live_session(&instructions, &sdp).await;
                    if !self.current_attempt(&attempt) { return; }
                    match result {
                        Ok((answer, session)) => {
                            if let Some(session) = session {
                                self.handle(json!({"type": "mural.session.created", "session": session}));
                            }
                            self.bridge_send(json!({"cmd": "answer", "attempt": attempt, "sdp": answer}));
                            let ready = spawn_forever(async move {
                                sleep(20.0).await;
                                if self.current_attempt(&attempt) && !self.conv.peek().started {
                                    self.connection_failed("The voice connection took too long. Please try again.".into());
                                }
                            });
                            self.conv.write_unchecked().ready_task = Some(ready);
                        }
                        Err(error) => self.connection_failed(error.to_string()),
                    }
                });
                conv.write_unchecked().connection_task = Some(task);
            }
            BridgeMessage::Error { attempt, code } => {
                if !self.current_attempt(&attempt) { return; }
                let message = match code.as_str() {
                    "microphone" => "Allow microphone access in System Settings → Privacy & Security → Microphone → Mural to start a conversation.",
                    "timeout" => "The voice connection took too long. Please try again.",
                    "unsupported" => "Voice needs microphone access, which is only available when Mural runs as an app bundle. Open Mural.app and try again.",
                    _ => "The voice connection couldn’t be established. Check your connection and try again.",
                };
                self.connection_failed(message.into());
            }
            BridgeMessage::Channel { attempt, open } => {
                if !self.current_attempt(&attempt) { return; }
                conv.write_unchecked().channel_open = open;
                if !open && !conv.peek().transport_closing {
                    self.fail("The voice connection ended unexpectedly. Your conversation has been saved.".into());
                }
            }
            BridgeMessage::Event { attempt, data } => {
                if !self.current_attempt(&attempt) { return; }
                let Ok(json) = serde_json::from_str::<Value>(&data) else { return };
                if json["type"].as_str() == Some("session.started") {
                    let mut c = conv.write_unchecked();
                    c.started = true;
                    if let Some(t) = c.ready_task.take() { t.cancel(); }
                }
                self.handle(json);
            }
            BridgeMessage::Ice { attempt, state } => {
                if !self.current_attempt(&attempt) || conv.peek().transport_closing { return; }
                match state.as_str() {
                    "disconnected" => {
                        if conv.peek().recovery_task.is_some() { return; }
                        // Gives a brief network handoff time to recover without leaving a dead call active.
                        let task = spawn_forever(async move {
                            sleep(8.0).await;
                            let alive = { let c = self.conv.peek(); c.attempt.is_some() && !c.transport_closing };
                            self.conv.write_unchecked().recovery_task = None;
                            if alive { self.fail(NETWORK_LOST.into()); }
                        });
                        conv.write_unchecked().recovery_task = Some(task);
                    }
                    "connected" | "completed" => self.cancel_recovery(),
                    "failed" => { self.cancel_recovery(); self.fail(NETWORK_LOST.into()); }
                    _ => {}
                }
            }
            BridgeMessage::Levels { input, output } => {
                let mut c = conv.write_unchecked();
                if (c.input_level - input).abs() < 0.004 && (c.output_level - output).abs() < 0.004 { return; }
                c.input_level = input;
                c.output_level = output;
                if c.state == ConnectionState::Active {
                    let now = uptime();
                    if input > 0.03 { c.activity.input_active(now); }
                    if output > 0.03 { c.activity.assistant_active(now); }
                }
            }
        }
    }

    fn connection_failed(self, message: String) {
        let state = self.conv.peek().state;
        if matches!(state, ConnectionState::Connecting | ConnectionState::Active) { self.fail(message); }
    }

    fn cancel_recovery(self) {
        if let Some(t) = self.conv.write_unchecked().recovery_task.take() { t.cancel(); }
    }

    fn transport_send(self, event: Value) -> bool {
        let attempt = { let c = self.conv.peek(); if !c.channel_open { return false; } c.attempt.clone() };
        let Some(attempt) = attempt else { return false };
        self.bridge_send(json!({"cmd": "send", "attempt": attempt, "data": event.to_string()}));
        true
    }

    fn transport_connect(self, instructions: String) {
        self.transport_disconnect();
        let attempt = new_id();
        {
            let mut c = self.conv.write_unchecked();
            c.attempt = Some(attempt.clone());
            c.instructions = instructions;
            c.channel_open = false;
            c.started = false;
            c.transport_closing = false;
        }
        self.bridge_send(json!({"cmd": "connect", "attempt": attempt}));
    }

    fn transport_close(self) {
        self.cancel_recovery();
        self.transport_send(json!({"type": "session.close", "event_id": new_id()}));
        self.conv.write_unchecked().transport_closing = true;
        self.bridge_send(json!({"cmd": "mute", "muted": true}));
    }

    fn transport_disconnect(self) {
        self.cancel_recovery();
        {
            let mut c = self.conv.write_unchecked();
            c.attempt = None;
            c.started = false;
            c.channel_open = false;
            c.transport_closing = true;
            if let Some(t) = c.ready_task.take() { t.cancel(); }
            c.input_level = 0.0;
            c.output_level = 0.0;
        }
        self.bridge_send(json!({"cmd": "disconnect"}));
    }

    // ---------- Derived ----------

    pub fn language(self) -> &'static LanguageModule { self.store.peek().language() }

    fn has_ai_consent(self) -> bool {
        self.store.peek().preferences().ai_consent_version == Some(AI_CONSENT_VERSION)
    }

    pub fn open_sheet(self, sheet: Sheet) {
        let ui = self.ui;
        let mut u = ui.write_unchecked();
        if !u.sheets.contains(&sheet) { u.sheets.push(sheet); }
    }

    pub fn close_sheet(self) {
        let ui = self.ui;
        let closed = ui.write_unchecked().sheets.pop();
        if closed == Some(Sheet::AiConsent) { self.resume_after_ai_consent(); }
        if closed == Some(Sheet::TypedReply) { self.conv.write_unchecked().typed_reply_error = None; }
    }

    // ---------- Conversation lifecycle ----------

    pub fn start(self) {
        let conv = self.conv;
        if conv.peek().is_running() { return; }
        if !self.has_ai_consent() {
            conv.write_unchecked().start_after_consent = true;
            self.open_sheet(Sheet::AiConsent);
            return;
        }
        if !credentials::has_key() { self.open_sheet(Sheet::Settings); return; }
        self.cancel_reset();
        self.meaning_reset();
        let language = self.language();
        let (theme, topic) = { let c = conv.peek(); (c.selected_theme.clone(), c.pending_topic.clone()) };
        let mut record = SessionRecord::new(language.id, theme.as_ref().map(|t| t.id.clone()), theme.as_ref().map(|t| t.title.clone()));
        if let Some(topic) = topic { record.topics = vec![topic]; }
        {
            let mut c = conv.write_unchecked();
            c.error = None;
            c.notice = None;
            c.last_assessment_key.clear();
            c.last_language_check.clear();
            c.pending_commands.clear();
            c.state = ConnectionState::Connecting;
            c.is_muted = false;
            c.session = Some(record.clone());
        }
        self.store.write_unchecked().save(&record);
        // Each new conversation starts fresh; learned vocabulary and difficulty still carry forward.
        let (learner, interests, meaning_language) = {
            let s = self.store.peek();
            (s.learner(), s.preferences().interests.clone(), s.preferences().meaning_language.clone())
        };
        let instructions = teaching::voice(language, &learner, theme.as_ref(), &interests, &meaning_language);
        self.transport_connect(instructions);
    }

    pub fn accept_ai_consent(self) {
        self.store.write_unchecked().update_preferences(|p| p.ai_consent_version = Some(AI_CONSENT_VERSION));
        self.close_sheet();
    }

    pub fn decline_ai_consent(self) {
        self.conv.write_unchecked().start_after_consent = false;
        self.close_sheet();
    }

    fn resume_after_ai_consent(self) {
        let conv = self.conv;
        if !conv.peek().start_after_consent { return; }
        conv.write_unchecked().start_after_consent = false;
        if self.has_ai_consent() { self.start(); }
    }

    pub fn select_language(self, id: &str) {
        let conv = self.conv;
        if conv.peek().is_running() || id == self.language().id || languages::module(id).is_none() { return; }
        self.cancel_reset();
        self.meaning_reset();
        {
            let mut c = conv.write_unchecked();
            c.language_generation += 1;
            for t in [c.connection_task.take(), c.close_task.take(), c.duration_task.take(), c.assessment_task.take(), c.save_task.take()].into_iter().flatten() {
                t.cancel();
            }
            for (_, t) in c.delegation_tasks.drain() { t.cancel(); }
            c.session = None;
            c.selected_theme = None;
            c.pending_topic = None;
            c.working = false;
            c.notice = None;
            c.error = None;
            c.last_assessment_key.clear();
            c.last_language_check.clear();
            c.pending_commands.clear();
            c.input_level = 0.0;
            c.output_level = 0.0;
            c.state = ConnectionState::Idle;
            c.is_muted = false;
        }
        self.store.write_unchecked().select_language(id);
    }

    pub fn select_meaning_language(self, value: &str) {
        if !MEANING_LANGUAGES.contains(&value) { return; }
        self.meaning_reset();
        let value = value.to_string();
        self.store.write_unchecked().update_preferences(|p| p.meaning_language = value);
        self.schedule_translation();
    }

    pub fn choose_theme(self, theme: Option<ConversationTheme>) {
        let conv = self.conv;
        let (running, has_session) = { let c = conv.peek(); (c.is_running(), c.session.is_some()) };
        if !running && has_session { self.reset_conversation(); }
        {
            let mut c = conv.write_unchecked();
            if theme.as_ref().map(|t| t.id.as_str()) != Some("current") { c.pending_topic = None; }
            c.selected_theme = theme.clone();
        }
        if conv.peek().state == ConnectionState::Active {
            let language = self.language();
            {
                let mut c = conv.write_unchecked();
                if let Some(s) = c.session.as_mut() {
                    s.theme_id = theme.as_ref().map(|t| t.id.clone());
                    s.title = theme.as_ref().map(|t| t.title.clone()).unwrap_or_else(|| language.default_title());
                }
            }
            self.append("instructions", &teaching::theme(theme.as_ref(), language), None);
            self.save();
        }
    }

    pub fn toggle_mute(self) {
        let conv = self.conv;
        if conv.peek().state != ConnectionState::Active { return; }
        let muted = { let mut c = conv.write_unchecked(); c.is_muted = !c.is_muted; c.is_muted };
        self.bridge_send(json!({"cmd": "mute", "muted": muted}));
        let kind = if muted { "session.input_audio.mute" } else { "session.input_audio.unmute" };
        self.transport_send(json!({"type": kind, "event_id": new_id()}));
    }

    pub fn delete_learning_data(self) {
        if self.conv.peek().is_running() { return; }
        self.meaning_reset();
        {
            let mut c = self.conv.write_unchecked();
            if let Some(t) = c.assessment_task.take() { t.cancel(); }
            if let Some(t) = c.save_task.take() { t.cancel(); }
        }
        self.reset_conversation();
        let ids = self.store.write_unchecked().delete_all();
        for id in ids { self.cancel_final_assessment(&id); }
    }

    pub fn delete_session(self, id: &str) {
        self.cancel_final_assessment(id);
        self.store.write_unchecked().delete_session(id);
    }

    pub fn correct_passage(self, session_id: &str, passage_id: &str, text: &str) {
        if self.store.write_unchecked().correct_passage(session_id, passage_id, text) {
            self.cancel_final_assessment(session_id);
        }
    }

    pub fn toggle_meaning(self) {
        self.store.write_unchecked().update_preferences(|p| p.meaning_visible = !p.meaning_visible);
        if self.store.peek().preferences().meaning_visible { self.schedule_translation(); } else { self.meaning_reset(); }
    }

    pub fn help(self) {
        let conv = self.conv;
        if conv.peek().state != ConnectionState::Active { return; }
        let user = conv.peek().user_passage();
        let instruction = {
            let mut c = conv.write_unchecked();
            c.activity.learner_engaged(uptime());
            c.inactivity_seconds = None;
            c.pace.ask_for_help(user.as_ref());
            c.pace.instruction()
        };
        let language = self.language();
        self.append("instructions", &instruction, None);
        self.append("instructions", &teaching::help(language), None);
        conv.write_unchecked().notice = Some("Mural will make that a little simpler.".into());
    }

    pub fn end(self, reason: &str) {
        let conv = self.conv;
        let state = conv.peek().state;
        if !matches!(state, ConnectionState::Active | ConnectionState::Connecting) { return; }
        let was_connecting = state == ConnectionState::Connecting;
        {
            let mut c = conv.write_unchecked();
            c.state = ConnectionState::Closing;
            c.is_muted = true;
            for t in [c.connection_task.take(), c.assessment_task.take(), c.duration_task.take()].into_iter().flatten() { t.cancel(); }
            for (_, t) in c.delegation_tasks.drain() { t.cancel(); }
            c.working = false;
            if let Some(s) = c.session.as_mut() { s.end_reason = Some(reason.into()); }
        }
        if was_connecting { self.finish(false); return; }
        self.transport_close();
        let task = spawn_forever(async move {
            sleep(5.0).await;
            if self.conv.peek().state == ConnectionState::Closing { self.finish(false); }
        });
        conv.write_unchecked().close_task = Some(task);
    }

    fn finish(self, final_usage: bool) {
        let conv = self.conv;
        if !conv.peek().is_running() { return; }
        {
            let mut c = conv.write_unchecked();
            for t in [c.close_task.take(), c.duration_task.take(), c.connection_task.take(), c.assessment_task.take(), c.save_task.take()].into_iter().flatten() {
                t.cancel();
            }
            for (_, t) in c.delegation_tasks.drain() { t.cancel(); }
        }
        self.transport_disconnect();
        {
            let mut c = conv.write_unchecked();
            c.pending_commands.clear();
            c.working = false;
            if let Some(s) = c.session.as_mut() {
                s.ended_at = Some(Date::now());
                s.usage_final = final_usage;
            }
        }
        self.save();
        conv.write_unchecked().state = ConnectionState::Ended;
        let ended = conv.peek().session.clone();
        if let Some(session) = ended { self.submit_final_assessment(session); }
        self.schedule_translation();
        self.schedule_reset();
        let mut c = conv.write_unchecked();
        let keep = matches!(c.notice.as_deref(), Some(TIME_LIMIT_NOTICE) | Some(QUIET_NOTICE));
        if !keep {
            let unconfirmed = !final_usage && c.session.as_ref().map(|s| s.provider_id.is_some()).unwrap_or(false);
            c.notice = unconfirmed.then(|| "Conversation saved. Final voice usage is unconfirmed.".to_string());
        }
    }

    fn fail(self, message: String) {
        {
            let conv = self.conv;
            let mut c = conv.write_unchecked();
            c.error = Some(message);
            if let Some(s) = c.session.as_mut() { s.end_reason = Some("Connection failed".into()); }
        }
        self.finish(false);
        self.cancel_reset();
        let conv = self.conv;
        conv.write_unchecked().state = ConnectionState::Failed;
    }

    fn save(self) {
        let session = self.conv.peek().session.clone();
        if let Some(session) = session { self.store.write_unchecked().save(&session); }
    }

    fn schedule_save(self) {
        let conv = self.conv;
        if conv.peek().save_task.is_some() { return; }
        let task = spawn_forever(async move {
            sleep(0.75).await;
            self.save();
            self.conv.write_unchecked().save_task = None;
        });
        conv.write_unchecked().save_task = Some(task);
    }

    fn append(self, kind: &str, text: &str, delegation_id: Option<&str>) -> bool {
        let conv = self.conv;
        if conv.peek().state != ConnectionState::Active { return false; }
        let id = new_id();
        // Bound short instruction updates conservatively below the protocol token cap.
        let accepted = self.transport_send(json!({
            "type": format!("session.{kind}.append"), "event_id": id,
            "delegation_id": delegation_id, "content": prefix(text, 1000),
        }));
        let mut c = conv.write_unchecked();
        if accepted { c.pending_commands.insert(id, Date::now()); }
        else { c.notice = Some("A conversation update couldn’t be sent. You can keep speaking.".into()); }
        accepted
    }

    fn handle(self, event: Value) {
        let conv = self.conv;
        let Some(kind) = event["type"].as_str() else { return };
        if conv.peek().session.is_none() { return; }
        let state = conv.peek().state;
        match kind {
            "mural.session.created" => {
                {
                    let mut c = conv.write_unchecked();
                    if let Some(s) = c.session.as_mut() {
                        s.provider_id = event["session"]["id"].as_str().map(String::from);
                        s.voice_seconds = 15.0;
                    }
                }
                self.save();
            }
            "session.started" => {
                if state != ConnectionState::Connecting { return; }
                {
                    let mut c = conv.write_unchecked();
                    c.state = ConnectionState::Active;
                    c.activity = ConversationActivity::new(uptime());
                    c.pace = ConversationPace::default();
                    c.inactivity_seconds = None;
                    if let Some(s) = c.session.as_mut() { s.provider_id = event["session"]["id"].as_str().map(String::from); }
                }
                self.append("instructions", &teaching::greeting(self.language()), None);
                self.start_duration_checks();
                self.save();
            }
            "session.input_transcript.delta" | "session.output_transcript.delta" => {
                if !matches!(state, ConnectionState::Active | ConnectionState::Closing) { return; }
                let (Some(delta), Some(start), Some(end)) = (event["delta"].as_str(), event["start_ms"].as_i64(), event["end_ms"].as_i64()) else { return };
                if start < 0 || end < start { return; }
                let speaker = if kind == "session.input_transcript.delta" { Speaker::User } else { Speaker::Assistant };
                let mut fragment = Fragment::new(speaker, delta, start, end);
                if let Some(id) = event["event_id"].as_str() { fragment.id = id.into(); }
                fragment.meaning_visible = self.store.peek().preferences().meaning_visible;
                {
                    let mut c = conv.write_unchecked();
                    if let Some(s) = c.session.as_mut() { s.append(fragment); }
                    if !delta.trim().is_empty() {
                        let now = uptime();
                        if speaker == Speaker::User { c.activity.learner_engaged(now); c.inactivity_seconds = None; }
                        else { c.activity.assistant_active(now); }
                    }
                }
                self.schedule_save();
                if speaker == Speaker::Assistant {
                    self.schedule_translation();
                    if state == ConnectionState::Active { self.check_language(); }
                } else if state == ConnectionState::Active {
                    self.schedule_assessment();
                }
            }
            "session.delegation.created" => {
                if state != ConnectionState::Active { return; }
                let d = &event["delegation"];
                if d["target"].as_str() != Some("client") { return; }
                if let Some(id) = d["id"].as_str() { self.delegate(id.to_string()); }
            }
            "session.usage.updated" | "session.closed" => {
                if let Some(seconds) = event["usage"]["seconds"].as_f64() {
                    if seconds.is_finite() && seconds >= 0.0 {
                        if let Some(s) = conv.write_unchecked().session.as_mut() { s.voice_seconds = seconds; }
                    }
                }
                if kind == "session.closed" {
                    if let Some(s) = conv.write_unchecked().session.as_mut() { s.end_reason = event["reason"].as_str().map(String::from); }
                    self.finish(true);
                } else {
                    self.schedule_save();
                }
            }
            "error" => {
                let mut c = conv.write_unchecked();
                if let Some(id) = event["error"]["client_event_id"].as_str() { c.pending_commands.remove(id); }
                c.notice = Some("A voice update was rejected. If Mural stops responding, end this conversation and start again.".into());
            }
            other => {
                if other.ends_with(".appended") {
                    if let Some(id) = event["client_event_id"].as_str() { conv.write_unchecked().pending_commands.remove(id); }
                }
            }
        }
    }

    fn start_duration_checks(self) {
        let conv = self.conv;
        if let Some(t) = conv.write_unchecked().duration_task.take() { t.cancel(); }
        let task = spawn_forever(async move {
            loop {
                sleep(1.0).await;
                let conv = self.conv;
                let (started_at, busy, muted) = {
                    let c = conv.peek();
                    let Some(session) = c.session.as_ref() else { return };
                    if c.state != ConnectionState::Active { return; }
                    (session.started_at, c.working || !c.delegation_tasks.is_empty(), c.is_muted)
                };
                let minutes = self.store.peek().preferences().session_minutes;
                if Date::now().since(started_at) > (minutes * 60) as f64 {
                    conv.write_unchecked().notice = Some(TIME_LIMIT_NOTICE.into());
                    self.end("Time limit");
                    return;
                }
                let action = {
                    let mut c = conv.write_unchecked();
                    c.inactivity_seconds = None;
                    let now = Date::now();
                    c.pending_commands.retain(|_, sent| now.since(*sent) <= 20.0);
                    c.activity.tick(uptime(), muted, busy)
                };
                match action {
                    Action::CheckIn => { self.append("instructions", &teaching::check_in(self.language()), None); }
                    Action::Warning(seconds) => conv.write_unchecked().inactivity_seconds = Some(seconds),
                    Action::End => {
                        conv.write_unchecked().notice = Some(QUIET_NOTICE.into());
                        self.end("Inactivity");
                        return;
                    }
                    Action::Wait => {}
                }
            }
        });
        conv.write_unchecked().duration_task = Some(task);
    }

    // ---------- Meanings ----------

    fn schedule_translation(self) {
        let (visible, meaning_language) = {
            let s = self.store.peek();
            (s.preferences().meaning_visible, s.preferences().meaning_language.clone())
        };
        if !visible { return; }
        let request = {
            let c = self.conv.peek();
            let (Some(session), Some(passage)) = (c.session.as_ref(), c.assistant_passage()) else { return };
            let request = MeaningRequest::new(&session.id, &passage, &session.language_id, &meaning_language);
            let cached = session.translations.get(&request.cache_key()).cloned();
            (request, cached)
        };
        self.meaning_update(request.0, request.1);
    }

    pub fn retry_meaning(self) {
        self.schedule_translation();
        let conv = self.conv;
        if conv.peek().meaning.desired.is_none() { return; }
        self.meaning_cancel_worker();
        conv.write_unchecked().meaning.error = None;
        self.meaning_begin();
    }

    fn meaning_update(self, request: MeaningRequest, cached: Option<String>) {
        let conv = self.conv;
        let changed = conv.peek().meaning.desired.as_ref().map(|d| !d.shares_context(&request)).unwrap_or(true);
        if changed { self.meaning_reset(); }
        conv.write_unchecked().meaning.desired = Some(request.clone());
        if let Some(cached) = cached.filter(|c| !c.is_empty()) {
            self.meaning_cancel_worker();
            let mut c = conv.write_unchecked();
            c.meaning.text = cached;
            c.meaning.rendered = Some(request);
            c.meaning.error = None;
            return;
        }
        let should_begin = {
            let mut c = conv.write_unchecked();
            let m = &mut c.meaning;
            if m.rendered.as_ref() == Some(&request) { return; }
            // Do not display a translation of text that was subsequently corrected.
            if let Some(rendered) = &m.rendered {
                if !request.text.starts_with(&rendered.text) { m.text.clear(); m.rendered = None; }
            }
            m.worker.is_none() && m.error.is_none()
        };
        if should_begin { self.meaning_begin(); }
    }

    fn meaning_reset(self) {
        self.meaning_cancel_worker();
        let conv = self.conv;
        let mut c = conv.write_unchecked();
        c.meaning.desired = None;
        c.meaning.rendered = None;
        c.meaning.text.clear();
        c.meaning.error = None;
    }

    fn meaning_cancel_worker(self) {
        let conv = self.conv;
        let mut c = conv.write_unchecked();
        c.meaning.generation += 1;
        if let Some(t) = c.meaning.worker.take() { t.cancel(); }
        c.meaning.loading = false;
    }

    fn meaning_begin(self) {
        let conv = self.conv;
        let token = {
            let mut c = conv.write_unchecked();
            if c.meaning.desired.is_none() || c.meaning.worker.is_some() { return; }
            c.meaning.loading = true;
            c.meaning.generation
        };
        let task = spawn_forever(async move {
            let conv = self.conv;
            sleep(0.45).await;
            let Some(request) = ({ let c = conv.peek(); (c.meaning.generation == token).then(|| c.meaning.desired.clone()).flatten() }) else { return };
            let result: Result<ApiResult, String> = if !self.has_ai_consent() {
                Err(AI_CONSENT_REQUIRED.into())
            } else if let Some(language) = languages::module(&request.learning_language_id) {
                api::respond(&teaching::translation(language, &request.meaning_language), &request.text, None, false).await.map_err(|e| e.to_string())
            } else {
                Err(crate::engine::models::ArchiveError::UnsupportedLanguage.to_string())
            };
            if conv.peek().meaning.generation != token { return; }
            match result {
                Ok(result) if !result.text.trim().is_empty() => {
                    // Record the translation on the conversation it belongs to.
                    let saved = {
                        let mut c = conv.write_unchecked();
                        match c.session.as_mut() {
                            Some(s) if s.id == request.session_id => {
                                s.translations.insert(request.cache_key(), result.text.clone());
                                s.input_tokens += result.usage.input;
                                s.output_tokens += result.usage.output;
                                true
                            }
                            _ => false,
                        }
                    };
                    if saved { self.save(); }
                    let again = {
                        let mut c = conv.write_unchecked();
                        let m = &mut c.meaning;
                        let latest = m.desired.clone();
                        if let Some(latest) = &latest {
                            if latest.shares_context(&request) && latest.text.starts_with(&request.text) {
                                m.text = result.text.clone();
                                m.rendered = Some(request.clone());
                            }
                        }
                        m.worker = None;
                        m.loading = false;
                        latest.as_ref() != Some(&request)
                    };
                    if again { self.meaning_begin(); }
                }
                other => {
                    let message = other.err().unwrap_or_else(|| "The translation came back empty. Please try again.".into());
                    let mut c = conv.write_unchecked();
                    let m = &mut c.meaning;
                    m.worker = None;
                    m.loading = false;
                    if m.rendered != m.desired { m.text.clear(); }
                    m.error = Some(message);
                }
            }
        });
        let mut c = conv.write_unchecked();
        if c.meaning.generation == token && c.meaning.loading { c.meaning.worker = Some(task); }
    }

    // ---------- Reset ----------

    pub fn reset_conversation(self) {
        let conv = self.conv;
        if conv.peek().is_running() { return; }
        self.cancel_reset();
        self.meaning_reset();
        let mut c = conv.write_unchecked();
        if let Some(t) = c.save_task.take() { t.cancel(); }
        c.language_generation += 1;
        c.session = None;
        c.selected_theme = None;
        c.pending_topic = None;
        c.notice = None;
        c.error = None;
        c.working = false;
        c.is_muted = false;
        c.input_level = 0.0;
        c.output_level = 0.0;
        c.state = ConnectionState::Idle;
    }

    fn cancel_reset(self) {
        let conv = self.conv;
        let mut c = conv.write_unchecked();
        if let Some(t) = c.reset_task.take() { t.cancel(); }
        c.reset_deadline = None;
    }

    fn schedule_reset(self) {
        self.cancel_reset();
        let conv = self.conv;
        let Some(session_id) = conv.peek().session.as_ref().map(|s| s.id.clone()) else { return };
        let task = spawn_forever(async move {
            sleep(15.0).await;
            let matches = { let c = self.conv.peek(); c.state == ConnectionState::Ended && c.session.as_ref().map(|s| s.id == session_id).unwrap_or(false) };
            if matches { self.reset_conversation(); }
        });
        let mut c = conv.write_unchecked();
        c.reset_task = Some(task);
        c.reset_deadline = Some(uptime() + 15.0);
    }

    /// Called when the window regains focus after the reset deadline passed.
    pub fn resume(self) {
        let expired = { let c = self.conv.peek(); c.state == ConnectionState::Ended && c.reset_deadline.map(|d| uptime() >= d).unwrap_or(false) };
        if expired { self.reset_conversation(); }
    }

    // ---------- Assessment ----------

    async fn assess(snapshot: SessionRecord, passage: Passage) -> Result<(Assessment, Usage), String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AssessmentResult { outcome: Outcome, suggested_level: i64, next_goal: String, capability: String, words: Vec<WordProposal> }
        let language = languages::module(&snapshot.language_id).ok_or("unsupported language")?;
        let result = api::respond(&teaching::assessment(language), &teaching::context(&snapshot, Some(&passage)), Some(api::assessment_schema(language)), false)
            .await.map_err(|e| e.to_string())?;
        let decoded: AssessmentResult = serde_json::from_str(&result.text).map_err(|e| e.to_string())?;
        let proposed = Assessment {
            passage_id: passage.id.clone(), revision_key: passage.revision_key(), outcome: decoded.outcome,
            suggested_level: decoded.suggested_level, next_goal: decoded.next_goal, capability: decoded.capability,
            words: decoded.words, created_at: Date::now(), context: snapshot.theme_id.clone().unwrap_or_else(|| "free".into()),
        };
        Ok((proposed, result.usage))
    }

    fn schedule_assessment(self) {
        let conv = self.conv;
        if let Some(t) = conv.write_unchecked().assessment_task.take() { t.cancel(); }
        let task = spawn_forever(async move {
            sleep(3.0).await;
            let conv = self.conv;
            let (snapshot, passage) = {
                let c = conv.peek();
                let Some(snapshot) = c.session.clone() else { return };
                let Some(p) = c.user_passage() else { return };
                if char_count(&p.text()) < 3 || p.revision_key() == c.last_assessment_key || c.state != ConnectionState::Active { return; }
                (snapshot, p)
            };
            let Some(target) = languages::module(&snapshot.language_id) else { return };
            // The passage remains saved without unverified learning evidence.
            let Ok((assessment, usage)) = Self::assess(snapshot.clone(), passage.clone()).await else { return };
            let validated = {
                let c = conv.peek();
                let same = c.state == ConnectionState::Active && c.session.as_ref().map(|s| s.id == snapshot.id).unwrap_or(false)
                    && c.user_passage().map(|p| p.revision_key()) == Some(passage.revision_key());
                if !same { return; }
                let Some(current) = c.session.as_ref() else { return };
                let Some(v) = LearningEngine::validate(&assessment, current) else { return };
                v
            };
            {
                let mut c = conv.write_unchecked();
                if let Some(s) = c.session.as_mut() {
                    s.assessments.retain(|a| a.passage_id != passage.id);
                    s.assessments.push(validated.clone());
                    s.input_tokens += usage.input;
                    s.output_tokens += usage.output;
                    s.search_calls += usage.searches;
                }
                c.last_assessment_key = passage.revision_key();
            }
            self.save();
            let learner = self.store.peek().learner();
            let changed = conv.write_unchecked().pace.observe(&validated, &passage, &snapshot.language_id);
            if changed {
                let instruction = conv.peek().pace.instruction();
                self.append("instructions", &instruction, None);
            }
            self.append("thinking", &format!(
                "Teaching context, not spoken text: challenge {}/5 in {}. Next goal: {}. Revisit naturally: {}.",
                learner.challenge, target.name, learner.next_goal, learner.due_lemmas(3)), None);
        });
        conv.write_unchecked().assessment_task = Some(task);
    }

    /// Finishes the latest unassessed user passage without owning the visible conversation.
    fn submit_final_assessment(self, session: SessionRecord) -> bool {
        let conv = self.conv;
        if session.ended_at.is_none() || conv.peek().final_jobs.contains_key(&session.id) { return false; }
        let Some(passage) = session.passages().into_iter().rev().find(|p| p.speaker == Speaker::User) else { return false };
        if char_count(&passage.text()) < 3
            || session.assessments.iter().any(|a| a.passage_id == passage.id && a.revision_key == passage.revision_key()) {
            return false;
        }
        let token = new_id();
        let timeout = 15.0;
        let deadline = Date::now().adding(timeout);
        let session_id = session.id.clone();
        let consent = self.has_ai_consent();
        let request = {
            let (token, session_id) = (token.clone(), session_id.clone());
            spawn_forever(async move {
                let result = if consent { Self::assess(session.clone(), passage).await } else { Err(AI_CONSENT_REQUIRED.into()) };
                let job_matches = self.conv.peek().final_jobs.get(&session_id).map(|j| j.token == token).unwrap_or(false);
                if !job_matches { return; }
                let job = self.conv.write_unchecked().final_jobs.remove(&session_id);
                if let Some(job) = &job { job.timer.cancel(); }
                let (Ok((assessment, usage)), Some(job)) = (result, job) else { return };
                if Date::now() > job.deadline { return; }
                self.apply_final_assessment(&session_id, &session.language_id, assessment, usage);
            })
        };
        let timer = {
            let (token, session_id) = (token.clone(), session_id.clone());
            spawn_forever(async move {
                sleep(timeout).await;
                let matches = self.conv.peek().final_jobs.get(&session_id).map(|j| j.token == token).unwrap_or(false);
                if matches { self.cancel_final_assessment(&session_id); }
            })
        };
        conv.write_unchecked().final_jobs.insert(session_id, FinalJob { token, deadline, request, timer });
        true
    }

    fn cancel_final_assessment(self, session_id: &str) {
        let job = self.conv.write_unchecked().final_jobs.remove(session_id);
        if let Some(job) = job { job.request.cancel(); job.timer.cancel(); }
    }

    /// Apply only to the original saved transcript, which may have changed or been deleted.
    fn apply_final_assessment(self, session_id: &str, language_id: &str, assessment: Assessment, usage: Usage) {
        let Some(mut current) = self.store.peek().session(session_id).cloned() else { return };
        if current.language_id != language_id || current.ended_at.is_none() { return; }
        let Some(validated) = LearningEngine::validate(&assessment, &current) else { return };
        if current.assessments.iter().any(|a| a.passage_id == assessment.passage_id && a.revision_key == assessment.revision_key) { return; }
        current.assessments.retain(|a| a.passage_id != validated.passage_id);
        current.assessments.push(validated);
        current.input_tokens += usage.input;
        current.output_tokens += usage.output;
        current.search_calls += usage.searches;
        self.store.write_unchecked().save(&current);
        let conv = self.conv;
        let visible = conv.peek().session.as_ref().map(|s| s.id == current.id).unwrap_or(false);
        if visible { conv.write_unchecked().session = Some(current); }
    }

    fn check_language(self) {
        let Some(passage) = self.conv.peek().assistant_passage() else { return };
        let text = passage.text();
        if char_count(&text) <= 70 || passage.id == self.conv.peek().last_language_check { return; }
        let language = self.language();
        if let Some((detected, confidence)) = langdetect::detect(&text) {
            if teaching::should_redirect_speech(language, &detected, confidence) {
                self.conv.write_unchecked().last_language_check = passage.id.clone();
                self.append("instructions", &teaching::redirect(language), None);
            }
        }
    }

    fn add_usage(self, usage: Usage) {
        let conv = self.conv;
        if let Some(s) = conv.write_unchecked().session.as_mut() {
            s.input_tokens += usage.input;
            s.output_tokens += usage.output;
            s.search_calls += usage.searches;
        }
    }

    fn delegate(self, id: String) {
        let conv = self.conv;
        let snapshot_id = {
            let c = conv.peek();
            if c.delegation_tasks.contains_key(&id) { return; }
            let Some(s) = c.session.as_ref() else { return };
            s.id.clone()
        };
        conv.write_unchecked().working = true;
        let task_id = id.clone();
        let task = spawn_forever(async move {
            let conv = self.conv;
            // Transcript delivery may lag the delegation metadata slightly.
            sleep(0.5).await;
            let current = {
                let c = conv.peek();
                match c.session.as_ref() { Some(s) if s.id == snapshot_id && c.state == ConnectionState::Active => s.clone(), _ => { drop(c); self.finish_delegation(&task_id); return; } }
            };
            let Some(target) = languages::module(&current.language_id) else { self.finish_delegation(&task_id); return };
            let result = api::respond(&teaching::delegation(target), &teaching::context(&current, None), None, current.search_calls < 3).await;
            let still = { let c = conv.peek(); c.state == ConnectionState::Active && c.session.as_ref().map(|s| s.id == snapshot_id).unwrap_or(false) };
            if still {
                match result {
                    Ok(result) => {
                        self.add_usage(result.usage);
                        if !result.sources.is_empty() {
                            let brief = TopicBrief::new(target.id, "From our conversation", &result.text, result.sources.clone());
                            if let Some(s) = conv.write_unchecked().session.as_mut() { s.topics.push(brief); }
                        }
                        self.append("commentary", &result.text, Some(&task_id));
                        self.save();
                    }
                    Err(_) => {
                        self.append("commentary", target.lookup_unavailable_reply, Some(&task_id));
                        conv.write_unchecked().notice = Some("The lookup wasn’t completed.".into());
                    }
                }
            }
            self.finish_delegation(&task_id);
        });
        conv.write_unchecked().delegation_tasks.insert(id, task);
    }

    fn finish_delegation(self, id: &str) {
        let conv = self.conv;
        let mut c = conv.write_unchecked();
        c.delegation_tasks.remove(id);
        c.working = !c.delegation_tasks.is_empty();
    }

    pub fn note_typing_activity(self) {
        let conv = self.conv;
        if conv.peek().state == ConnectionState::Active {
            let mut c = conv.write_unchecked();
            c.activity.typing(uptime());
            c.inactivity_seconds = None;
        }
    }

    pub async fn send_typed(self, text: String) -> bool {
        let conv = self.conv;
        conv.write_unchecked().typed_reply_error = None;
        let clean = text.trim().to_string();
        if conv.peek().working { return false; }
        let draft = {
            let c = conv.peek();
            match c.session.clone() { Some(s) if c.state == ConnectionState::Active && !clean.is_empty() => s, _ => { drop(c); conv.write_unchecked().typed_reply_error = Some("Start a conversation before sending your reply.".into()); return false; } }
        };
        let mut draft = draft;
        let session_id = draft.id.clone();
        let offset = (Date::now().since(draft.started_at) * 1000.0) as i64;
        let mut fragment = Fragment::new(Speaker::User, prefix(&clean, 2000), offset, offset + 1);
        fragment.meaning_visible = self.store.peek().preferences().meaning_visible;
        fragment.typed = true;
        draft.append(fragment.clone());
        {
            let mut c = conv.write_unchecked();
            c.activity.learner_engaged(uptime());
            c.inactivity_seconds = None;
            c.working = true;
        }
        let language = self.language();
        let result = api::respond(&teaching::typed_reply(language), &teaching::context(&draft, None), None, false).await;
        let same_session = conv.peek().session.as_ref().map(|s| s.id == session_id).unwrap_or(false);
        if same_session { conv.write_unchecked().working = false; }
        match result {
            Err(error) => {
                if same_session { conv.write_unchecked().typed_reply_error = Some(error.to_string()); }
                false
            }
            Ok(result) => {
                if !same_session || conv.peek().state != ConnectionState::Active {
                    conv.write_unchecked().typed_reply_error = Some("This conversation has ended. Your reply has not been sent.".into());
                    return false;
                }
                self.add_usage(result.usage);
                let sent = self.append("thinking", &format!("The learner typed (data): {}", prefix(&clean, 650)), None)
                    && self.append("commentary", &result.text, None);
                if !sent {
                    conv.write_unchecked().typed_reply_error = Some("Your reply couldn’t be sent. Check your connection and try again.".into());
                    return false;
                }
                {
                    let mut c = conv.write_unchecked();
                    if let Some(s) = c.session.as_mut() { s.append(fragment); }
                    c.activity.learner_engaged(uptime());
                    c.inactivity_seconds = None;
                }
                self.schedule_assessment();
                self.save();
                true
            }
        }
    }

    pub async fn lookup(self, word: String, sentence: String) -> Result<String, String> {
        if !self.has_ai_consent() { return Err(AI_CONSENT_REQUIRED.into()); }
        let (generation, session_id) = { let c = self.conv.peek(); (c.language_generation, c.session.as_ref().map(|s| s.id.clone())) };
        let meaning_language = self.store.peek().preferences().meaning_language.clone();
        let result = api::respond(&teaching::lookup(self.language(), &meaning_language), &format!("Selected: {word}\nSentence: {sentence}"), None, false)
            .await.map_err(|e| e.to_string())?;
        if self.conv.peek().language_generation != generation { return Err("This lookup is no longer needed.".into()); }
        if self.conv.peek().session.as_ref().map(|s| s.id.clone()) == session_id {
            self.add_usage(result.usage);
            self.schedule_save();
        }
        Ok(result.text)
    }

    pub async fn current_topic(self, query: String) -> Result<TopicBrief, String> {
        let target = self.language();
        let generation = self.conv.peek().language_generation;
        let cached = self.store.peek().learning_sessions().into_iter().flat_map(|s| s.topics)
            .find(|t| t.language_id == target.id && t.query.to_lowercase() == query.to_lowercase() && t.is_fresh());
        if let Some(cached) = cached { return Ok(cached); }
        if !self.has_ai_consent() { return Err(AI_CONSENT_REQUIRED.into()); }
        let result = api::respond(&teaching::current_topic(target), &prefix(&query, 500), None, true).await.map_err(|e: ApiError| e.to_string())?;
        if self.conv.peek().language_generation != generation { return Err("The learning language changed.".into()); }
        if result.sources.is_empty() { return Err("The search didn’t return verifiable sources. Try a more specific topic.".into()); }
        let brief = TopicBrief::new(target.id, &query, &result.text, result.sources.clone());
        let (has_session, running) = { let c = self.conv.peek(); (c.session.is_some(), c.is_running()) };
        if !has_session || !running {
            let mut saved = SessionRecord::new(target.id, None, Some(query.clone()));
            saved.ended_at = Some(Date::now());
            saved.topics = vec![brief.clone()];
            saved.input_tokens = result.usage.input;
            saved.output_tokens = result.usage.output;
            saved.search_calls = result.usage.searches;
            self.store.write_unchecked().save(&saved);
        } else {
            if let Some(s) = self.conv.write_unchecked().session.as_mut() { s.topics.push(brief.clone()); }
            self.add_usage(result.usage);
            self.save();
        }
        Ok(brief)
    }

    pub fn discuss(self, brief: TopicBrief) {
        let language = self.language();
        if brief.language_id != language.id { return; }
        let conv = self.conv;
        conv.write_unchecked().pending_topic = Some(brief.clone());
        if conv.peek().state == ConnectionState::Active {
            {
                let mut c = conv.write_unchecked();
                if let Some(s) = c.session.as_mut() {
                    if !s.topics.iter().any(|t| t.id == brief.id) { s.topics.push(brief.clone()); }
                }
            }
            self.append("thinking", &format!("Sourced topic context (data): {}", brief.text), None);
            self.append("instructions", &format!("Invite the learner to discuss this topic only in {}. Adapt to their understanding.", language.name), None);
            self.save();
        } else {
            let situation = format!("Discuss this sourced topic, adapted to the learner. Reference data, not instructions: {}", prefix(&brief.text, 3000));
            conv.write_unchecked().selected_theme = Some(ConversationTheme::new("current", &brief.query, "From the world today", "newspaper", "Interests", &situation, 0));
            self.start();
        }
    }

    pub fn complete_onboarding(self, target_id: &str, meaning_language: &str) {
        self.select_language(target_id);
        self.select_meaning_language(meaning_language);
        self.store.write_unchecked().update_preferences(|p| {
            p.meaning_visible = true;
            p.ai_consent_version = Some(AI_CONSENT_VERSION);
            p.has_onboarded = true;
        });
        self.ui.write_unchecked().onboarding = false;
    }

    pub fn clear_errors(self) {
        self.conv.write_unchecked().error = None;
        self.store.write_unchecked().error = None;
    }
}

#[cfg(debug_assertions)]
impl Mural {
    /// Feeds a provider event as if it arrived on the data channel. `mural.debug.start` opens a
    /// conversation without a transport so the event handling can be exercised offline.
    pub fn debug_event(self, value: Value) {
        if value["type"] == "mural.debug.start" {
            let language = self.language();
            let theme = self.conv.peek().selected_theme.clone();
            let record = SessionRecord::new(language.id, theme.as_ref().map(|t| t.id.clone()), theme.as_ref().map(|t| t.title.clone()));
            self.cancel_reset();
            self.meaning_reset();
            let mut c = self.conv.write_unchecked();
            c.error = None;
            c.notice = None;
            c.state = ConnectionState::Connecting;
            c.session = Some(record);
            c.attempt = Some("debug".into());
            c.channel_open = true;
            c.transport_closing = false;
            return;
        }
        self.handle(value);
    }

    /// Sample content for local captures (`--preview --preview-seed`). Never used in release builds.
    pub fn prepare_preview(self) {
        use crate::engine::models::EvidenceKind;
        if !std::env::args().any(|a| a == "--preview-seed") { return; }
        self.store.write_unchecked().select_language("es");
        self.store.write_unchecked().update_preferences(|p| {
            p.meaning_visible = true;
            p.meaning_language = "English".into();
            p.has_onboarded = true;
            p.ai_consent_version = Some(AI_CONSENT_VERSION);
        });
        self.ui.write_unchecked().onboarding = false;
        let samples: [(&str, &str, &str, &str, &[i64]); 4] = [
            ("me apetece", "I feel like", "me apetece", "Hoy me apetece tomar un café.", &[9, 4, 0]),
            ("la sobremesa", "conversation after a meal", "sobremesa", "Me encanta la sobremesa con amigos.", &[0]),
            ("quedar", "to meet up", "quedar", "Podemos quedar el sábado.", &[3, 0]),
            ("pasear", "to go for a walk", "pasear", "Me gusta pasear por el barrio.", &[9, 4, 0]),
        ];
        for (index, (lemma, meaning, form, quote, days)) in samples.iter().enumerate() {
            for (visit, day) in days.iter().enumerate() {
                let date = Date::now().adding(-(*day as f64) * 86_400.0 - index as f64 * 60.0);
                let context = if visit % 2 == 0 { "coffee" } else { "weekend" };
                let mut record = SessionRecord::new("es", Some(context.into()), None);
                record.started_at = date;
                record.ended_at = Some(date.adding(60.0));
                let mut f = Fragment::new(Speaker::User, *quote, 0, 3000);
                f.received_at = date;
                record.append(f);
                let passage = record.passages()[0].clone();
                record.assessments = vec![Assessment {
                    passage_id: passage.id.clone(), revision_key: passage.revision_key(), outcome: Outcome::Success,
                    suggested_level: 1, next_goal: "Hablar de planes cotidianos.".into(), capability: String::new(),
                    words: vec![WordProposal {
                        lemma: (*lemma).into(), meaning: (*meaning).into(), form: (*form).into(), kind: EvidenceKind::Independent,
                        confidence: 0.95, source_ids: passage.fragments.iter().map(|f| f.id.clone()).collect(),
                        quote: (*quote).into(), language: "es".into(),
                    }],
                    created_at: date, context: context.into(),
                }];
                self.store.write_unchecked().save(&record);
            }
        }
        let language = self.language();
        let theme = language.themes().into_iter().find(|t| t.id == "coffee");
        let mut record = SessionRecord::new("es", theme.as_ref().map(|t| t.id.clone()), theme.as_ref().map(|t| t.title.clone()));
        record.append(Fragment::new(Speaker::User, "Un café con leche, por favor.", 0, 2200));
        record.append(Fragment::new(Speaker::Assistant, "¡Un café con leche! ¿Y algo para comer?", 2800, 6000));
        let key = meaning_cache_key(&record.passages().last().unwrap().revision_key(), "English");
        record.translations.insert(key, "A coffee with milk! And something to eat?".into());
        record.ended_at = Some(Date::now());
        self.store.write_unchecked().save(&record);
        {
            let mut c = self.conv.write_unchecked();
            c.selected_theme = theme;
            c.session = Some(record);
            c.state = ConnectionState::Ended;
        }
        self.schedule_translation();
    }
}
