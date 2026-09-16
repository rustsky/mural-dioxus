# Mural for macOS (Dioxus)

A desktop port of [Chuloo/mural](https://github.com/Chuloo/mural), the conversation-first
language app, built with [Dioxus](https://dioxuslabs.com) 0.7. It follows the iPhone client
(`apps/ios`): same eight language modules, teaching prompts, learning engine, recall bars and
backup format. Backups exported here import on iPhone and Android, and the other way round.

Mural uses your own OpenAI API key (GPT-Live-1 for voice, GPT-5.6 Luna for teaching). The key is
stored in the macOS login Keychain and sent only to OpenAI.

## Build

Requirements: macOS 13+, current stable Rust, and the Dioxus CLI (`cargo install dioxus-cli`).

```sh
cargo test                     # core learning, archive and API tests
./scripts/build-macos.sh       # dist/Mural.app and dist/Mural-<version>-<arch>.dmg
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

Not ported: managed accounts, hosted minutes and purchases (disabled in the iPhone build too),
and simulator-only verification harnesses.

### Debug automation

Debug builds accept `--preview` (in-memory record), `--preview-seed` (sample Spanish data), and
`MURAL_AUTOMATION=<dir>` for scripted checks without screen-recording access (see
`src/app/automation.rs`). `MURAL_FAKE_MICROPHONE=1` and `MURAL_DEBUG_API_KEY` exercise the voice
request path without a microphone prompt or Keychain access. None of these exist in release builds.

## License

MIT, like the original project. The Mural name and logo identify the original project; the
software license does not grant trademark rights.
