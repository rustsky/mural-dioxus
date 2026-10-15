//! Natural on-device voice for Ollama teachers: Silero VAD finds each utterance, a recogniser
//! chosen per language transcribes it, and Kokoro or Piper speaks Mural's reply. Everything runs
//! through sherpa-onnx on this computer (macOS and Windows); models download once per language.
//!
//! The recognisers were picked by measuring native and accented recordings of Shadow phrases
//! (`benchmark_recognisers` below) for at least 60% of words heard in under a second: a model that
//! only knows the target language cannot drift into a neighbouring one, which is how multilingual
//! recognisers failed learners (Spanish heard as Portuguese, Chinese and Norwegian in other scripts).
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use sherpa_onnx::{
    GenerationConfig, OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
    OfflineTransducerModelConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig, OfflineWhisperModelConfig, SileroVadModelConfig, VadModelConfig, VoiceActivityDetector,
};

use super::store::LearningStore;

const RELEASES: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";
pub const SAMPLE_RATE: i32 = 16_000;
const THREADS: i32 = 4;

/// One downloadable model: a `.tar.bz2` release archive holding a folder of the same name, or a single file.
#[derive(Debug, PartialEq)]
pub struct Pack {
    pub name: &'static str,
    /// Under `RELEASES`, or a full URL.
    url: &'static str,
    pub megabytes: u32,
    archive: bool,
}

const VAD: Pack = Pack { name: "silero_vad.onnx", url: "asr-models/silero_vad.onnx", megabytes: 1, archive: false };
const PARAKEET: Pack = Pack { name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8", url: "asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2", megabytes: 464, archive: true };
/// German-only NeMo FastConformer.
const FASTCONFORMER_DE: Pack = Pack { name: "sherpa-onnx-nemo-transducer-stt_de_fastconformer_hybrid_large_pc-int8", url: "asr-models/sherpa-onnx-nemo-transducer-stt_de_fastconformer_hybrid_large_pc-int8.tar.bz2", megabytes: 103, archive: true };
const SENSEVOICE: Pack = Pack { name: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09", url: "asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2", megabytes: 158, archive: true };
/// The archive also holds full-precision weights, which are skipped when unpacking.
const WHISPER_SMALL: Pack = Pack { name: "sherpa-onnx-whisper-small", url: "asr-models/sherpa-onnx-whisper-small.tar.bz2", megabytes: 609, archive: true };
/// The National Library of Norway's NB-Whisper base (Apache-2.0). Mural never downloads it: each
/// learner who wants it builds it with `scripts/convert-nb-whisper.sh`, which installs it here.
const NB_WHISPER: Pack = Pack { name: "nb-whisper-base-int8", url: "", megabytes: 0, archive: true };
const KOKORO: Pack = Pack { name: "kokoro-multi-lang-v1_0", url: "tts-models/kokoro-multi-lang-v1_0.tar.bz2", megabytes: 334, archive: true };
const PIPER_DE: Pack = Pack { name: "vits-piper-de_DE-thorsten-high", url: "tts-models/vits-piper-de_DE-thorsten-high.tar.bz2", megabytes: 110, archive: true };
const PIPER_NO: Pack = Pack { name: "vits-piper-no_NO-talesyntese-medium", url: "tts-models/vits-piper-no_NO-talesyntese-medium.tar.bz2", megabytes: 64, archive: true };

#[derive(Debug, PartialEq)]
enum Listener {
    /// Parakeet TDT v3: fast, 25 European languages, picks the language itself.
    Parakeet,
    /// A NeMo transducer that knows one language.
    Transducer(&'static Pack),
    /// Whisper told which language to expect: model folder, file prefix and Whisper language code.
    Whisper(&'static Pack, &'static str, &'static str),
    SenseVoice,
}

impl Listener {
    fn pack(&self) -> &'static Pack {
        match self {
            Listener::Parakeet => &PARAKEET,
            Listener::Transducer(pack) | Listener::Whisper(pack, ..) => pack,
            Listener::SenseVoice => &SENSEVOICE,
        }
    }
}

#[derive(Debug, PartialEq)]
enum Speaker {
    /// Kokoro speaker id and espeak language.
    Kokoro(i32, &'static str),
    /// Piper model folder and file.
    Piper(&'static Pack, &'static str),
}

#[derive(Debug, PartialEq)]
struct Setup {
    listener: Listener,
    /// Hears an utterance again when the listener heard nothing, or (on macOS) another language.
    second: Option<Listener>,
    speaker: Speaker,
}

const SMALL: fn(&'static str) -> Listener = |code| Listener::Whisper(&WHISPER_SMALL, "small", code);

fn setup(language_id: &str) -> Option<Setup> {
    use Listener::*;
    let (listener, second, speaker) = match language_id {
        "en" => (Parakeet, None, Speaker::Kokoro(3, "en-us")),                     // af_heart
        "es" => (Parakeet, Some(SMALL("es")), Speaker::Kokoro(28, "es")),          // ef_dora
        "fr" => (Parakeet, Some(SMALL("fr")), Speaker::Kokoro(30, "fr")),          // ff_siwis; espeak-ng has no "fr-fr" voice
        "it" => (Parakeet, Some(SMALL("it")), Speaker::Kokoro(35, "it")),          // if_sara
        "pt" => (SMALL("pt"), None, Speaker::Kokoro(42, "pt-br")),                 // pf_dora
        "zh" => (SenseVoice, None, Speaker::Kokoro(47, "zh")),                     // zf_xiaoxiao
        "de" => (Transducer(&FASTCONFORMER_DE), None, Speaker::Piper(&PIPER_DE, "de_DE-thorsten-high.onnx")),
        // NB-Whisper heard Norwegian learners far better (65% of words against 43%), but only a
        // learner who built it has it; otherwise Whisper small, told it is Norwegian.
        "nb" => (if path(&NB_WHISPER).exists() { Whisper(&NB_WHISPER, "nb-base", "no") } else { SMALL("no") },
                 None, Speaker::Piper(&PIPER_NO, "no_NO-talesyntese-medium.onnx")),
        _ => return None,
    };
    Some(Setup { listener, second, speaker })
}

fn packs(language_id: &str) -> Vec<&'static Pack> {
    let Some(s) = setup(language_id) else { return vec![] };
    let speaker = match s.speaker { Speaker::Kokoro(..) => &KOKORO, Speaker::Piper(pack, _) => pack };
    let mut packs = vec![&VAD, s.listener.pack()];
    packs.extend(s.second.map(|l| l.pack()));
    packs.push(speaker);
    packs
}

pub fn supported(language_id: &str) -> bool { setup(language_id).is_some() }

pub fn directory() -> PathBuf { LearningStore::directory().join("voice-models") }

fn path(pack: &Pack) -> PathBuf { directory().join(pack.name) }

/// Megabytes still to download before this language can use the natural voice.
pub fn missing_megabytes(language_id: &str) -> u32 {
    packs(language_id).into_iter().filter(|p| !path(p).exists()).map(|p| p.megabytes).sum()
}

fn download_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(60))
            .build()
            .expect("HTTP client")
    })
}

/// Runs blocking work off the UI thread.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || { let _ = tx.send(work()); });
    rx.await.map_err(|_| "The voice engine stopped unexpectedly.".to_string())
}

/// Install progress: the fraction of all downloads received, or that a download is being unpacked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Progress { Downloading(f64), Unpacking }

/// Downloads whatever this language still needs.
pub async fn install(language_id: &str, progress: impl Fn(Progress)) -> Result<(), String> {
    // A drill left and reopened mid-download asks again; two installs would write the same part
    // and staging files. The second waits, showing the first one's progress, then finds what the
    // first already put in place.
    static INSTALLING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    static LATEST: std::sync::Mutex<Option<Progress>> = std::sync::Mutex::new(None);
    let _one_at_a_time = loop {
        if let Ok(guard) = INSTALLING.try_lock() { break guard; }
        if let Some(step) = *LATEST.lock().unwrap() { progress(step); }
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    let progress = |step: Progress| { *LATEST.lock().unwrap() = Some(step); progress(step); };
    let needed: Vec<&Pack> = packs(language_id).into_iter().filter(|p| !path(p).exists()).collect();
    let total: f64 = needed.iter().map(|p| p.megabytes as f64).sum::<f64>().max(1.0);
    let mut done = 0.0;
    std::fs::create_dir_all(directory()).map_err(|_| "Mural couldn’t create its voice folder.".to_string())?;
    for pack in needed {
        let base = done;
        download(pack, |step| progress(match step {
            Progress::Downloading(fraction) => Progress::Downloading((base + fraction * pack.megabytes as f64) / total),
            other => other,
        })).await?;
        done += pack.megabytes as f64;
    }
    *LATEST.lock().unwrap() = None;
    Ok(())
}

async fn download(pack: &'static Pack, progress: impl Fn(Progress)) -> Result<(), String> {
    let failed = || format!("The voice download ({}) didn’t finish. Check your connection and try again.", pack.name);
    let part = directory().join(format!(".{}.part", pack.name));
    let url = if pack.url.starts_with("https://") { pack.url.to_string() } else { format!("{RELEASES}/{}", pack.url) };
    let mut response = download_client().get(url).send().await.map_err(|_| failed())?;
    if !response.status().is_success() { return Err(failed()); }
    let length = response.content_length();
    let mut file = std::fs::File::create(&part).map_err(|_| failed())?;
    let mut received: u64 = 0;
    loop {
        match response.chunk().await {
            Ok(Some(bytes)) => {
                std::io::Write::write_all(&mut file, &bytes).map_err(|_| failed())?;
                received += bytes.len() as u64;
                if let Some(total) = length { progress(Progress::Downloading(received as f64 / total as f64)); }
            }
            Ok(None) => break,
            Err(_) => { let _ = std::fs::remove_file(&part); return Err(failed()); }
        }
    }
    drop(file);
    if length.is_some_and(|total| total != received) { let _ = std::fs::remove_file(&part); return Err(failed()); }
    let target = path(pack);
    let archive = pack.archive;
    let cleanup = part.clone();
    if archive { progress(Progress::Unpacking); }
    let result = blocking(move || -> std::io::Result<()> {
        if !archive { return std::fs::rename(&part, &target); }
        let staging = directory().join(format!(".{}.staging", pack.name));
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging)?;
        let reader = bzip2::read::BzDecoder::new(std::io::BufReader::new(std::fs::File::open(&part)?));
        let mut tar = tar::Archive::new(reader);
        for entry in tar.entries()? {
            let mut entry = entry?;
            // Sample recordings are not needed at runtime, nor full-precision weights beside int8 ones.
            let path = entry.path()?.into_owned();
            if path.components().any(|c| c.as_os_str() == "test_wavs") { continue; }
            let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            if file.ends_with("coder.onnx") && !file.contains("int8") { continue; }
            entry.unpack_in(&staging)?;
        }
        std::fs::rename(staging.join(pack.name), &target)?;
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::remove_file(&part)
    }).await?;
    result.map_err(|error| {
        eprintln!("voice model {} couldn’t be unpacked: {error}", pack.name);
        let _ = std::fs::remove_file(&cleanup);
        failed()
    })
}

// ---------- Engine ----------

struct Loaded {
    language_id: String,
    recognizer: OfflineRecognizer,
    second: Option<OfflineRecognizer>,
    tts: OfflineTts,
    sid: i32,
}

impl Loaded {
    fn transcribe(&self, audio: &[f32]) -> String {
        let heard = decode(&self.recognizer, audio);
        match &self.second {
            Some(second) if wrong_language(&self.language_id, &heard) => {
                let again = decode(second, audio);
                if again.is_empty() { heard } else { again }
            }
            _ => heard,
        }
    }
}

fn decode(recognizer: &OfflineRecognizer, audio: &[f32]) -> String {
    let stream = recognizer.create_stream();
    stream.accept_waveform(SAMPLE_RATE, audio);
    recognizer.decode(&stream);
    stream.get_result().map(|r| r.text.trim().to_string()).unwrap_or_default()
}

/// Nothing heard, or confidently another language: a multilingual recogniser guessed wrong.
/// Language detection is macOS-only, so elsewhere only silence brings in the second recogniser.
fn wrong_language(language_id: &str, heard: &str) -> bool {
    if heard.is_empty() { return true; }
    crate::engine::langdetect::detect(heard)
        .is_some_and(|(detected, confidence)| confidence > 0.5 && !detected.starts_with(language_id))
}

#[derive(Default)]
struct State {
    loaded: Option<Loaded>,
    vad: Option<VoiceActivityDetector>,
    /// Recent audio, so each utterance can start a little before the VAD noticed speech.
    history: Vec<f32>,
    /// Stream position of `history[0]`.
    history_start: usize,
    /// Whether anything was transcribed since listening restarted, i.e. during this hold.
    heard_any: bool,
}

/// Shorter than this is a breath, a click or a chair, and a multilingual recogniser will turn it
/// into a confident sentence in the wrong language. The VAD already holds normal segments to
/// 0.3s; this guards the flush on release, which has no such floor.
const MIN_SPEECH: usize = (SAMPLE_RATE as usize) * 35 / 100;

/// Silero reports speech slightly late; this much earlier audio keeps the first word intact.
const LEAD_IN: usize = SAMPLE_RATE as usize * 4 / 5;
const HISTORY: usize = SAMPLE_RATE as usize * 40;

impl State {
    fn restart_listening(&mut self) {
        if let Some(vad) = &self.vad { vad.reset(); }
        self.history.clear();
        self.history_start = 0;
        self.heard_any = false;
    }

    /// The learner let go: whatever the VAD still holds, or, when it found no speech at all in
    /// the hold, the whole hold. A short "Bom dia!" can fall under the VAD's speech threshold,
    /// yet the learner plainly said something; quiet holds stay empty, as recognisers invent
    /// words for silence.
    fn finish_turn(&mut self) -> Vec<String> {
        let Some(vad) = &self.vad else { return vec![] };
        vad.flush();
        let heard = self.drain();
        if !heard.is_empty() || self.heard_any { return heard; }
        let Some(loaded) = &self.loaded else { return heard };
        let loudness = (self.history.iter().map(|s| s * s).sum::<f32>() / self.history.len().max(1) as f32).sqrt();
        if self.history.len() < MIN_SPEECH || loudness < 0.01 { return heard; }
        let text = loaded.transcribe(&self.history);
        if std::env::var("MURAL_TRACE").is_ok() { keep_utterance(&self.history, &text); }
        if text.is_empty() { vec![] } else { self.heard_any = true; vec![text] }
    }
}

type Job = Box<dyn FnOnce(&mut State) + Send>;

/// All models live on one worker thread, so recognition and speech never run at the same time.
fn worker() -> &'static mpsc::Sender<Job> {
    static WORKER: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new().name("mural-voice".into()).spawn(move || {
            let mut state = State::default();
            for job in rx { job(&mut state); }
        }).expect("voice worker");
        tx
    })
}

async fn run<T: Send + 'static>(job: impl FnOnce(&mut State) -> T + Send + 'static) -> Result<T, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    worker().send(Box::new(move |state| { let _ = tx.send(job(state)); })).map_err(|_| "The voice engine stopped.".to_string())?;
    rx.await.map_err(|_| "The voice engine stopped unexpectedly.".to_string())
}

fn file(pack: &Pack, name: &str) -> Option<String> { Some(path(pack).join(name).to_string_lossy().into_owned()) }

fn recognizer_config(listener: &Listener) -> OfflineRecognizerConfig {
    let mut config = OfflineRecognizerConfig::default();
    let model = &mut config.model_config;
    let pack = listener.pack();
    match listener {
        Listener::Parakeet | Listener::Transducer(_) => {
            model.transducer = OfflineTransducerModelConfig {
                encoder: file(pack, "encoder.int8.onnx"),
                decoder: file(pack, "decoder.int8.onnx"),
                joiner: file(pack, "joiner.int8.onnx"),
            };
            model.tokens = file(pack, "tokens.txt");
            model.model_type = Some("nemo_transducer".into());
        }
        Listener::Whisper(_, prefix, code) => {
            model.whisper = OfflineWhisperModelConfig {
                encoder: file(pack, &format!("{prefix}-encoder.int8.onnx")),
                decoder: file(pack, &format!("{prefix}-decoder.int8.onnx")),
                language: Some((*code).into()),
                task: Some("transcribe".into()),
                tail_paddings: -1,
                ..Default::default()
            };
            model.tokens = file(pack, &format!("{prefix}-tokens.txt"));
        }
        Listener::SenseVoice => {
            model.sense_voice = OfflineSenseVoiceModelConfig { model: file(pack, "model.int8.onnx"), language: Some("zh".into()), use_itn: true };
            model.tokens = file(pack, "tokens.txt");
        }
    }
    model.num_threads = THREADS;
    config
}

fn load(language_id: &str) -> Result<Loaded, String> {
    let s = setup(language_id).ok_or("The natural voice doesn’t support this language yet.")?;
    let failed = "Mural couldn’t load its speech recognizer. Delete the voice models in Settings and try again.";
    let recognizer = OfflineRecognizer::create(&recognizer_config(&s.listener)).ok_or(failed)?;
    let second = match &s.second {
        Some(listener) => Some(OfflineRecognizer::create(&recognizer_config(listener)).ok_or(failed)?),
        None => None,
    };
    let (model, sid) = match s.speaker {
        Speaker::Kokoro(sid, lang) => (OfflineTtsModelConfig {
            kokoro: OfflineTtsKokoroModelConfig {
                model: file(&KOKORO, "model.onnx"),
                voices: file(&KOKORO, "voices.bin"),
                tokens: file(&KOKORO, "tokens.txt"),
                data_dir: file(&KOKORO, "espeak-ng-data"),
                dict_dir: file(&KOKORO, "dict"),
                lexicon: Some(format!("{},{}", file(&KOKORO, "lexicon-us-en.txt").unwrap_or_default(), file(&KOKORO, "lexicon-zh.txt").unwrap_or_default())),
                lang: Some(lang.into()),
                length_scale: 1.0,
            },
            ..Default::default()
        }, sid),
        Speaker::Piper(pack, onnx) => (OfflineTtsModelConfig {
            vits: OfflineTtsVitsModelConfig {
                model: file(pack, onnx),
                tokens: file(pack, "tokens.txt"),
                data_dir: file(pack, "espeak-ng-data"),
                noise_scale: 0.667,
                noise_scale_w: 0.8,
                length_scale: 1.0,
                ..Default::default()
            },
            ..Default::default()
        }, 0),
    };
    let tts = OfflineTts::create(&OfflineTtsConfig {
        model: OfflineTtsModelConfig { num_threads: THREADS, ..model },
        max_num_sentences: 1,
        silence_scale: 0.2,
        ..Default::default()
    }).ok_or("Mural couldn’t load its voice. Delete the voice models in Settings and try again.")?;
    Ok(Loaded { language_id: language_id.into(), recognizer, second, tts, sid })
}

fn new_vad() -> Option<VoiceActivityDetector> {
    VoiceActivityDetector::create(&VadModelConfig {
        silero_vad: SileroVadModelConfig {
            model: Some(path(&VAD).to_string_lossy().into_owned()),
            threshold: 0.5,
            // Learners pause to think; a longer silence marks the end of a turn.
            min_silence_duration: 1.2,
            min_speech_duration: 0.3,
            window_size: 512,
            max_speech_duration: 30.0,
        },
        sample_rate: SAMPLE_RATE,
        num_threads: 1,
        ..Default::default()
    }, 60.0)
}

/// Loads the models for `language_id` (once) and starts listening afresh.
pub async fn prepare(language_id: &str) -> Result<(), String> {
    let language_id = language_id.to_string();
    run(move |state| {
        if state.loaded.as_ref().map(|l| l.language_id != language_id).unwrap_or(true) {
            state.loaded = None;
            state.loaded = Some(load(&language_id)?);
        }
        state.vad = Some(new_vad().ok_or("Mural couldn’t load its voice activity detector.")?);
        state.restart_listening();
        Ok(())
    }).await?
}

/// Frees the models' memory.
pub fn unload() {
    let _ = worker().send(Box::new(|state| { *state = State::default(); }));
}

/// Forgets partly heard audio, e.g. while Mural is speaking.
pub fn reset_listening() {
    let _ = worker().send(Box::new(|state| state.restart_listening()));
}

/// Ends the current utterance now, without waiting for silence: the learner said they are done.
pub async fn end_turn() -> Result<Vec<String>, String> {
    run(move |state| state.finish_turn()).await
}

/// Feeds 16 kHz microphone audio; returns the text of every utterance that just finished.
pub async fn hear(samples: Vec<f32>) -> Result<Vec<String>, String> {
    run(move |state| {
        if state.vad.is_none() || state.loaded.is_none() { return vec![] }
        state.history.extend_from_slice(&samples);
        if state.history.len() > HISTORY {
            let excess = state.history.len() - HISTORY;
            state.history.drain(..excess);
            state.history_start += excess;
        }
        if let Some(vad) = &state.vad { vad.accept_waveform(&samples); }
        state.drain()
    }).await
}

impl State {
    /// The text of every utterance the VAD has finished with.
    fn drain(&mut self) -> Vec<String> {
        let (Some(vad), Some(loaded)) = (&self.vad, &self.loaded) else { return vec![] };
        let mut heard = vec![];
        while let Some(segment) = vad.front() {
            if segment.samples().len() < MIN_SPEECH {
                vad.pop();
                continue;
            }
            let start = (segment.start().max(0) as usize).saturating_sub(LEAD_IN).max(self.history_start);
            let end = segment.start().max(0) as usize + segment.samples().len();
            let audio = self.history.get(start - self.history_start..end.saturating_sub(self.history_start).min(self.history.len()))
                .filter(|a| a.len() >= segment.samples().len())
                .unwrap_or(segment.samples());
            let text = loaded.transcribe(audio);
            if std::env::var("MURAL_TRACE").is_ok() { keep_utterance(audio, &text); }
            if !text.is_empty() { heard.push(text); }
            vad.pop();
        }
        self.heard_any |= !heard.is_empty();
        heard
    }
}

/// Tracing only: saves what the recogniser heard as a 16-bit WAV next to its transcript, so
/// recognisers can be compared on a learner's real voice rather than a synthetic one.
fn keep_utterance(audio: &[f32], text: &str) {
    let dir = LearningStore::directory().join("utterances");
    if std::fs::create_dir_all(&dir).is_err() { return; }
    let name = chrono::Local::now().format("%Y%m%d-%H%M%S%3f").to_string();
    let data: Vec<u8> = audio.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
    let mut wav = Vec::with_capacity(44 + data.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE as u32).to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE as u32 * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    let _ = std::fs::write(dir.join(format!("{name}.wav")), wav);
    let _ = std::fs::write(dir.join(format!("{name}.txt")), text);
}

/// Speaks `text`, sending each sentence's audio (samples, sample rate) as soon as it is ready.
/// Stops early when `chunks` is closed.
pub async fn speak(text: String, speed: f32, chunks: tokio::sync::mpsc::UnboundedSender<(Vec<f32>, i32)>) -> Result<(), String> {
    run(move |state| {
        let Some(loaded) = &state.loaded else { return Err("The voice isn’t ready.".to_string()) };
        let rate = loaded.tts.sample_rate();
        let config = GenerationConfig { sid: loaded.sid, speed, ..Default::default() };
        let sender = chunks.clone();
        let callback = move |samples: &[f32], _progress: f32| sender.send((samples.to_vec(), rate)).is_ok();
        loaded.tts.generate_with_config(&text, &config, Some(callback)).map(|_| ()).ok_or_else(|| "Mural couldn’t speak that reply.".to_string())
    }).await?
}

/// Deletes downloaded models.
pub fn remove_all() -> Result<(), String> {
    unload();
    let dir = directory();
    if !Path::new(&dir).exists() { return Ok(()); }
    std::fs::remove_dir_all(dir).map_err(|_| "Mural couldn’t delete the voice models.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_a_voice() {
        for id in ["nb", "es", "en", "fr", "de", "it", "pt", "zh"] {
            let p = packs(id);
            assert_eq!(p[0], &VAD, "{id}");
            assert!(p.len() == 3 || p.len() == 4, "{id}: listener, optional second listener, speaker");
        }
        assert!(packs("xx").is_empty());
        assert_eq!(packs("de"), vec![&VAD, &FASTCONFORMER_DE, &PIPER_DE]);
        assert_eq!(packs("zh")[1], &SENSEVOICE);
        // Never downloaded: NB-Whisper when the learner built it, Whisper small otherwise.
        assert!(packs("nb")[1] == &NB_WHISPER || packs("nb")[1] == &WHISPER_SMALL);
        // Parakeet guesses the language; where it guessed wrong in testing, forced Whisper hears it again.
        assert_eq!(packs("es"), vec![&VAD, &PARAKEET, &WHISPER_SMALL, &KOKORO]);
    }

    #[test]
    fn whisper_listens_for_the_language_being_learned() {
        assert!(matches!(setup("nb").unwrap().listener, Listener::Whisper(_, _, "no")));
        assert_eq!(setup("pt").unwrap().listener, Listener::Whisper(&WHISPER_SMALL, "small", "pt"));
        assert_eq!(setup("fr").unwrap().second, Some(Listener::Whisper(&WHISPER_SMALL, "small", "fr")));
    }

    #[test]
    fn silence_is_a_reason_to_listen_again() {
        assert!(wrong_language("es", ""));
        assert!(!wrong_language("es", "¿Dónde está la estación de tren?"));
    }

    /// Linear resampling is plenty for a recogniser test.
    fn resample(samples: &[f32], from: i32, to: i32) -> Vec<f32> {
        let ratio = from as f64 / to as f64;
        (0..(samples.len() as f64 / ratio) as usize).map(|i| {
            let x = i as f64 * ratio;
            let (j, f) = (x as usize, (x.fract()) as f32);
            let a = samples[j];
            let b = *samples.get(j + 1).unwrap_or(&a);
            a + (b - a) * f
        }).collect()
    }

    fn words(text: &str) -> Vec<String> {
        text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
    }

    /// Mural speaks a sentence in each language and the same recogniser the learner gets hears it
    /// back through the VAD, after downloading whatever is missing through the app's own install
    /// path: `MURAL_VOICE_LANGS=es,de cargo test --release -- --ignored round_trip`.
    #[test]
    #[ignore]
    fn round_trip() {
        let sentences = [
            ("en", "Hello, I would like a coffee with milk, please."),
            ("es", "Hola, quisiera un café con leche, por favor."),
            ("fr", "Bonjour, je voudrais un café au lait, s'il vous plaît."),
            ("it", "Buongiorno, vorrei un caffè con latte, per favore."),
            ("pt", "Olá, eu queria um café com leite, por favor."),
            ("de", "Hallo, ich hätte gern einen Kaffee mit Milch, bitte."),
            ("nb", "Hei, jeg vil gjerne ha en kaffe med melk, takk."),
            ("zh", "你好，我想要一杯加牛奶的咖啡。"),
        ];
        let only = std::env::var("MURAL_VOICE_LANGS").ok();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let mut failures = vec![];
        for (id, sentence) in sentences {
            if only.as_ref().is_some_and(|o| !o.split(',').any(|l| l == id)) { continue; }
            if missing_megabytes(id) > 0 {
                println!("{id}: downloading {} MB", missing_megabytes(id));
                if let Err(e) = runtime.block_on(install(id, |_| {})) { failures.push(format!("{id}: download: {e}")); continue; }
            }
            let loaded = match load(id) { Ok(l) => l, Err(e) => { failures.push(format!("{id}: load: {e}")); continue } };
            let audio = loaded.tts.generate_with_config(sentence, &GenerationConfig { sid: loaded.sid, speed: 0.95, ..Default::default() }, None::<fn(&[f32], f32) -> bool>);
            let Some(audio) = audio else { failures.push(format!("{id}: no speech generated")); continue };
            // A held turn: the microphone opens a moment before the learner speaks.
            let spoken = resample(audio.samples(), audio.sample_rate(), SAMPLE_RATE);
            let seconds = spoken.len() as f32 / SAMPLE_RATE as f32;
            let mut samples = vec![0.0; SAMPLE_RATE as usize / 2];
            samples.extend(spoken);
            samples.extend(std::iter::repeat_n(0.0, SAMPLE_RATE as usize * 2));
            let mut state = State { loaded: Some(loaded), vad: new_vad(), ..Default::default() };
            let mut heard = vec![];
            for chunk in samples.chunks(1600) {
                state.history.extend_from_slice(chunk);
                state.vad.as_ref().unwrap().accept_waveform(chunk);
                heard.extend(state.drain());
            }
            heard.extend(state.finish_turn());
            let text = heard.join(" ");
            let score = if id == "zh" {
                let want: Vec<char> = sentence.chars().filter(|c| c.is_alphanumeric()).collect();
                let got: Vec<char> = text.chars().filter(|c| c.is_alphanumeric()).collect();
                want.iter().filter(|c| got.contains(c)).count() as f32 / want.len() as f32
            } else {
                let (want, got) = (words(sentence), words(&text));
                want.iter().filter(|w| got.contains(w)).count() as f32 / want.len() as f32
            };
            println!("{id}: {seconds:.1}s spoken, {:.0}% of words heard\n    said:  {sentence}\n    heard: {text}", score * 100.0);
            if score < 0.7 { failures.push(format!("{id}: heard “{text}”")); }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// Runs recordings from `MURAL_VOICE_WAVS` through the app's own pipeline (VAD, recogniser and
    /// any second recogniser) as a learner's held turn would be, and scores them as Shadow does
    /// when the folder has a `manifest.tsv` (file, language, text). File names start with the language.
    #[test]
    #[ignore]
    fn recognise_wavs() {
        use crate::engine::shadow;
        let Ok(dir) = std::env::var("MURAL_VOICE_WAVS") else { return };
        let expected: std::collections::HashMap<String, String> = std::fs::read_to_string(format!("{dir}/manifest.tsv")).unwrap_or_default()
            .lines().filter_map(|l| { let mut p = l.splitn(3, '\t'); Some((p.next()?.to_string(), p.nth(1)?.to_string())) }).collect();
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "wav")).collect();
        files.sort();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let mut loaded: Option<Loaded> = None;
        let mut totals: std::collections::BTreeMap<String, (f64, usize, f64, f64)> = Default::default();
        for file in files {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            let id = name.split(['-', '.']).next().unwrap().to_string();
            if loaded.as_ref().map(|l| l.language_id != id).unwrap_or(true) {
                loaded = None;
                // The app's own download path, so this also checks the models arrive on this OS.
                if missing_megabytes(&id) > 0 {
                    println!("{id}: downloading {} MB", missing_megabytes(&id));
                    runtime.block_on(install(&id, |_| {})).unwrap_or_else(|e| panic!("{id}: {e}"));
                }
                loaded = Some(load(&id).unwrap_or_else(|e| panic!("{id}: {e}")));
            }
            let Some(wave) = sherpa_onnx::Wave::read(&file.to_string_lossy()) else { continue };
            // A microphone is already open before the learner speaks.
            let mut samples = vec![0.0; SAMPLE_RATE as usize];
            samples.extend(resample(wave.samples(), wave.sample_rate(), SAMPLE_RATE));
            samples.extend(std::iter::repeat_n(0.0, SAMPLE_RATE as usize * 2));
            let mut state = State { loaded: loaded.take(), vad: new_vad(), ..Default::default() };
            let mut heard = vec![];
            let mut took = 0.0;
            for chunk in samples.chunks(1600) {
                state.history.extend_from_slice(chunk);
                state.vad.as_ref().unwrap().accept_waveform(chunk);
                let started = std::time::Instant::now();
                heard.extend(state.drain());
                took += started.elapsed().as_secs_f64();
            }
            let started = std::time::Instant::now();
            heard.extend(state.finish_turn());
            took += started.elapsed().as_secs_f64();
            loaded = state.loaded.take();
            let heard = heard.join(" ");
            match expected.get(&name) {
                Some(text) => {
                    let score = shadow::score(text, &heard).score;
                    let t = totals.entry(id.clone()).or_default();
                    t.0 += score; t.1 += 1; t.2 += took; t.3 = t.3.max(took);
                    if score < 0.85 { println!("  {name}: “{text}” → “{heard}” ({:.0}%, {took:.2}s)", score * 100.0); }
                }
                None => println!("{name}: heard “{heard}” ({took:.2}s)"),
            }
        }
        for (id, (score, n, secs, slowest)) in totals {
            println!("{id}: {:.0}% of words heard over {n} clips, {:.2}s per utterance, slowest {slowest:.2}s", 100.0 * score / n as f64, secs / n as f64);
        }
    }

    /// Compares candidate recognisers on recorded clips (`manifest.tsv`: file, language, text),
    /// scored as Shadow scores a repeat. Research only: `MURAL_BENCH_CLIPS=<dir> MURAL_BENCH_MODELS=<dir>`.
    #[test]
    #[ignore]
    fn benchmark_recognisers() {
        use crate::engine::shadow;
        use sherpa_onnx::*;
        let (Ok(clips), Ok(models)) = (std::env::var("MURAL_BENCH_CLIPS"), std::env::var("MURAL_BENCH_MODELS")) else { return };
        let m = |dir: &str, f: &str| Some(format!("{models}/{dir}/{f}"));
        let app = |dir: &str, f: &str| Some(directory().join(dir).join(f).to_string_lossy().into_owned());
        let manifest = std::fs::read_to_string(format!("{clips}/manifest.tsv")).unwrap();
        let rows: Vec<(String, String, String)> = manifest.lines().filter_map(|l| {
            let mut p = l.splitn(3, '\t');
            Some((p.next()?.into(), p.next()?.into(), p.next()?.into()))
        }).collect();
        let candidates: Vec<(&str, Vec<&str>)> = vec![
            ("parakeet", vec!["en", "es", "fr", "it", "pt", "de"]),
            ("canary", vec!["en", "es", "fr", "de"]),
            ("moonshine-es", vec!["es"]),
            ("omni-300m", vec!["zh", "nb", "es"]),
            ("omni-1b", vec!["zh", "nb", "es"]),
            ("sensevoice", vec!["zh"]),
            ("moonshine-zh", vec!["zh"]),
            ("firered2-ctc", vec!["zh"]),
            ("whisper-small", vec!["en", "es", "fr", "it", "pt", "de", "nb", "zh"]),
            ("whisper-base", vec!["en", "es", "fr", "it", "pt", "de", "nb"]),
            ("whisper-tiny", vec!["nb"]),
            ("nb-whisper-base", vec!["nb"]),
            ("fastconformer-de", vec!["de"]),
            ("fastconformer-pt", vec!["pt"]),
            ("fastconformer-10", vec!["de", "en", "es", "fr", "it"]),
            ("nb-whisper-small", vec!["nb"]),
            ("whisper-turbo", vec!["en", "es", "fr", "it", "pt", "de", "nb", "zh"]),
            ("parakeet+whisper-small", vec!["en", "es", "fr", "it", "pt"]),
            ("fastconformer-pt+whisper-small", vec!["pt"]),
            ("fastconformer-de+whisper-small", vec!["de"]),
            ("sensevoice+whisper-small", vec!["zh"]),
            ("nb-whisper-base+nb-whisper-small", vec!["nb"]),
        ];
        let build = |name: &str, lang: &str| -> OfflineRecognizerConfig {
        let mut c = OfflineRecognizerConfig::default();
        let mc = &mut c.model_config;
        let whisper = |dir: &str, enc: &str, dec: &str, models_dir: bool| OfflineWhisperModelConfig {
            encoder: if models_dir { m(dir, enc) } else { app(dir, enc) },
            decoder: if models_dir { m(dir, dec) } else { app(dir, dec) },
            language: Some(if lang == "nb" { "no".into() } else { lang.into() }),
            task: Some("transcribe".into()), tail_paddings: -1, ..Default::default()
        };
        match name {
            "parakeet" => {
                mc.transducer = OfflineTransducerModelConfig { encoder: app(PARAKEET.name, "encoder.int8.onnx"), decoder: app(PARAKEET.name, "decoder.int8.onnx"), joiner: app(PARAKEET.name, "joiner.int8.onnx") };
                mc.tokens = app(PARAKEET.name, "tokens.txt"); mc.model_type = Some("nemo_transducer".into());
            }
            "fastconformer-de" | "fastconformer-pt" | "fastconformer-10" => {
                let d = match name {
                    "fastconformer-de" => "sherpa-onnx-nemo-transducer-stt_de_fastconformer_hybrid_large_pc-int8",
                    "fastconformer-pt" => "sherpa-onnx-nemo-transducer-stt_pt_fastconformer_hybrid_large_pc-int8",
                    _ => "sherpa-onnx-nemo-fast-conformer-transducer-be-de-en-es-fr-hr-it-pl-ru-uk-20k-int8",
                };
                mc.transducer = OfflineTransducerModelConfig { encoder: m(d, "encoder.int8.onnx"), decoder: m(d, "decoder.int8.onnx"), joiner: m(d, "joiner.int8.onnx") };
                mc.tokens = m(d, "tokens.txt"); mc.model_type = Some("nemo_transducer".into());
            }
            "canary" => {
                let d = "sherpa-onnx-nemo-canary-180m-flash-en-es-de-fr-int8";
                mc.canary = OfflineCanaryModelConfig { encoder: m(d, "encoder.int8.onnx"), decoder: m(d, "decoder.int8.onnx"), src_lang: Some(lang.into()), tgt_lang: Some(lang.into()), use_pnc: true };
                mc.tokens = m(d, "tokens.txt");
            }
            "moonshine-es" | "moonshine-zh" => {
                let d = if name == "moonshine-es" { "sherpa-onnx-moonshine-base-es-quantized-2026-02-27" } else { "sherpa-onnx-moonshine-base-zh-quantized-2026-02-27" };
                mc.moonshine = OfflineMoonshineModelConfig { encoder: m(d, "encoder_model.ort"), merged_decoder: m(d, "decoder_model_merged.ort"), ..Default::default() };
                mc.tokens = m(d, "tokens.txt");
            }
            "omni-300m" => {
                        let d = "sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-v2-int8-2026-02-05";
                        mc.omnilingual = OfflineOmnilingualAsrCtcModelConfig { model: app(d, "model.int8.onnx") }; mc.tokens = app(d, "tokens.txt");
                    }
            "omni-1b" => {
                let d = "sherpa-onnx-omnilingual-asr-1600-languages-1B-ctc-v2-int8-2026-02-05";
                mc.omnilingual = OfflineOmnilingualAsrCtcModelConfig { model: m(d, "model.int8.onnx") }; mc.tokens = m(d, "tokens.txt");
            }
            "sensevoice" => {
                let d = "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09";
                mc.sense_voice = OfflineSenseVoiceModelConfig { model: m(d, "model.int8.onnx"), language: Some("zh".into()), use_itn: true }; mc.tokens = m(d, "tokens.txt");
            }
            "firered2-ctc" => {
                let d = "sherpa-onnx-fire-red-asr2-ctc-zh_en-int8-2026-02-25";
                mc.fire_red_asr_ctc = OfflineFireRedAsrCtcModelConfig { model: m(d, "model.int8.onnx") }; mc.tokens = m(d, "tokens.txt");
            }
            "whisper-small" => { mc.whisper = whisper("sherpa-onnx-whisper-small", "small-encoder.int8.onnx", "small-decoder.int8.onnx", true); mc.tokens = m("sherpa-onnx-whisper-small", "small-tokens.txt"); }
            "whisper-base" => { mc.whisper = whisper("sherpa-onnx-whisper-base", "base-encoder.int8.onnx", "base-decoder.int8.onnx", true); mc.tokens = m("sherpa-onnx-whisper-base", "base-tokens.txt"); }
            "whisper-tiny" => { mc.whisper = whisper("sherpa-onnx-whisper-tiny", "tiny-encoder.int8.onnx", "tiny-decoder.int8.onnx", true); mc.tokens = m("sherpa-onnx-whisper-tiny", "tiny-tokens.txt"); }
            "nb-whisper-base" => { mc.whisper = whisper("nb-whisper-base", "nb-base-encoder.int8.onnx", "nb-base-decoder.int8.onnx", true); mc.tokens = m("nb-whisper-base", "nb-base-tokens.txt"); }
            "nb-whisper-small" => { mc.whisper = whisper("nb-whisper-small", "nb-small-encoder.int8.onnx", "nb-small-decoder.int8.onnx", true); mc.tokens = m("nb-whisper-small", "nb-small-tokens.txt"); }
            "whisper-turbo" => { mc.whisper = whisper("sherpa-onnx-whisper-turbo", "turbo-encoder.int8.onnx", "turbo-decoder.int8.onnx", false); mc.tokens = app("sherpa-onnx-whisper-turbo", "turbo-tokens.txt"); }
            _ => unreachable!(),
        }
        mc.num_threads = std::env::var("MURAL_BENCH_THREADS").ok().and_then(|t| t.parse().ok()).unwrap_or(THREADS);
        if let Ok(provider) = std::env::var("MURAL_BENCH_PROVIDER") { mc.provider = Some(provider); }
            c
        };
        let decode = |recognizer: &OfflineRecognizer, samples: &[f32]| -> String {
            let stream = recognizer.create_stream();
            stream.accept_waveform(SAMPLE_RATE, samples);
            recognizer.decode(&stream);
            stream.get_result().map(|r| r.text).unwrap_or_default()
        };
        // "fast+slow": the second model hears the clip again only when the first heard nothing,
        // or (on macOS) something that isn't the target language.
        let wrong = |lang: &str, heard: &str| -> bool {
            if heard.trim().is_empty() { return true; }
            match crate::engine::langdetect::detect(heard) {
                Some((detected, confidence)) => !detected.starts_with(if lang == "nb" { "n" } else { lang }) && confidence > 0.5,
                None => false,
            }
        };
        let only = std::env::var("MURAL_BENCH_ONLY").ok();
        for (name, langs) in candidates {
            if only.as_ref().is_some_and(|o| !o.split(',').any(|x| x == name)) { continue; }
            for lang in langs {
                let (first, second) = name.split_once('+').map(|(a, b)| (a, Some(b))).unwrap_or((name, None));
                let Some(recognizer) = OfflineRecognizer::create(&build(first, lang)) else { println!("{name} {lang}: failed to load"); continue };
                let fallback = second.and_then(|b| OfflineRecognizer::create(&build(b, lang)));
                let mut fell_back = 0;
                let (mut total, mut good, mut n, mut secs, mut audio_secs) = (0.0, 0, 0, 0.0, 0.0);
                let (mut native, mut accented) = (vec![], vec![]);
                let mut slowest: f64 = 0.0;
                let mut worst = vec![];
                for (file, l, text) in rows.iter().filter(|r| r.1 == lang) {
                    let _ = l;
                    let wave = Wave::read(&format!("{clips}/{file}")).unwrap();
                    let mut samples = vec![0.0; SAMPLE_RATE as usize / 2];
                    samples.extend(resample(wave.samples(), wave.sample_rate(), SAMPLE_RATE));
                    samples.extend(vec![0.0; SAMPLE_RATE as usize / 2]);
                    let started = std::time::Instant::now();
                    let mut heard = decode(&recognizer, &samples);
                    if let Some(fallback) = &fallback {
                        if wrong(lang, &heard) { heard = decode(fallback, &samples); fell_back += 1; }
                    }
                    let took = started.elapsed().as_secs_f64();
                    secs += took;
                    slowest = slowest.max(took);
                    audio_secs += samples.len() as f64 / SAMPLE_RATE as f64;
                    let a = shadow::score(text, &heard);
                    total += a.score; n += 1;
                    if file.ends_with("-a.wav") { accented.push(a.score) } else { native.push(a.score) }
                    if a.verdict() == shadow::Verdict::Good { good += 1 } else { worst.push(format!("{file}: “{}” → “{}”", text, heard.trim())); }
                }
                let mean = |v: &[f64]| 100.0 * v.iter().sum::<f64>() / v.len().max(1) as f64;
                println!("{name:14} {lang}: score {:3.0}% (native {:3.0}%, accented {:3.0}%)  good {good:2}/{n}  {:.2}s/clip, slowest {:.2}s  (RTF {:.2})",
                         100.0 * total / n as f64, mean(&native), mean(&accented), secs / n as f64, slowest, secs / audio_secs);
                if fallback.is_some() { println!("        fell back on {fell_back}/{n}"); }
                for w in worst.iter().take(3) { println!("        {w}"); }
            }
        }
    }

    /// Every Shadow phrase must be speakable, and a sample of them, spoken by Mural and heard by
    /// the recogniser, must score Good: otherwise a learner saying it perfectly would fail too.
    /// Downloads missing models. `MURAL_VOICE_LANGS=es cargo test --release -- --ignored shadow_phrases`.
    #[test]
    #[ignore]
    fn shadow_phrases() {
        use crate::engine::{phrases, shadow};
        let only = std::env::var("MURAL_VOICE_LANGS").ok();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let mut failures = vec![];
        for id in phrases::languages() {
            if only.as_ref().is_some_and(|o| !o.split(',').any(|l| l == id)) { continue; }
            runtime.block_on(install(id, |_| {})).unwrap_or_else(|e| panic!("{id}: {e}"));
            let loaded = load(id).unwrap_or_else(|e| panic!("{id}: {e}"));
            let all = phrases::all(id);
            let (mut silent, mut sampled, mut good) = (0, 0, 0);
            for (i, phrase) in all.iter().enumerate() {
                let config = GenerationConfig { sid: loaded.sid, speed: 0.95, ..Default::default() };
                let audio = loaded.tts.generate_with_config(&phrase.text, &config, None::<fn(&[f32], f32) -> bool>);
                let Some(audio) = audio.filter(|a| a.samples().len() > 1000) else {
                    silent += 1;
                    failures.push(format!("{id} {}: no speech for “{}”", phrase.id, phrase.text));
                    continue;
                };
                if i % 40 != 0 { continue; }
                sampled += 1;
                let mut samples = resample(audio.samples(), audio.sample_rate(), SAMPLE_RATE);
                samples.extend(std::iter::repeat_n(0.0, SAMPLE_RATE as usize));
                let heard = loaded.transcribe(&samples);
                let attempt = shadow::score(&phrase.text, &heard);
                if attempt.verdict() == shadow::Verdict::Good { good += 1 } else {
                    println!("  {id} {} {:.0}%: said “{}” heard “{}”", phrase.id, attempt.score * 100.0, phrase.text, heard.trim());
                }
            }
            println!("{id}: {} phrases, {silent} unspeakable, {good}/{sampled} sampled scored Good", all.len());
            if sampled > 0 && (good as f32) < sampled as f32 * 0.8 { failures.push(format!("{id}: only {good}/{sampled} scored Good")); }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
