//! Natural on-device voice for Ollama teachers: Silero VAD finds each utterance, Parakeet or
//! Omnilingual transcribes it, and Kokoro or Piper speaks Mural's reply. Everything runs through
//! sherpa-onnx on this computer (macOS and Windows); models download once per language.
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use sherpa_onnx::{
    GenerationConfig, OfflineOmnilingualAsrCtcModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineTransducerModelConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig, SileroVadModelConfig, VadModelConfig, VoiceActivityDetector,
};

use super::store::LearningStore;

const RELEASES: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";
pub const SAMPLE_RATE: i32 = 16_000;
const THREADS: i32 = 4;

/// One downloadable model: a `.tar.bz2` release archive holding a folder of the same name, or a single file.
#[derive(Debug, PartialEq)]
pub struct Pack {
    pub name: &'static str,
    url: &'static str,
    pub megabytes: u32,
    archive: bool,
}

const VAD: Pack = Pack { name: "silero_vad.onnx", url: "asr-models/silero_vad.onnx", megabytes: 1, archive: false };
const PARAKEET: Pack = Pack { name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8", url: "asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2", megabytes: 464, archive: true };
const OMNILINGUAL: Pack = Pack { name: "sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-v2-int8-2026-02-05", url: "asr-models/sherpa-onnx-omnilingual-asr-1600-languages-300M-ctc-v2-int8-2026-02-05.tar.bz2", megabytes: 278, archive: true };
const KOKORO: Pack = Pack { name: "kokoro-multi-lang-v1_0", url: "tts-models/kokoro-multi-lang-v1_0.tar.bz2", megabytes: 334, archive: true };
const PIPER_DE: Pack = Pack { name: "vits-piper-de_DE-thorsten-high", url: "tts-models/vits-piper-de_DE-thorsten-high.tar.bz2", megabytes: 110, archive: true };
const PIPER_NO: Pack = Pack { name: "vits-piper-no_NO-talesyntese-medium", url: "tts-models/vits-piper-no_NO-talesyntese-medium.tar.bz2", megabytes: 64, archive: true };

#[derive(Debug, PartialEq)]
enum Listener { Parakeet, Omnilingual }

#[derive(Debug, PartialEq)]
enum Speaker {
    /// Kokoro speaker id and espeak language.
    Kokoro(i32, &'static str),
    /// Piper model folder and file.
    Piper(&'static Pack, &'static str),
}

#[derive(Debug, PartialEq)]
struct Setup { listener: Listener, speaker: Speaker }

fn setup(language_id: &str) -> Option<Setup> {
    use Listener::*;
    let (listener, speaker) = match language_id {
        "en" => (Parakeet, Speaker::Kokoro(3, "en-us")),       // af_heart
        "es" => (Parakeet, Speaker::Kokoro(28, "es")),         // ef_dora
        "fr" => (Parakeet, Speaker::Kokoro(30, "fr-fr")),      // ff_siwis
        "it" => (Parakeet, Speaker::Kokoro(35, "it")),         // if_sara
        "pt" => (Parakeet, Speaker::Kokoro(42, "pt-br")),      // pf_dora
        "zh" => (Omnilingual, Speaker::Kokoro(47, "zh")),      // zf_xiaoxiao
        "de" => (Parakeet, Speaker::Piper(&PIPER_DE, "de_DE-thorsten-high.onnx")),
        "nb" => (Omnilingual, Speaker::Piper(&PIPER_NO, "no_NO-talesyntese-medium.onnx")),
        _ => return None,
    };
    Some(Setup { listener, speaker })
}

fn packs(language_id: &str) -> Vec<&'static Pack> {
    let Some(s) = setup(language_id) else { return vec![] };
    let listener = match s.listener { Listener::Parakeet => &PARAKEET, Listener::Omnilingual => &OMNILINGUAL };
    let speaker = match s.speaker { Speaker::Kokoro(..) => &KOKORO, Speaker::Piper(pack, _) => pack };
    vec![&VAD, listener, speaker]
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
    Ok(())
}

async fn download(pack: &'static Pack, progress: impl Fn(Progress)) -> Result<(), String> {
    let failed = || format!("The voice download ({}) didn’t finish. Check your connection and try again.", pack.name);
    let part = directory().join(format!(".{}.part", pack.name));
    let mut response = download_client().get(format!("{RELEASES}/{}", pack.url)).send().await.map_err(|_| failed())?;
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
            // Sample recordings are not needed at runtime.
            if entry.path()?.components().any(|c| c.as_os_str() == "test_wavs") { continue; }
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
    tts: OfflineTts,
    sid: i32,
}

#[derive(Default)]
struct State {
    loaded: Option<Loaded>,
    vad: Option<VoiceActivityDetector>,
    /// Recent audio, so each utterance can start a little before the VAD noticed speech.
    history: Vec<f32>,
    /// Stream position of `history[0]`.
    history_start: usize,
}

/// Silero reports speech slightly late; this much earlier audio keeps the first word intact.
const LEAD_IN: usize = SAMPLE_RATE as usize * 2 / 5;
const HISTORY: usize = SAMPLE_RATE as usize * 40;

impl State {
    fn restart_listening(&mut self) {
        if let Some(vad) = &self.vad { vad.reset(); }
        self.history.clear();
        self.history_start = 0;
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

fn load(language_id: &str) -> Result<Loaded, String> {
    let s = setup(language_id).ok_or("The natural voice doesn’t support this language yet.")?;
    let mut config = OfflineRecognizerConfig::default();
    match s.listener {
        Listener::Parakeet => {
            config.model_config.transducer = OfflineTransducerModelConfig {
                encoder: file(&PARAKEET, "encoder.int8.onnx"),
                decoder: file(&PARAKEET, "decoder.int8.onnx"),
                joiner: file(&PARAKEET, "joiner.int8.onnx"),
            };
            config.model_config.tokens = file(&PARAKEET, "tokens.txt");
            config.model_config.model_type = Some("nemo_transducer".into());
        }
        Listener::Omnilingual => {
            config.model_config.omnilingual = OfflineOmnilingualAsrCtcModelConfig { model: file(&OMNILINGUAL, "model.int8.onnx") };
            config.model_config.tokens = file(&OMNILINGUAL, "tokens.txt");
        }
    }
    config.model_config.num_threads = THREADS;
    let recognizer = OfflineRecognizer::create(&config).ok_or("Mural couldn’t load its speech recognizer. Delete the voice models in Settings and try again.")?;
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
    Ok(Loaded { language_id: language_id.into(), recognizer, tts, sid })
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

/// Feeds 16 kHz microphone audio; returns the text of every utterance that just finished.
pub async fn hear(samples: Vec<f32>) -> Result<Vec<String>, String> {
    run(move |state| {
        let (Some(vad), Some(loaded)) = (&state.vad, &state.loaded) else { return vec![] };
        state.history.extend_from_slice(&samples);
        if state.history.len() > HISTORY {
            let excess = state.history.len() - HISTORY;
            state.history.drain(..excess);
            state.history_start += excess;
        }
        vad.accept_waveform(&samples);
        let mut heard = vec![];
        while let Some(segment) = vad.front() {
            let start = (segment.start().max(0) as usize).saturating_sub(LEAD_IN).max(state.history_start);
            let end = segment.start().max(0) as usize + segment.samples().len();
            let audio = state.history.get(start - state.history_start..end.saturating_sub(state.history_start).min(state.history.len()))
                .filter(|a| a.len() >= segment.samples().len())
                .unwrap_or(segment.samples());
            let stream = loaded.recognizer.create_stream();
            stream.accept_waveform(SAMPLE_RATE, audio);
            loaded.recognizer.decode(&stream);
            if let Some(text) = stream.get_result().map(|r| r.text.trim().to_string()).filter(|t| !t.is_empty()) {
                heard.push(text);
            }
            vad.pop();
        }
        heard
    }).await
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
            assert_eq!(p.len(), 3, "{id}");
            assert_eq!(p[0], &VAD);
        }
        assert!(packs("xx").is_empty());
        assert_eq!(packs("de")[2].name, "vits-piper-de_DE-thorsten-high");
        assert_eq!(packs("zh")[1], &OMNILINGUAL);
    }
}
