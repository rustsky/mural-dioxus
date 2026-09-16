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

Rust crates compiled into the app, with their licences, are listed in `THIRD_PARTY_RUST.txt`
(generated from `Cargo.lock` by `scripts/rust-licenses.sh`). Their licence texts are in each
crate's source, available from https://crates.io.

## Voice models (downloaded on first use, not shipped in the app)

| Model | Author | Licence | Used for |
| --- | --- | --- | --- |
| [Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), int8 ONNX export by sherpa-onnx | NVIDIA | **CC-BY-4.0** | Listening: de, en, es, fr, it, pt |
| [Omnilingual ASR 300M CTC v2](https://github.com/facebookresearch/omnilingual-asr), int8 ONNX export by sherpa-onnx | Meta Platforms, Inc. | Apache-2.0 | Listening: nb, zh |
| [Kokoro-82M v1.0](https://huggingface.co/hexgrad/Kokoro-82M) | hexgrad | Apache-2.0 | Speaking: en, es, fr, it, pt, zh |
| [Piper](https://huggingface.co/rhasspy/piper-voices) voice “thorsten (high)”, trained on the [Thorsten-Voice](https://github.com/thorstenMueller/Thorsten-Voice) dataset (CC0) | Rhasspy / Thorsten Müller | MIT | Speaking: de |
| [Piper](https://huggingface.co/rhasspy/piper-voices) voice “talesyntese (medium)”, trained on the [Språkbanken](https://www.nb.no/sprakbanken/en/resource-catalogue/oai-nb-no-sbr-15/) dataset (CC0) | Rhasspy / National Library of Norway | MIT | Speaking: nb |
| [Silero VAD](https://github.com/snakers4/silero-vad) | Silero Team | MIT | Detecting the end of a turn |
| espeak-ng-data (pronunciation data inside the Kokoro and Piper downloads) | espeak-ng contributors | GPL-3.0-or-later | Pronunciation |

Parakeet TDT 0.6B v3 is © NVIDIA and licensed under the Creative Commons Attribution 4.0
International licence (https://creativecommons.org/licenses/by/4.0/). Mural uses a quantized ONNX
conversion published by the sherpa-onnx project; no other changes were made.
