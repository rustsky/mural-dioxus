# Mural for macOS (Dioxus)

A desktop port of [Chuloo/mural](https://github.com/Chuloo/mural), the conversation-first
language app, built with [Dioxus](https://dioxuslabs.com) 0.7. It follows the iPhone client
(`apps/ios`): same eight language modules, teaching prompts, learning engine, recall bars and
backup format. Backups exported here import on iPhone and Android, and the other way round.

Mural uses your own OpenAI API key (GPT-Live-1 for voice, GPT-5.6 Luna for teaching). The key is
stored in the macOS login Keychain and sent only to OpenAI. Alternatively, choose an Ollama teacher
(a model on this Mac, or Ollama Cloud with a free key); conversations then use the on-device voice
described below and need no OpenAI key.

## Build

Requirements: macOS 13+, current stable Rust, and the Dioxus CLI (`cargo install dioxus-cli`).

```sh
cargo test                     # core learning, archive and API tests
./scripts/build-macos.sh       # dist/Mural.app, the .dmg and the source archive
```

The script signs ad-hoc by default. Set `SIGN_IDENTITY="Developer ID Application: …"` to sign
with a hardened runtime for distribution (notarization is a separate step).

`cargo run` starts the app for development, but **voice needs the app bundle**: WebKit only
exposes the microphone when the host app's Info.plist declares `NSMicrophoneUsageDescription`.

## How it is put together

| Path | Contents |
| --- | --- |
| `src/engine/` | Port of `apps/ios/Core`: language modules, themes, transcripts, archive (v1 migration, validation, merge), learning projection, teaching prompts, pace, inactivity policy, provider errors, pinyin |
| `src/services/` | Learning record (JSON in `~/Library/Application Support/Mural`), Keychain, OpenAI Responses and live-session requests |
| `src/app/state.rs` | Conversation coordinator: session lifecycle, meanings, assessments, delegations, typed replies, topics |
| `src/bridge.js` | WebRTC transport running in the webview (microphone, echo cancellation, playback, level meters) and the orb animation |
| `src/app/*.rs` | Talk, Themes, Words, Settings, onboarding and sheets |
| `tests/fixtures/cross-platform/` | Shared fixtures from `shared/fixtures`, checked by the same tests as iPhone and Android |

Voice runs over WebRTC inside WKWebView, so it gets the system's echo cancellation and Opus codec.
Rust keeps the API key and makes every HTTP request; the webview only exchanges SDP and events.
Mandarin pinyin uses the same system dictionary as the iPhone app (`CFStringTokenizer`), and
off-language detection uses Apple's NaturalLanguage framework.

### On-device voice (Ollama teachers)

With an Ollama teacher, the **Natural** voice (default) runs open models through
[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx), which works the same on macOS and Windows:

| Step | Model | Languages |
| --- | --- | --- |
| End of turn | Silero VAD | all |
| Listening | NVIDIA Parakeet TDT 0.6B v3 (int8) | de, en, es, fr, it, pt |
| Listening | Meta Omnilingual ASR 300M CTC (int8) | nb, zh |
| Speaking | Kokoro-82M v1.0 | en, es, fr, it, pt, zh |
| Speaking | Piper (thorsten-high, talesyntese-medium) | de, nb |

The webview captures the microphone with echo cancellation and streams 16 kHz PCM to Rust
(`src/services/voice.rs`), which transcribes each utterance, asks the teacher for a reply, and
streams the synthesized audio back sentence by sentence. The microphone is ignored while Mural
speaks. Models download on first use per language (about 340–800 MB) into
`~/Library/Application Support/Mural/voice-models` and can be deleted in Settings. **System
speech** is the alternative: WebKit speech recognition (needs Dictation turned on) and system voices.

sherpa-onnx is linked statically and includes espeak-ng (GPL-3.0) for Kokoro and Piper
pronunciation, which is why this port is licensed under the GPL (see below). Model licences and
credits, including the CC-BY-4.0 attribution for Parakeet, are in `THIRD_PARTY_NOTICES.md`.

Not ported: managed accounts, hosted minutes and purchases (disabled in the iPhone build too),
and simulator-only verification harnesses.

### Debug automation

Debug builds accept `--preview` (in-memory record), `--preview-seed` (sample Spanish data), and
`MURAL_AUTOMATION=<dir>` for scripted checks without screen-recording access (see
`src/app/automation.rs`). `MURAL_FAKE_MICROPHONE=1` and `MURAL_DEBUG_API_KEY` exercise the voice
request path without a microphone prompt or Keychain access; with it, `MURAL_FAKE_SPEECH="a|b"`
(system speech) or `MURAL_FAKE_SPEECH_WAV="a.wav|b.wav"` (natural voice) stand in for the learner.
None of these exist in release builds. `MURAL_TRACE=1` logs voice bridge traffic to stderr in any build.

## License

GPL-3.0-or-later (`LICENSE`). The voice engine statically links GPL-3.0 code (espeak-ng,
piper-phonemize), so the app as a whole is distributed under the GPL.

The original [Chuloo/mural](https://github.com/Chuloo/mural) code is © 2026 Hackmamba under the MIT
License (`LICENSE-MIT`); that notice is kept for the parts derived from it. Third-party components,
voice models and Rust crates are listed in `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_RUST.txt`
(regenerate the latter with `scripts/rust-licenses.sh`); the app shows them in Settings →
Licences and open-source notices.

When you give the app to anyone, give them the source too: `scripts/build-macos.sh` writes
`dist/Mural-<version>-source.tar.gz` next to the disk image for that purpose.

The Mural name and logo identify the original project; no licence grants trademark rights.
