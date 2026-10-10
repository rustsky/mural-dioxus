# Mural Desktop

Learn a language by talking. Mural holds a real conversation with you, drills the phrases you need
until they come out without thinking, and plays through everyday situations from hello to goodbye.

A desktop port of [Chuloo/mural](https://github.com/Chuloo/mural), built with
[Dioxus](https://dioxuslabs.com) 0.7 for macOS, Windows and Linux. It follows the iPhone client
(`apps/ios`): same eight language modules, teaching prompts, learning engine, recall bars and
backup format. Backups exported here import on iPhone and Android, and the other way round.

![Four screens of Mural: a conversation in progress with its scenario beats, a scenario brief, the
phrase list, and a scored repeat](docs/images/features.png)

## The four tabs

**Talk** — open conversation. Mural speaks only your target language, follows what you say, and
corrects gently as you go. It picks one teaching move per turn — repair a breakdown, recast a
mistake you keep making, bring back a word that is due, introduce something new, or stretch you —
from what the last turn actually showed. The conversation scrolls in the window, with word lookup
and meanings on every line.

**Shadow** — 1500 everyday German phrases, 300 in each band from A1 to C1. Mural says one in its
on-device voice, slowly if you ask; you hold the microphone and say it back; the on-device
recogniser scores the attempt and marks the words that did not come through. Due phrases come
first and a tick marks what you have said.

**Dialog** — everyday situations played out: where you are, who Mural plays, what you want, and
the beats to pass through. The beats tick off as the conversation reaches them, and near the end
Mural is told to land the exchange rather than open new ground.

**Words** — the vocabulary Mural noticed you using, with recall bars that grow as you use a word
again on other days and in other contexts.

## Teachers

- **OpenAI** — bring your own API key (GPT-Live-1 for voice, GPT-5.6 Luna for teaching). The key is
  kept in the system keychain and sent only to OpenAI.
- **Ollama** — a model on this computer, or Ollama Cloud with a free key. Conversations use the
  on-device voice below and need no OpenAI key.

Shadow and Dialog are German-only for now: each catalogue is one file per language, and the tabs
hide themselves for languages without one. Dialog ships 193 of a planned 365 scenarios.

## Download

Builds for every push are in the GitHub Actions run artifacts; tagged versions (`v*`) are published
as GitHub releases.

| Platform | File | Notes |
| --- | --- | --- |
| macOS 13+ (arm64, x86_64) | `Mural-<version>-<arch>.dmg` | Ad-hoc signed, not notarized |
| Windows x86_64 | `Mural-<version>-x86_64-setup.exe` | Per-user NSIS installer; unsigned, so SmartScreen asks for confirmation. Downloads WebView2 only if missing |
| Linux x86_64 | `Mural-<version>-x86_64.AppImage` | WebKitGTK, GTK and GStreamer bundled; `chmod +x` before running |
| Source | `Mural-<version>-source.tar.gz` | |

### Platform differences

| | macOS | Windows | Linux |
| --- | --- | --- | --- |
| API key storage | Login Keychain | Credential Manager | Secret Service (GNOME Keyring, KWallet) |
| Data directory | `~/Library/Application Support/Mural` | `%APPDATA%\Mural` | `~/.local/share/Mural` |
| Mandarin pinyin, off-language detection | Yes | Not yet | Not yet |

Pinyin uses the same system dictionary as the iPhone app (`CFStringTokenizer`), and off-language
detection uses Apple's NaturalLanguage framework, hence macOS only for now.

## Voice

### OpenAI

Voice runs over WebRTC inside the webview, so it gets the system's echo cancellation and Opus
codec. Rust holds the API key and makes every HTTP request; the webview only exchanges SDP and
events.

### On-device (Ollama teachers, Shadow and Dialog)

The **Natural** voice runs open models through
[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx):

| Step | Model | Languages |
| --- | --- | --- |
| End of turn | Silero VAD | all |
| Listening | NVIDIA Parakeet TDT 0.6B v3 (int8) | de, en, es, fr, it, pt |
| Listening | Meta Omnilingual ASR 300M CTC (int8) | nb, zh |
| Speaking | Kokoro-82M v1.0 | en, es, fr, it, pt, zh |
| Speaking | Piper (thorsten-high, talesyntese-medium) | de, nb |

The webview captures the microphone with echo cancellation and streams 16 kHz PCM to Rust
(`src/services/voice.rs`), which transcribes each utterance, asks the teacher for a reply, and
streams synthesized audio back sentence by sentence. Models download on first use per language
(about 340–800 MB; German is ~575 MB) into `voice-models` in the data directory, and can be
deleted in Settings.

**Hold to speak** (Settings) keeps the microphone closed between turns and opens it only while you
hold it, so a pause is thinking time rather than the end of your turn. Releasing flushes the
recogniser immediately. With an OpenAI teacher the provider's own detector still decides when a
turn ended.

**System speech** is the alternative: the operating system's own recognition (Dictation must be on)
and voices.

## Build

Requirements: current stable Rust and the Dioxus CLI (`cargo install dioxus-cli`; CI pins the
version matching `Cargo.lock`).

```sh
cargo test    # engine, scoring, catalogues, archive and API tests
```

| Platform | Command | Output |
| --- | --- | --- |
| macOS | `./scripts/build-macos.sh` | `dist/Mural.app`, the `.dmg` and the source archive |
| Linux | `./scripts/build-linux-appimage.sh` | `dist/Mural-<version>-<arch>.AppImage` (needs WebKitGTK 4.1, GTK 3 and GStreamer; package list in `.github/workflows/build.yml`) |
| Windows | `./scripts/rust-licenses.sh && dx bundle --release --package-types nsis --out-dir target/installer` | `*-setup.exe` under `target/installer` (run from Git Bash) |

The macOS script signs ad-hoc by default. Set `SIGN_IDENTITY="Developer ID Application: …"` to sign
with a hardened runtime for distribution (notarization is a separate step).

`cargo run` starts the app for development, but on macOS **voice needs the app bundle**: WebKit only
exposes the microphone when the host app's Info.plist declares `NSMicrophoneUsageDescription`.

CI (`.github/workflows/build.yml`) tests and builds all platforms on every push and pull request.

## Project layout

| Path | Contents |
| --- | --- |
| `src/engine/` | Port of `apps/ios/Core`: language modules, themes, transcripts, archive, learning projection, teaching prompts, pace, inactivity policy, pinyin |
| `src/engine/plan.rs` | The per-turn teaching move: repair, recast, retrieve, introduce, stretch, consolidate |
| `src/engine/phrases.rs`, `src/engine/shadow.rs` | The phrase catalogue and the repeat scoring |
| `src/engine/scenarios.rs` | The dialogue scenarios and their beats |
| `src/services/` | Learning record, keychain access, OpenAI and Ollama requests, on-device voice, shadowing and dialogue progress |
| `src/app/state.rs` | Conversation coordinator: session lifecycle, meanings, assessments, delegations, typed replies, scenarios |
| `src/app/talk.rs`, `shadow.rs`, `dialog.rs`, `library.rs` | The four tabs |
| `src/bridge.js` | WebRTC transport in the webview (microphone, echo cancellation, playback, level meters) and the orb animation |
| `assets/phrases/`, `assets/scenarios/` | Content, one JSON file per language |
| `tests/fixtures/cross-platform/` | Shared fixtures from `shared/fixtures`, checked by the same tests as iPhone and Android |

Learning progress lives in one JSON archive, the same format the phones back up and restore.
Shadowing and dialogue progress are separate files on this computer only, since drill attempts are
not conversation evidence.

Not ported: managed accounts, hosted minutes and purchases (disabled in the iPhone build too), and
simulator-only verification harnesses.

## A note on the German content

The 1500 phrases and the scenarios were written for this port and have **not been reviewed by a
native speaker**. Structure is machine-checked — every phrase is unique, banded, short enough to
shadow — but idiom is not. The B2 and C1 material is where that matters most. Corrections welcome.

## Debugging

Debug builds only:

| Flag / variable | Effect |
| --- | --- |
| `--preview` | In-memory learning record |
| `--preview-seed` | Sample Spanish data |
| `MURAL_AUTOMATION=<dir>` | Scripted checks without screen-recording access (see `src/app/automation.rs`) |
| `MURAL_FAKE_MICROPHONE=1`, `MURAL_DEBUG_API_KEY` | Exercise the voice request path without a microphone prompt or keychain access |
| `MURAL_FAKE_SPEECH="a\|b"` | Stand-in learner utterances (system speech) |
| `MURAL_FAKE_SPEECH_WAV="a.wav\|b.wav"` | Stand-in learner audio (natural voice) |

Any build: `MURAL_TRACE=1` logs voice bridge traffic to stderr.

## License

GPL-3.0-or-later (`LICENSE`). The voice engine statically links GPL-3.0 code (espeak-ng,
piper-phonemize, used by Kokoro and Piper for pronunciation), so the app as a whole is distributed
under the GPL.

The original [Chuloo/mural](https://github.com/Chuloo/mural) code is © 2026 Hackmamba under the MIT
License (`LICENSE-MIT`); that notice is kept for the parts derived from it.

Third-party components, voice models (including the CC-BY-4.0 attribution for Parakeet) and Rust
crates are listed in `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_RUST.txt` (regenerate the latter
with `scripts/rust-licenses.sh`). The app shows them in Settings → Licences and open-source notices.

Film clips for a phrase open [playphrase.me](https://playphrase.me) in your browser. Mural does not
fetch, store or redistribute their clips; PlayPhrase offers no API and asks that its clips are not
republished.

When you give the app to anyone, give them the source too. The source archive is published with
every release, and `scripts/build-macos.sh` writes one next to the disk image.

The Mural name and logo identify the original project; no licence grants trademark rights.
