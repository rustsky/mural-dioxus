use std::collections::HashMap;
use std::time::Duration;

use dioxus::core::{spawn_forever, Task};
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::engine::activity::{Action, ConversationActivity};
use crate::engine::languages::{self, ConversationTheme, LanguageModule, MEANING_LANGUAGES};
use crate::engine::learning::LearningEngine;
use crate::engine::models::{Assessment, ErrorProposal, Fragment, Outcome, Passage, SessionRecord, Speaker, TopicBrief, WordProposal};
use crate::engine::pace::ConversationPace;
use crate::engine::phrases::{self, Phrase};
use crate::engine::plan::TeachingPlan;
use crate::engine::scenarios::{beat_window, parse_checklist, Scenario, BEATS_PER_TURN};
use crate::engine::shadow::{self as shadow_engine, Attempt as ShadowAttempt};
use crate::engine::text::{char_count, prefix};
use crate::engine::time::{new_id, uptime, Date};
use crate::engine::{langdetect, meaning_cache_key, teaching, AI_CONSENT_REQUIRED, AI_CONSENT_VERSION};
use crate::services::api::{self, ApiError, ApiResult, Usage};
use crate::services::credentials::{OLLAMA, OPENAI};
use crate::services::provider;
use crate::services::dialog_store;
use crate::services::shadow_store;
use crate::services::voice;
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
    /// The scenario being played, if the learner started one from Dialog.
    pub scenario: Option<&'static Scenario>,
    /// Indices of the beats reached so far, in the order they were reported.
    pub beats_covered: Vec<usize>,
    /// True once Mural has been told to start closing; it is only said once.
    wrapping_up: bool,
    pub input_level: f64,
    pub output_level: f64,
    pub is_muted: bool,
    /// Push to talk only: true while the learner holds the microphone.
    pub holding: bool,
    /// Mirrors the preference, so the view can label the microphone without reading settings.
    pub hold_to_talk: bool,
    /// Speech captured during this hold, still waiting for the learner to finish.
    held_speech: bool,
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
    // Local voice (Ollama teacher): speech recognition → teacher → speech synthesis on this computer
    local_voice: bool,
    local_notes: Vec<String>,
    local_turn: Option<Task>,
    local_turn_again: bool,
    natural_voice: bool,
    speech_task: Option<Task>,
    prepare_task: Option<Task>,
    pub preparing: Option<String>,
    // Coordinator tasks
    connection_task: Option<Task>,
    assessment_task: Option<Task>,
    /// A turn arrived while an assessment was running; assess again once it finishes.
    assessment_again: bool,
    /// The scenario checker: its own session beside the conversation and the assessment, on the
    /// same teacher, so a long assessment never holds it back and ending the conversation never
    /// cuts it off.
    check_task: Option<Task>,
    check_again: bool,
    delegation_tasks: HashMap<String, Task>,
    close_task: Option<Task>,
    duration_task: Option<Task>,
    save_task: Option<Task>,
    reset_task: Option<Task>,
    reset_deadline: Option<f64>,
    activity: ConversationActivity,
    pace: ConversationPace,
    plan: TeachingPlan,
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
            state: ConnectionState::Idle, session: None, selected_theme: None,
            scenario: None, beats_covered: vec![], wrapping_up: false, input_level: 0.0, output_level: 0.0,
            is_muted: false, holding: false, hold_to_talk: false, held_speech: false, working: false, error: None, typed_reply_error: None, notice: None, inactivity_seconds: None,
            meaning: Meaning::default(), start_after_consent: false,
            attempt: None, instructions: String::new(), channel_open: false, started: false, transport_closing: false,
            recovery_task: None, ready_task: None,
            local_voice: false, local_notes: vec![], local_turn: None, local_turn_again: false,
            natural_voice: false, speech_task: None, prepare_task: None, preparing: None,
            connection_task: None, assessment_task: None, assessment_again: false, check_task: None, check_again: false,
            delegation_tasks: HashMap::new(), close_task: None,
            duration_task: None, save_task: None, reset_task: None, reset_deadline: None,
            activity: ConversationActivity::new(0.0), pace: ConversationPace::default(), plan: TeachingPlan::default(),
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
            ConnectionState::Connecting if self.preparing.is_some() => return self.preparing.clone().unwrap_or_default(),
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
            ConnectionState::Active if self.holding => "Listening — release when you’re done",
            ConnectionState::Active if self.hold_to_talk => "Hold the microphone to speak",
            ConnectionState::Active if self.is_muted => "Microphone muted",
            ConnectionState::Active => "Microphone on",
            ConnectionState::Connecting => "Connecting microphone",
            _ => "Microphone off",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab { Talk, Shadow, Dialog, Words }

#[derive(Clone, Debug, PartialEq)]
pub enum Sheet {
    Settings,
    AiConsent,
    TypedReply,
    Transcript,
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
    Local { attempt: String, event: String, #[serde(default)] text: String },
    Pcm { attempt: String, data: String },
}

const NETWORK_LOST: &str = "The network connection was lost. Click the microphone to start a new conversation.";
const TIME_LIMIT_NOTICE: &str = "You’ve reached your conversation time limit.";
const QUIET_NOTICE: &str = "Mural ended this quiet session to avoid running up usage.";

fn decode_pcm(data: &str) -> Option<Vec<f32>> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).ok()?;
    Some(bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect())
}

fn encode_pcm(samples: &[f32]) -> String {
    use base64::Engine;
    let bytes: Vec<u8> = samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn tracing() -> bool {
    static TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TRACE.get_or_init(|| std::env::var("MURAL_TRACE").is_ok())
}

async fn sleep(seconds: f64) { tokio::time::sleep(Duration::from_secs_f64(seconds)).await }

/// One phrase being drilled: what is playing, what was heard, and how it scored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shadow {
    /// The bridge session id while the microphone is ours.
    pub attempt: Option<String>,
    pub phrase_id: Option<String>,
    pub holding: bool,
    pub speaking: bool,
    pub ready: bool,
    /// Download or warm-up progress, shown in place of the drill.
    pub preparing: Option<String>,
    pub notice: Option<String>,
    pub result: Option<ShadowAttempt>,
    /// What the recogniser has returned during this hold.
    heard: Vec<String>,
}

impl Shadow {
    pub fn is_busy(&self) -> bool { self.preparing.is_some() }
}

/// Bumped whenever the teacher settings change. Views that branch on them read this first, so
/// they re-render when the teacher does.
pub static PROVIDER_REVISION: GlobalSignal<u32> = Signal::global(|| 0);

/// Saves teacher settings and lets every view that depends on them know.
pub fn update_provider(change: impl FnOnce(&mut provider::ProviderSettings)) -> Result<provider::ProviderSettings, String> {
    let saved = provider::update(change);
    if saved.is_ok() { *PROVIDER_REVISION.write() += 1; }
    saved
}

/// The app's single controller: a copyable handle over reactive state, like ConversationCoordinator on iPhone.
#[derive(Clone, Copy, PartialEq)]
pub struct Mural {
    pub conv: Signal<Conv>,
    pub store: Signal<LearningStore>,
    pub ui: Signal<Ui>,
    pub shadow: Signal<Shadow>,
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
            shadow: Signal::new_in_scope(Shadow::default(), root),
            bridge: Signal::new_in_scope(None, root),
        }
    }

    // ---------- Bridge ----------

    pub fn attach_bridge(self) {
        let mut eval = document::eval(include_str!("../bridge.js"));
        let bridge = self.bridge;
        *bridge.write_unchecked() = Some(eval);
        // MURAL_TRACE prints bridge traffic to stderr, in release builds too, for diagnosing voice issues.
        let fake = cfg!(debug_assertions) && std::env::var("MURAL_FAKE_MICROPHONE").is_ok();
        let phrases: Vec<String> = if fake {
            std::env::var("MURAL_FAKE_SPEECH").unwrap_or_default().split('|').filter(|p| !p.is_empty()).map(String::from).collect()
        } else { vec![] };
        // Debug builds: WAV files stand in for the learner's voice in natural-voice conversations.
        let clips: Vec<String> = if fake {
            std::env::var("MURAL_FAKE_SPEECH_WAV").unwrap_or_default().split('|').filter_map(|path| {
                let wave = sherpa_onnx::Wave::read(path)?;
                let samples = sherpa_onnx::LinearResampler::create(wave.sample_rate(), voice::SAMPLE_RATE)?.resample(wave.samples(), true);
                Some(encode_pcm(&samples))
            }).collect()
        } else { vec![] };
        let _ = eval.send(json!({"cmd": "config", "fakeMicrophone": fake, "fakeSpeech": phrases, "fakeAudio": clips, "trace": tracing()}));
        spawn_forever(async move {
            loop {
                match eval.recv::<Value>().await {
                    Ok(value) => {
                        if tracing() && value["kind"] != "levels" && value["kind"] != "pcm" {
                            eprintln!("{:.1} bridge <- {}", uptime(), prefix(&value.to_string(), 300));
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
        if tracing() && value["cmd"] != "send" && value["cmd"] != "play" { eprintln!("{:.1} bridge -> {}", uptime(), prefix(&value.to_string(), 200)); }
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
                    "speech" => "Allow speech recognition in System Settings → Privacy & Security → Speech Recognition → Mural, then start again.",
                    "speech-disabled" => "Speech recognition needs Dictation. Turn it on in System Settings → Keyboard → Dictation (Siri can stay off), then start again.",
                    "speech-unavailable" => "Speech recognition stopped working. Check that Dictation is on in System Settings → Keyboard, then start again.",
                    "speech-natural" => "The natural voice couldn’t use the microphone. Check your audio input and try again.",
                    "speech-unsupported" => "Speech recognition isn’t available on this computer. On a Mac, turn on Dictation in System Settings, or type your replies.",
                    "speech-network" => "Speech recognition couldn’t reach Apple’s service. Check your connection, or turn on on-device Dictation in System Settings.",
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
            BridgeMessage::Local { attempt, event, text } => {
                if !self.current_attempt(&attempt) { return; }
                match event.as_str() {
                    "ready" => {
                        conv.write_unchecked().started = true;
                        self.handle(json!({"type": "session.started", "session": {}}));
                    }
                    "heard" => self.local_heard(&text),
                    _ => {}
                }
            }
            BridgeMessage::Pcm { attempt, data } => {
                if self.shadow.peek().attempt.as_deref() == Some(attempt.as_str()) {
                    let Some(samples) = decode_pcm(&data) else { return };
                    let shadow = self.shadow;
                    spawn_forever(async move {
                        match voice::hear(samples).await {
                            Ok(heard) if !heard.is_empty() => shadow.write_unchecked().heard.extend(heard),
                            Ok(_) => {}
                            Err(error) => eprintln!("voice: {error}"),
                        }
                    });
                    return;
                }
                if !self.current_attempt(&attempt) || !conv.peek().natural_voice { return; }
                let Some(samples) = decode_pcm(&data) else { return };
                spawn_forever(async move {
                    let heard = match voice::hear(samples).await {
                        Ok(heard) => heard,
                        Err(error) => { eprintln!("voice: {error}"); return; }
                    };
                    if tracing() && !heard.is_empty() { eprintln!("{:.1} heard {heard:?}", uptime()); }
                    for text in heard {
                        if !self.current_attempt(&attempt) { return; }
                        self.local_heard(&text);
                    }
                });
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
        if self.conv.peek().local_voice { return self.conv.peek().channel_open; }
        let attempt = { let c = self.conv.peek(); if !c.channel_open { return false; } c.attempt.clone() };
        let Some(attempt) = attempt else { return false };
        self.bridge_send(json!({"cmd": "send", "attempt": attempt, "data": event.to_string()}));
        true
    }

    fn transport_connect(self, instructions: String, local: bool) {
        let natural = local && provider::current().voice == provider::VoiceStyle::Natural && voice::supported(self.language().id);
        self.transport_disconnect();
        let attempt = new_id();
        {
            let mut c = self.conv.write_unchecked();
            c.attempt = Some(attempt.clone());
            c.instructions = instructions;
            c.channel_open = local;
            c.started = false;
            c.transport_closing = false;
            c.local_voice = local;
            c.natural_voice = natural;
        }
        if natural {
            self.prepare_natural_voice(attempt);
        } else if local {
            self.bridge_send(json!({"cmd": "local_start", "attempt": attempt, "lang": self.language().locale, "engine": "system"}));
        } else {
            self.bridge_send(json!({"cmd": "connect", "attempt": attempt}));
        }
    }

    fn transport_close(self) {
        if self.conv.peek().local_voice {
            // Nothing is billed on this computer, so the session closes with zero voice time.
            self.bridge_send(json!({"cmd": "mute", "muted": true}));
            let reason = self.conv.peek().session.as_ref().and_then(|s| s.end_reason.clone());
            spawn_forever(async move {
                self.handle(json!({"type": "session.closed", "usage": {"seconds": 0}, "reason": reason}));
            });
            return;
        }
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
            for t in [c.local_turn.take(), c.speech_task.take(), c.prepare_task.take()].into_iter().flatten() { t.cancel(); }
            c.natural_voice = false;
            c.preparing = None;
            c.local_notes.clear();
            c.local_turn_again = false;
            c.local_voice = false;
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
        let teacher = provider::current().teacher;
        let local = teacher.is_ollama();
        if local && teacher == provider::Teacher::OllamaCloud && !OLLAMA.has_key() {
            conv.write_unchecked().error = Some(ApiError::OllamaMissingKey.to_string());
            self.open_sheet(Sheet::Settings);
            return;
        }
        if !local && !OPENAI.has_key() { self.open_sheet(Sheet::Settings); return; }
        self.cancel_reset();
        self.meaning_reset();
        let language = self.language();
        let (theme, topic, scenario) = {
            let c = conv.peek();
            (c.selected_theme.clone(), c.pending_topic.clone(), c.scenario)
        };
        let mut record = SessionRecord::new(
            language.id,
            scenario.map(|s| s.id.clone()).or_else(|| theme.as_ref().map(|t| t.id.clone())),
            scenario.map(|s| s.title.clone()).or_else(|| theme.as_ref().map(|t| t.title.clone())),
        );
        if let Some(topic) = topic.clone() { record.topics = vec![topic]; }
        {
            let mut c = conv.write_unchecked();
            c.error = None;
            c.notice = None;
            c.last_assessment_key.clear();
            c.last_language_check.clear();
            c.pending_commands.clear();
            c.state = ConnectionState::Connecting;
            c.is_muted = false;
            c.beats_covered.clear();
            c.wrapping_up = false;
            c.session = Some(record.clone());
        }
        self.store.write_unchecked().save(&record);
        // Each new conversation starts fresh; learned vocabulary and difficulty still carry forward.
        let (learner, interests, meaning_language) = {
            let s = self.store.peek();
            (s.learner(), s.preferences().interests.clone(), s.preferences().meaning_language.clone())
        };
        let brief = teaching::Brief { theme: theme.as_ref(), scenario, topic: topic.as_ref() };
        let mut instructions = teaching::voice(language, &learner, brief, &interests, &meaning_language);
        if local { instructions = format!("{instructions}\n{}", teaching::local_voice(language)); }
        self.transport_connect(instructions, local);
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
            for t in [c.connection_task.take(), c.close_task.take(), c.duration_task.take(), c.assessment_task.take(), c.check_task.take(), c.save_task.take()].into_iter().flatten() {
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


    /// Opens the microphone while the learner holds it, and ends the turn the moment they let go.
    pub fn set_holding(self, holding: bool) {
        let conv = self.conv;
        if conv.peek().state != ConnectionState::Active || !self.push_to_talk() { return; }
        if conv.peek().holding == holding { return; }
        {
            let mut c = conv.write_unchecked();
            c.holding = holding;
            c.is_muted = !holding;
            if holding { c.held_speech = false; }
            // Holding the microphone is engagement, even before any words arrive.
            if holding { c.activity.learner_engaged(uptime()); }
        }
        if holding {
            // The floor is theirs: stop Mural mid-sentence rather than swallow their opening words,
            // which the recogniser would otherwise receive as a fragment and guess at.
            if let Some(t) = conv.write_unchecked().speech_task.take() { t.cancel(); }
            self.bridge_send(json!({"cmd": "barge_in"}));
            voice::reset_listening();
        }
        self.bridge_send(json!({"cmd": "mute", "muted": !holding}));
        let kind = if holding { "session.input_audio.unmute" } else { "session.input_audio.mute" };
        self.transport_send(json!({"type": kind, "event_id": new_id()}));
        if !holding {
            if tracing() { eprintln!("{:.1} released, speech held: {}", uptime(), conv.peek().held_speech); }
            // The provider's own detector closes a live turn; on-device voice needs telling.
            if conv.peek().natural_voice { self.end_local_turn(); }
            // System speech has no buffer to flush, so whatever was heard is already in hand.
            else if std::mem::take(&mut conv.write_unchecked().held_speech) { self.local_turn(); }
        }
    }

    pub fn push_to_talk(self) -> bool { provider::current().push_to_talk }

    // ---------- Shadowing ----------

    /// The phrase being drilled, if the catalogue still has it.
    pub fn shadow_phrase(self) -> Option<&'static Phrase> {
        let id = self.shadow.peek().phrase_id.clone()?;
        phrases::find(self.language().id, &id)
    }

    /// Opens a phrase and makes sure the voice is ready to speak and listen.
    pub fn shadow_open(self, phrase_id: String) {
        if self.conv.peek().is_running() { return; }
        {
            let mut s = self.shadow.write_unchecked();
            s.phrase_id = Some(phrase_id);
            s.result = None;
            s.notice = None;
            s.heard.clear();
        }
        if self.shadow.peek().ready || self.shadow.peek().is_busy() { return; }
        self.shadow_prepare();
    }

    /// Downloads the voice models if needed, then opens the microphone for the drill.
    fn shadow_prepare(self) {
        let language_id = self.language().id;
        if !voice::supported(language_id) {
            self.shadow.write_unchecked().notice = Some("Shadowing needs the on-device voice, which doesn’t cover this language yet.".into());
            return;
        }
        let shadow = self.shadow;
        let attempt = format!("shadow-{}", new_id());
        shadow.write_unchecked().attempt = Some(attempt.clone());
        spawn_forever(async move {
            let megabytes = voice::missing_megabytes(language_id);
            if megabytes > 0 {
                shadow.write_unchecked().preparing = Some(format!("Downloading the voice… 0% of {megabytes} MB"));
                let progress = move |step: voice::Progress| {
                    let text = match step {
                        voice::Progress::Downloading(fraction) => format!("Downloading the voice… {}% of {megabytes} MB", (fraction * 100.0).floor() as i64),
                        voice::Progress::Unpacking => "Unpacking the voice…".to_string(),
                    };
                    let mut s = shadow.write_unchecked();
                    if s.preparing.as_ref() != Some(&text) { s.preparing = Some(text); }
                };
                if let Err(error) = voice::install(language_id, progress).await {
                    let mut s = shadow.write_unchecked();
                    s.preparing = None;
                    s.notice = Some(error);
                    return;
                }
            }
            if shadow.peek().attempt.as_deref() != Some(attempt.as_str()) { return; }
            shadow.write_unchecked().preparing = Some("Warming up the voice…".into());
            let result = voice::prepare(language_id).await;
            if shadow.peek().attempt.as_deref() != Some(attempt.as_str()) { return; }
            {
                let mut s = shadow.write_unchecked();
                s.preparing = None;
                match &result {
                    Ok(()) => { s.ready = true; }
                    Err(error) => { s.notice = Some(error.clone()); }
                }
            }
            if result.is_ok() {
                // The same audio session a local conversation uses: microphone in, playback out.
                self.bridge_send(json!({"cmd": "local_start", "attempt": attempt, "lang": self.language().locale, "engine": "natural"}));
                self.bridge_send(json!({"cmd": "mute", "muted": true}));
            }
        });
    }

    /// Opens a phrase that has never been tried, chosen at random from `within` when it is not
    /// empty, so the category showing is respected. Says so when there is nothing left untried.
    pub fn shadow_open_untried(self, within: Vec<String>) {
        let record = shadow_store::current();
        let language_id = self.language().id;
        let pool: Vec<&Phrase> = phrases::all(language_id).iter()
            .filter(|p| record.get(&p.id).attempts == 0)
            .filter(|p| within.is_empty() || within.contains(&p.id))
            .collect();
        // uuid v4 is already how this app gets randomness.
        let seed = uuid::Uuid::new_v4().as_u128();
        match phrases::pick(&pool, seed) {
            Some(phrase) => self.shadow_open(phrase.id.clone()),
            None => self.shadow.write_unchecked().notice = Some("You’ve tried every phrase here. Pick one to say again.".into()),
        }
    }

    /// Back to the list, keeping the voice ready for the next phrase.
    pub fn shadow_open_none(self) {
        let mut s = self.shadow.write_unchecked();
        s.phrase_id = None;
        s.result = None;
        s.heard.clear();
    }

    /// Hands the microphone and the models back when the drill is left.
    pub fn shadow_close(self) {
        let had = self.shadow.write_unchecked().attempt.take();
        *self.shadow.write_unchecked() = Shadow::default();
        if had.is_some() {
            self.bridge_send(json!({"cmd": "mute", "muted": true}));
            self.bridge_send(json!({"cmd": "disconnect"}));
        }
    }

    /// Speaks the phrase: `slow` is for hearing every syllable.
    pub fn shadow_say(self, slow: bool) {
        let (Some(phrase), Some(attempt)) = (self.shadow_phrase(), self.shadow.peek().attempt.clone()) else { return };
        if !self.shadow.peek().ready { return; }
        let text = phrase.text.clone();
        self.shadow.write_unchecked().speaking = true;
        voice::reset_listening();
        self.bridge_send(json!({"cmd": "play_begin", "attempt": attempt}));
        let shadow = self.shadow;
        spawn_forever(async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let generation = voice::speak(text, if slow { 0.7 } else { 0.95 }, tx);
            let forward = async {
                let mut seconds = 0.0;
                while let Some((samples, rate)) = rx.recv().await {
                    seconds += samples.len() as f64 / rate.max(1) as f64;
                    self.bridge_send(json!({"cmd": "play", "attempt": attempt, "rate": rate, "data": encode_pcm(&samples)}));
                }
                seconds
            };
            let (result, seconds) = tokio::join!(generation, forward);
            if tracing() { eprintln!("{:.1} shadow spoke {:.1}s of audio", uptime(), seconds); }
            self.bridge_send(json!({"cmd": "play_end", "attempt": attempt}));
            let mut s = shadow.write_unchecked();
            s.speaking = false;
            if let Err(error) = result { s.notice = Some(error); }
        });
    }

    /// Holds the microphone open for the learner's repeat, and scores it on release.
    pub fn shadow_hold(self, holding: bool) {
        let shadow = self.shadow;
        if !shadow.peek().ready || shadow.peek().holding == holding { return; }
        if holding && shadow.peek().speaking { return; }
        {
            let mut s = shadow.write_unchecked();
            s.holding = holding;
            if holding { s.heard.clear(); s.result = None; }
        }
        if holding {
            self.bridge_send(json!({"cmd": "barge_in"}));
        }
        self.bridge_send(json!({"cmd": "mute", "muted": !holding}));
        if holding { voice::reset_listening(); return; }
        spawn_forever(async move {
            // The last audio is still crossing from the page; flushing now would clip it.
            sleep(0.25).await;
            if let Ok(rest) = voice::end_turn().await {
                shadow.write_unchecked().heard.extend(rest);
            }
            self.shadow_score();
        });
    }

    /// Compares what was heard with the phrase, and records the attempt.
    fn shadow_score(self) {
        let Some(phrase) = self.shadow_phrase() else { return };
        let heard = self.shadow.peek().heard.join(" ");
        let attempt = shadow_engine::score(&phrase.text, &heard);
        let good = attempt.verdict() == shadow_engine::Verdict::Good;
        if let Err(error) = shadow_store::update(|record| { record.record(&phrase.id, attempt.score, good, Date::now()); }) {
            self.shadow.write_unchecked().notice = Some(error);
        }
        let mut s = self.shadow.write_unchecked();
        s.result = Some(attempt);
    }

    /// Opens the phrase on PlayPhrase in the browser, where film clips of it live.
    pub fn shadow_in_films(self) {
        let Some(phrase) = self.shadow_phrase() else { return };
        let query: String = phrase.text.chars().map(|c| match c {
            ' ' => "%20".to_string(),
            c if c.is_ascii_alphanumeric() => c.to_string(),
            c => c.to_string().bytes().map(|b| format!("%{b:02X}")).collect(),
        }).collect();
        let language = self.language().id;
        let _ = open::that(format!("https://playphrase.me/#/search?q={query}&language={language}"));
    }

    /// Transcribes whatever the learner said before letting go, without waiting for silence.
    fn end_local_turn(self) {
        let attempt = self.conv.peek().attempt.clone();
        let Some(attempt) = attempt else { return };
        spawn_forever(async move {
            // The last audio chunks are still crossing from the page; flushing now would clip them.
            sleep(0.25).await;
            let heard = match voice::end_turn().await {
                Ok(heard) => heard,
                Err(error) => { eprintln!("voice: {error}"); return; }
            };
            if tracing() && !heard.is_empty() { eprintln!("{:.1} heard on release {heard:?}", uptime()); }
            if !self.current_attempt(&attempt) { return; }
            for text in heard.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
                self.local_transcript(Speaker::User, text);
                self.conv.write_unchecked().held_speech = true;
            }
            // One reply for everything said during the hold, however many pauses it contained.
            let spoke = std::mem::take(&mut self.conv.write_unchecked().held_speech);
            if spoke { self.local_turn(); } else { self.local_listen(); }
        });
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
            if let Some(t) = c.check_task.take() { t.cancel(); }
            if let Some(t) = c.save_task.take() { t.cancel(); }
        }
        self.reset_conversation();
        // Shadowing progress is learning data too, even though it lives in its own file.
        if let Err(error) = shadow_store::remove_all() { self.conv.write_unchecked().notice = Some(error); }
        if let Err(error) = dialog_store::remove_all() { self.conv.write_unchecked().notice = Some(error); }
        *self.shadow.write_unchecked() = Shadow::default();
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

    /// Changing this mid-conversation would leave the microphone in a state the learner did not
    /// choose, so it only takes effect for the next one.
    /// Returns the setting as it now stands, so the view can follow it.
    pub fn toggle_push_to_talk(self) -> bool {
        if self.conv.peek().is_running() { return provider::current().push_to_talk; }
        let held = !provider::current().push_to_talk;
        match update_provider(|s| s.push_to_talk = held) {
            Ok(updated) => updated.push_to_talk,
            Err(error) => {
                self.conv.write_unchecked().notice = Some(error);
                provider::current().push_to_talk
            }
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
        self.local_speak_now();
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
            for t in [c.connection_task.take(), c.duration_task.take()].into_iter().flatten() { t.cancel(); }
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
            for t in [c.close_task.take(), c.duration_task.take(), c.connection_task.take(), c.save_task.take()].into_iter().flatten() {
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
        if self.conv.peek().scenario.is_some() { self.save_scenario_progress(); }
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
        if conv.peek().local_voice { return self.local_append(kind, text); }
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
                    c.holding = false;
                    c.activity = ConversationActivity::with_challenge(uptime(), self.store.peek().learner().challenge);
                    c.pace = ConversationPace::default();
                    c.plan = TeachingPlan::default();
                    c.inactivity_seconds = None;
                    if let Some(s) = c.session.as_mut() { s.provider_id = event["session"]["id"].as_str().map(String::from); }
                }
                // Push to talk keeps the microphone closed until the learner asks for it.
                conv.write_unchecked().hold_to_talk = self.push_to_talk();
                if self.push_to_talk() {
                    conv.write_unchecked().is_muted = true;
                    self.bridge_send(json!({"cmd": "mute", "muted": true}));
                    self.transport_send(json!({"type": "session.input_audio.mute", "event_id": new_id()}));
                }
                self.append("instructions", &teaching::greeting(self.language()), None);
                self.local_speak_now();
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
                    self.schedule_check();
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
                let (started_at, busy, muted, local) = {
                    let c = conv.peek();
                    let Some(session) = c.session.as_ref() else { return };
                    if c.state != ConnectionState::Active { return; }
                    (session.started_at, c.working || !c.delegation_tasks.is_empty(), c.is_muted, c.local_voice)
                };
                // A model on this computer costs nothing, so its conversations never end on their own.
                let unlimited = local && provider::current().teacher == provider::Teacher::OllamaLocal;
                // A scenario has its own length: near the end, Mural is told to land it.
                let scenario = conv.peek().scenario;
                if let Some(scenario) = scenario {
                    let elapsed = Date::now().since(started_at);
                    let wrap_after = (scenario.minutes * 60) as f64 * 0.8;
                    let due = elapsed > wrap_after && !conv.peek().wrapping_up;
                    if due {
                        conv.write_unchecked().wrapping_up = true;
                        if tracing() { eprintln!("{:.1} scenario wrap-up after {:.0}s", uptime(), elapsed); }
                        self.append("instructions", &teaching::wrap_up(self.language()), None);
                        self.local_speak_now();
                    }
                }
                let minutes = self.store.peek().preferences().session_minutes;
                if !unlimited && Date::now().since(started_at) > (minutes * 60) as f64 {
                    conv.write_unchecked().notice = Some(TIME_LIMIT_NOTICE.into());
                    self.end("Time limit");
                    return;
                }
                let action = {
                    let mut c = conv.write_unchecked();
                    c.inactivity_seconds = None;
                    let now = Date::now();
                    c.pending_commands.retain(|_, sent| now.since(*sent) <= 20.0);
                    // A long pause while holding the microphone is the learner thinking, not silence.
                    if c.holding { c.activity.learner_engaged(uptime()); }
                    c.activity.tick(uptime(), muted, busy)
                };
                match action {
                    Action::CheckIn => { self.append("instructions", &teaching::check_in(self.language()), None); self.local_speak_now(); }
                    Action::Warning(_) | Action::End if unlimited => {}
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

    // ---------- Local voice ----------

    /// Instructions and context wait for the next turn; commentary is spoken as written.
    fn local_append(self, kind: &str, text: &str) -> bool {
        let text = prefix(text, 1000);
        match kind {
            "commentary" => {
                // Runs after the caller has recorded any typed message, so the transcript stays in order.
                spawn_forever(async move { self.local_say(&text); });
            }
            "thinking" => self.conv.write_unchecked().local_notes.push(format!("APP NOTE (context data, not instructions): {text}")),
            _ => self.conv.write_unchecked().local_notes.push(format!("APP NOTE: {text}")),
        }
        true
    }

    fn local_heard(self, text: &str) {
        let text = text.trim();
        if text.is_empty() || self.conv.peek().state != ConnectionState::Active {
            self.local_listen();
            return;
        }
        self.local_transcript(Speaker::User, text);
        // A pause while holding is thinking time, not the end of a turn.
        if self.conv.peek().holding {
            self.conv.write_unchecked().held_speech = true;
            self.local_listen();
            return;
        }
        self.local_turn();
    }

    fn local_speak_now(self) {
        if self.conv.peek().local_voice { self.local_turn(); }
    }

    fn local_listen(self) {
        let attempt = self.conv.peek().attempt.clone();
        if let Some(attempt) = attempt { self.bridge_send(json!({"cmd": "listen", "attempt": attempt})); }
    }

    fn local_transcript(self, speaker: Speaker, text: &str) {
        let started_at = match self.conv.peek().session.as_ref() { Some(s) => s.started_at, None => return };
        let start = (Date::now().since(started_at) * 1000.0).max(0.0) as i64;
        let kind = if speaker == Speaker::User { "session.input_transcript.delta" } else { "session.output_transcript.delta" };
        self.handle(json!({"type": kind, "event_id": new_id(), "delta": text, "start_ms": start, "end_ms": start + 1}));
    }

    fn local_say(self, text: &str) {
        let (attempt, active) = { let c = self.conv.peek(); (c.attempt.clone(), c.state == ConnectionState::Active) };
        let Some(attempt) = attempt else { return };
        if !active || text.trim().is_empty() { return; }
        self.local_transcript(Speaker::Assistant, text.trim());
        if self.conv.peek().natural_voice {
            self.speak_natural(attempt, text.trim().to_string());
        } else {
            self.bridge_send(json!({"cmd": "speak", "attempt": attempt, "text": text.trim()}));
        }
    }

    /// Downloads (first time) and loads the on-device voice, then opens the microphone.
    fn prepare_natural_voice(self, attempt: String) {
        let language_id = self.language().id;
        let task = spawn_forever(async move {
            let conv = self.conv;
            let megabytes = voice::missing_megabytes(language_id);
            if megabytes > 0 {
                conv.write_unchecked().preparing = Some(format!("Downloading voices… 0% of {megabytes} MB"));
                let progress = move |step: voice::Progress| {
                    let text = match step {
                        voice::Progress::Downloading(fraction) => format!("Downloading voices… {}% of {megabytes} MB", (fraction * 100.0).floor() as i64),
                        voice::Progress::Unpacking => "Unpacking voices…".to_string(),
                    };
                    let mut c = conv.write_unchecked();
                    if c.preparing.as_ref() != Some(&text) { c.preparing = Some(text); }
                };
                if let Err(error) = voice::install(language_id, progress).await {
                    if self.current_attempt(&attempt) { self.connection_failed(error); }
                    return;
                }
            }
            if !self.current_attempt(&attempt) { return; }
            conv.write_unchecked().preparing = Some("Warming up the voice…".into());
            let result = voice::prepare(language_id).await;
            if !self.current_attempt(&attempt) { return; }
            {
                let mut c = conv.write_unchecked();
                c.preparing = None;
                c.prepare_task = None;
            }
            match result {
                Ok(()) => self.bridge_send(json!({"cmd": "local_start", "attempt": attempt, "lang": self.language().locale, "engine": "natural"})),
                Err(error) => self.connection_failed(error),
            }
        });
        self.conv.write_unchecked().prepare_task = Some(task);
    }

    /// Streams the reply to the page sentence by sentence; listening pauses until playback ends.
    fn speak_natural(self, attempt: String, text: String) {
        let previous = self.conv.write_unchecked().speech_task.take();
        voice::reset_listening();
        self.bridge_send(json!({"cmd": "play_begin", "attempt": attempt}));
        let task = spawn_forever(async move {
            // A newer reply replaces one that is still being generated.
            if let Some(previous) = previous { previous.cancel(); }
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let generation = voice::speak(text, 0.95, tx);
            let forward = async {
                while let Some((samples, rate)) = rx.recv().await {
                    self.bridge_send(json!({"cmd": "play", "attempt": attempt, "rate": rate, "data": encode_pcm(&samples)}));
                }
            };
            let (result, _) = tokio::join!(generation, forward);
            if let Err(error) = result {
                self.conv.write_unchecked().notice = Some(error);
            }
            self.bridge_send(json!({"cmd": "play_end", "attempt": attempt}));
            self.conv.write_unchecked().speech_task = None;
        });
        self.conv.write_unchecked().speech_task = Some(task);
    }

    /// Asks the teacher for Mural's next spoken reply. One turn runs at a time; a request made
    /// meanwhile runs right after it, and notes wait for the next turn.
    fn local_turn(self) {
        let conv = self.conv;
        if conv.peek().local_turn.is_some() { conv.write_unchecked().local_turn_again = true; return; }
        let task = spawn_forever(async move {
            let conv = self.conv;
            // Lets several updates from one action arrive before the request goes out.
            sleep(0.1).await;
            let (messages, session_id) = {
                let mut c = conv.write_unchecked();
                let Some(session) = c.session.clone() else { c.local_turn = None; return };
                let mut messages = vec![json!({"role": "system", "content": c.instructions})];
                let passages = session.passages();
                // Recent turns only: long histories slow small local models and overflow their context.
                for passage in passages.iter().skip(passages.len().saturating_sub(16)) {
                    let role = if passage.speaker == Speaker::User { "user" } else { "assistant" };
                    messages.push(json!({"role": role, "content": prefix(&passage.text(), 1200)}));
                }
                let notes: Vec<String> = c.local_notes.drain(..).collect();
                let notes: Vec<&String> = notes.iter().skip(notes.len().saturating_sub(6)).collect();
                if !notes.is_empty() {
                    // Small models return nothing unless the conversation ends on the learner's words,
                    // so notes go just before the learner's latest message.
                    let joined = notes.iter().map(|n| n.as_str()).collect::<Vec<_>>().join("\n");
                    let learner_last = messages.last().map(|m| m["role"] == "user").unwrap_or(false);
                    if learner_last {
                        messages.insert(messages.len() - 1, json!({"role": "system", "content": joined}));
                    } else {
                        messages.push(json!({"role": "user", "content": joined}));
                    }
                }
                // Nothing new to answer. Ending on Mural's own words makes the model continue them,
                // and it soon starts speaking the learner's part too.
                if messages.last().map(|m| m["role"] != "user").unwrap_or(true) {
                    c.local_turn = None;
                    c.local_turn_again = false;
                    drop(c);
                    self.local_listen();
                    return;
                }
                (messages, session.id)
            };
            conv.write_unchecked().working = true;
            let result = api::converse(messages).await;
            let current = {
                let c = conv.peek();
                c.state == ConnectionState::Active && c.local_voice && c.session.as_ref().map(|s| s.id == session_id).unwrap_or(false)
            };
            let again = {
                let mut c = conv.write_unchecked();
                c.local_turn = None;
                c.working = false;
                std::mem::take(&mut c.local_turn_again)
            };
            if !current { return; }
            match result {
                Ok(result) => {
                    self.add_usage(result.usage);
                    self.local_say(&result.text);
                }
                Err(error) => {
                    conv.write_unchecked().notice = Some(format!("Mural couldn’t reply. {error}"));
                    if !again { self.local_listen(); }
                }
            }
            if again { self.local_turn(); }
        });
        conv.write_unchecked().local_turn = Some(task);
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
        if let Some(t) = c.check_task.take() { t.cancel(); }
        c.check_again = false;
        c.language_generation += 1;
        c.session = None;
        c.selected_theme = None;
        c.scenario = None;
        c.beats_covered.clear();
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

    /// Which unfinished steps the conversation has now reached. A capable teacher reads the whole
    /// checklist in one request, so steps count in whatever order they happened; a small model on
    /// this computer answers yes or no about each of the next few instead.
    async fn check_beats(snapshot: SessionRecord, scenario: &'static Scenario, done: Vec<usize>) -> Vec<usize> {
        let transcript = teaching::scenario_transcript(&snapshot);
        if provider::current().teacher != provider::Teacher::OllamaLocal {
            let instructions = teaching::beat_checklist(scenario);
            let mut answered = api::respond(&instructions, &transcript, None, false).await;
            if answered.is_err() {
                // Usually a busy server: a step missed now would wait for the next turn.
                sleep(3.0).await;
                answered = api::respond(&instructions, &transcript, None, false).await;
            }
            return match answered {
                Ok(result) => {
                    if tracing() { eprintln!("{:.1} checklist: {}", uptime(), result.text.trim()); }
                    parse_checklist(&result.text, scenario.beats.len(), &done)
                }
                Err(error) => {
                    if tracing() { eprintln!("{:.1} checklist failed: {error}", uptime()); }
                    vec![]
                }
            };
        }
        let candidates = beat_window(scenario.beats.len(), &done);
        let checks = candidates.iter().map(|&index| {
            let transcript = &transcript;
            async move {
                let answered = api::respond(&teaching::beat_check(scenario, index), transcript, None, false).await;
                let reached = match answered {
                    // One word is asked for; a sentence that starts with it is just as clear.
                    Ok(result) => {
                        let said = result.text.trim().to_lowercase();
                        said.starts_with("yes") || said.starts_with("ja")
                    }
                    Err(error) => {
                        if tracing() { eprintln!("{:.1} beat check failed: {error}", uptime()); }
                        false
                    }
                };
                if tracing() { eprintln!("{:.1} beat {index} reached: {reached}", uptime()); }
                reached
            }
        });
        let answers = futures_util::future::join_all(checks).await;
        // A few small steps in one turn is plausible; more usually means the model is agreeing with itself.
        candidates.into_iter().zip(answers).filter(|(_, reached)| *reached).map(|(index, _)| index).take(BEATS_PER_TURN).collect()
    }

    /// Folds the beats a turn reached into the arc, and says so when the last one lands.
    fn scenario_progress(self, covered: Vec<usize>) {
        let Some(scenario) = self.conv.peek().scenario else { return };
        let (added, done) = {
            let mut c = self.conv.write_unchecked();
            let before = c.beats_covered.len();
            for index in covered {
                if !c.beats_covered.contains(&index) { c.beats_covered.push(index); }
            }
            c.beats_covered.sort_unstable();
            (c.beats_covered.len() > before, c.beats_covered.len() >= scenario.beats.len())
        };
        if !added { return; }
        if done && !dialog_store::current().get(&scenario.id).completed {
            self.conv.write_unchecked().notice = Some(format!("You’ve been through everything in “{}”.", scenario.title));
        }
        self.save_scenario_progress();
    }

    /// Records the run so the Dialog list can show what has been played.
    fn save_scenario_progress(self) {
        let (Some(scenario), beats) = ({ let c = self.conv.peek(); (c.scenario, c.beats_covered.len()) }) else { return };
        let total = scenario.beats.len();
        if let Err(error) = dialog_store::update(|r| { r.record(&scenario.id, beats, total, Date::now()); }) {
            self.conv.write_unchecked().notice = Some(error);
        }
    }

    /// Chooses a scenario to play. A conversation already running is moved to it, rather than
    /// carrying on with the old situation while the screen shows the new one.
    pub fn choose_scenario(self, scenario: Option<&'static Scenario>) {
        let conv = self.conv;
        let (running, has_session) = { let c = conv.peek(); (c.is_running(), c.session.is_some()) };
        if !running && has_session { self.reset_conversation(); }
        {
            let mut c = conv.write_unchecked();
            c.scenario = scenario;
            c.beats_covered.clear();
            c.wrapping_up = false;
            if scenario.is_some() { c.selected_theme = None; c.pending_topic = None; }
        }
        if conv.peek().state != ConnectionState::Active { return; }
        let language = self.language();
        {
            let mut c = conv.write_unchecked();
            if let Some(s) = c.session.as_mut() {
                s.theme_id = scenario.map(|x| x.id.clone());
                s.title = scenario.map(|x| x.title.clone()).unwrap_or_else(|| language.default_title());
            }
        }
        self.append("instructions", &teaching::switch_scenario(language, scenario), None);
        self.local_speak_now();
        self.save();
    }


    // ---------- Assessment ----------

    async fn assess(snapshot: SessionRecord, passage: Passage) -> Result<(Assessment, Usage), String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AssessmentResult { outcome: Outcome, suggested_level: i64, next_goal: String, capability: String, words: Vec<WordProposal>,
            #[serde(default)] errors: Vec<ErrorProposal> }
        let language = languages::module(&snapshot.language_id).ok_or("unsupported language")?;
        let result = api::respond(&teaching::assessment(language), &teaching::context(&snapshot, Some(&passage)),
            Some(api::assessment_schema(language)), false).await.map_err(|e| e.to_string())?;
        let decoded: AssessmentResult = match serde_json::from_str(&result.text) {
            Ok(decoded) => decoded,
            Err(error) => {
                if tracing() { eprintln!("{:.1} assess could not read the reply: {error} in {}", uptime(), prefix(&result.text, 160)); }
                return Err(error.to_string());
            }
        };
        let proposed = Assessment {
            passage_id: passage.id.clone(), revision_key: passage.revision_key(), outcome: decoded.outcome,
            suggested_level: decoded.suggested_level, next_goal: decoded.next_goal, capability: decoded.capability,
            words: decoded.words, errors: decoded.errors, created_at: Date::now(),
            context: snapshot.theme_id.clone().unwrap_or_else(|| "free".into()),
        };
        Ok((proposed, result.usage))
    }

    /// Assesses the latest turn. A model on this computer can take a minute over it, so a running
    /// assessment is never cancelled for a newer one: the newer turn waits and runs after it.
    fn schedule_assessment(self) {
        let conv = self.conv;
        if conv.peek().assessment_task.is_some() {
            conv.write_unchecked().assessment_again = true;
            return;
        }
        let task = spawn_forever(async move {
            sleep(1.5).await;
            self.assess_turn().await;
            let again = {
                let mut c = self.conv.write_unchecked();
                c.assessment_task = None;
                std::mem::take(&mut c.assessment_again)
            };
            if again { self.schedule_assessment(); }
        });
        conv.write_unchecked().assessment_task = Some(task);
    }

    /// Checks the scenario's steps after a learner turn, in the checker's own session. A turn that
    /// arrives meanwhile is checked once this one finishes.
    fn schedule_check(self) {
        let conv = self.conv;
        if conv.peek().scenario.is_none() { return; }
        if conv.peek().check_task.is_some() {
            conv.write_unchecked().check_again = true;
            return;
        }
        let task = spawn_forever(async move {
            sleep(1.0).await;
            self.check_turn().await;
            let again = {
                let mut c = self.conv.write_unchecked();
                c.check_task = None;
                std::mem::take(&mut c.check_again)
            };
            if again { self.schedule_check(); }
        });
        conv.write_unchecked().check_task = Some(task);
    }

    async fn check_turn(self) {
        // Read everything out before awaiting: a borrow held across an await blocks every write.
        let (snapshot, scenario, done) = {
            let c = self.conv.peek();
            let (Some(snapshot), Some(scenario)) = (c.session.clone(), c.scenario) else { return };
            (snapshot, scenario, c.beats_covered.clone())
        };
        if done.len() >= scenario.beats.len() { return; }
        let covered = Self::check_beats(snapshot.clone(), scenario, done).await;
        // The conversation may have ended meanwhile, which is fine; a different one is not.
        let same = {
            let c = self.conv.peek();
            c.session.as_ref().map(|s| s.id == snapshot.id).unwrap_or(false) && c.scenario.map(|s| s.id == scenario.id).unwrap_or(false)
        };
        if same { self.scenario_progress(covered); }
    }

    async fn assess_turn(self) {
        {
            let conv = self.conv;
            let (snapshot, passage) = {
                let c = conv.peek();
                let Some(snapshot) = c.session.clone() else { return };
                let Some(p) = c.user_passage() else { return };
                if char_count(&p.text()) < 3 || p.revision_key() == c.last_assessment_key || c.state != ConnectionState::Active { return; }
                (snapshot, p)
            };
            let Some(target) = languages::module(&snapshot.language_id) else { return };
            if tracing() { eprintln!("{:.1} assessing passage {}", uptime(), prefix(&passage.text(), 40)); }
            // The passage remains saved without unverified learning evidence.
            let assessed = Self::assess(snapshot.clone(), passage.clone()).await;
            if let Err(error) = &assessed {
                if tracing() { eprintln!("{:.1} assessment failed: {error}", uptime()); }
            }
            let Ok((assessment, usage)) = assessed else { return };
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
            // One teaching action for the next reply, chosen from the evidence this turn produced.
            let next = conv.write_unchecked().plan.next(&validated, &learner, &snapshot.language_id, Date::now());
            if let Some(next) = next {
                self.append("instructions", &teaching::teaching_move(target, &next), None);
            }
            self.append("thinking", &format!(
                "Teaching context, not spoken text: challenge {}/5 in {}. Next goal: {}. Revisit naturally: {}.",
                learner.challenge, target.name, learner.next_goal, learner.due_words(3)), None);
        }
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
                self.schedule_check();
                self.save();
                true
            }
        }
    }

    /// A short gloss for the word list, asked for separately so the list stays readable: the
    /// explanation shown on screen is prose, and its first sentence is not always the meaning.
    pub async fn gloss(self, word: String, sentence: String) -> Result<String, String> {
        if !self.has_ai_consent() { return Err(AI_CONSENT_REQUIRED.into()); }
        let meaning_language = self.store.peek().preferences().meaning_language.clone();
        let result = api::respond(&teaching::gloss(self.language(), &meaning_language),
            &format!("Selected: {word}\nSentence: {sentence}"), None, false)
            .await.map_err(|e| e.to_string())?;
        self.add_usage(result.usage);
        Ok(result.text.trim().trim_end_matches('.').to_string())
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
        if provider::current().teacher.is_ollama() && !OLLAMA.has_key() {
            return Err("Topic search uses Ollama’s web search, which needs a free Ollama account key. Add one in Settings → Teacher.".into());
        }
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
            self.local_speak_now();
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
            // "local": true drives the on-device path, which otherwise needs a microphone.
            c.local_voice = value["local"].as_bool().unwrap_or(false);
            c.natural_voice = value["natural"].as_bool().unwrap_or(false);
            return;
        }
        // Stands in for the assessor reporting which scenario beats have been reached.
        if value["type"] == "mural.debug.beats" {
            let covered: Vec<usize> = value["beats"].as_array().map(|a| {
                a.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect()
            }).unwrap_or_default();
            let scenario = self.conv.peek().scenario;
            let covered = covered.into_iter().filter(|i| scenario.map(|s| s.valid_beat(*i)).unwrap_or(false)).collect();
            self.scenario_progress(covered);
            return;
        }
        // Stands in for the learner repeating a phrase in the Shadow tab.
        if value["type"] == "mural.debug.shadow_heard" {
            if let Some(text) = value["text"].as_str() {
                self.shadow.write_unchecked().heard = vec![text.to_string()];
                self.shadow_score();
            }
            return;
        }
        // Stands in for an utterance the on-device recogniser finished, which needs a microphone.
        if value["type"] == "mural.debug.heard" {
            if let Some(text) = value["text"].as_str() { self.local_heard(text); }
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
                    errors: vec![],
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
