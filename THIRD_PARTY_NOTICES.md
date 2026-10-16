# Licences and credits

Mural for macOS is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version. It is distributed in the hope that it will be
useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
FOR A PARTICULAR PURPOSE. See `LICENSE` for the full text.

The complete corresponding source code is published with every build (`Mural-<version>-source.tar.gz`
next to the disk image). Anyone who receives the app from you is entitled to it.

## Original project

Mural is a port of [Chuloo/mural](https://github.com/Chuloo/mural), © 2026 Hackmamba, released
under the MIT License (`LICENSE-MIT`). That notice applies to the code derived from it; the
combined work is distributed under GPL-3.0-or-later. The Mural name and logo identify the original
project; no licence grants trademark rights.

## Linked into the app

The voice engine is [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 1.13.8, linked statically
together with the libraries below. Their sources are available at the links, at the versions pinned
by the sherpa-onnx 1.13.8 release.

| Component | Licence | Source |
| --- | --- | --- |
| sherpa-onnx | Apache-2.0 | https://github.com/k2-fsa/sherpa-onnx |
| ONNX Runtime | MIT | https://github.com/microsoft/onnxruntime |
| espeak-ng (including ucd-tools) | **GPL-3.0-or-later** | https://github.com/espeak-ng/espeak-ng |
| piper-phonemize (sherpa-onnx fork) | **GPL-3.0** | https://github.com/csukuangfj/piper-phonemize |
| kaldi-native-fbank | Apache-2.0 | https://github.com/csukuangfj/kaldi-native-fbank |
| kaldi-decoder | Apache-2.0 | https://github.com/k2-fsa/kaldi-decoder |
| kaldifst (OpenFst) | Apache-2.0 | https://github.com/csukuangfj/kaldifst |
| KISS FFT | BSD-3-Clause | https://github.com/mborgerding/kissfft |
| simple-sentencepiece | Apache-2.0 | https://github.com/pkufool/simple-sentencepiece |

The Linux AppImage additionally contains system libraries from the build machine (Ubuntu 24.04),
among them WebKitGTK, GTK 3, GLib and GStreamer with its base, good and bad plugin sets, which are
licensed under the LGPL-2.1-or-later (some GStreamer plugins under other open-source licences).
They are separate shared libraries inside the AppImage and can be replaced; their sources are
available from Ubuntu (https://launchpad.net/ubuntu). The Windows installer is built with NSIS
(zlib licence).

Rust crates compiled into the app, with their licences, are listed in `THIRD_PARTY_RUST.txt`
(generated from `Cargo.lock` by `scripts/rust-licenses.sh`). Their licence texts are in each
crate's source, available from https://crates.io.

One crate is included in this repository rather than fetched: [tao](https://github.com/tauri-apps/tao)
0.37.1 (Apache-2.0), in `vendor/tao` with its licence. Its version number is changed to 0.34.99
so it can stand in for the tao 0.34 that Dioxus asks for, which is why `THIRD_PARTY_RUST.txt` lists
that number. The source is unchanged; `vendor/tao/MURAL.md` lists the two manifest edits.

## Voice models (downloaded on first use, not shipped in the app)

| Model | Author | Licence | Used for |
| --- | --- | --- | --- |
| [Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), int8 ONNX export by sherpa-onnx | NVIDIA | **CC-BY-4.0** | Listening: en, es, fr, it |
| [FastConformer Hybrid Large German](https://catalog.ngc.nvidia.com/orgs/nvidia/teams/nemo/models/stt_de_fastconformer_hybrid_large_pc) (stt_de_fastconformer_hybrid_large_pc), int8 ONNX export by sherpa-onnx | NVIDIA | **CC-BY-4.0** | Listening: de |
| [Whisper small](https://github.com/openai/whisper), int8 ONNX export by sherpa-onnx | OpenAI | MIT | Listening: pt, nb; second opinion for es, fr, it |
| [SenseVoice Small](https://github.com/FunAudioLLM/SenseVoice), int8 ONNX export by sherpa-onnx | FunAudioLLM (Alibaba) | [FunASR Model License](https://github.com/modelscope/FunASR/blob/main/MODEL_LICENSE) | Listening: zh |
| [Kokoro-82M v1.0](https://huggingface.co/hexgrad/Kokoro-82M) | hexgrad | Apache-2.0 | Speaking: en, es, fr, it, pt, zh |
| [Piper](https://huggingface.co/rhasspy/piper-voices) voice “thorsten (high)”, trained on the [Thorsten-Voice](https://github.com/thorstenMueller/Thorsten-Voice) dataset (CC0) | Rhasspy / Thorsten Müller | MIT | Speaking: de |
| [Piper](https://huggingface.co/rhasspy/piper-voices) voice “talesyntese (medium)”, trained on the [Språkbanken](https://www.nb.no/sprakbanken/en/resource-catalogue/oai-nb-no-sbr-15/) dataset (CC0) | Rhasspy / National Library of Norway | MIT | Speaking: nb |
| [Silero VAD](https://github.com/snakers4/silero-vad) | Silero Team | MIT | Detecting the end of a turn |
| espeak-ng-data (pronunciation data inside the Kokoro and Piper downloads) | espeak-ng contributors | GPL-3.0-or-later | Pronunciation |

Parakeet TDT 0.6B v3 and FastConformer Hybrid Large German are © NVIDIA and licensed under the
Creative Commons Attribution 4.0 International licence (https://creativecommons.org/licenses/by/4.0/).
Mural uses quantized ONNX conversions published by the sherpa-onnx project; no other changes were made.

NB-Whisper base (© the National Library of Norway, Apache-2.0) is not distributed with Mural. A
learner may build it with `scripts/convert-nb-whisper.sh`, which downloads it from the National
Library and converts it on their own computer.
