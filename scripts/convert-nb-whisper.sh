#!/usr/bin/env bash
# Builds Mural's optional Norwegian listening model on your own computer from the National Library
# of Norway's NB-Whisper base (Apache-2.0, https://huggingface.co/NbAiLab/nb-whisper-base).
# Mural doesn't download or ship this model; without it, Norwegian uses Whisper small instead.
#
# What it does: downloads the original weights, converts them unchanged to the original Whisper
# layout, exports them with sherpa-onnx's own script (the version Mural is built with), quantizes
# them to int8, and installs the result into Mural's voice-models folder. Mural uses it from the
# next Norwegian conversation.
#
# Needs Python 3.9–3.12 (or uv) and about 3 GB of temporary disk space; takes a few minutes.
# On Windows, run it from Git Bash.
#
#   ./scripts/convert-nb-whisper.sh                  # install into Mural's voice-models folder
#   ./scripts/convert-nb-whisper.sh <folder>         # install somewhere else
set -euo pipefail

NB_REVISION=2ab372b6baa181a22f54f18030cae3703402c59e
SHERPA_VERSION=v1.13.8
NAME=nb-whisper-base-int8

case "$(uname -s)" in
  Darwin) default="$HOME/Library/Application Support/Mural/voice-models" ;;
  MINGW*|MSYS*|CYGWIN*) default="$APPDATA/Mural/voice-models" ;;
  *) default="${XDG_DATA_HOME:-$HOME/.local/share}/Mural/voice-models" ;;
esac
target="${1:-$default}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cd "$work"

echo "Setting up a Python environment in ${work}…"
if command -v uv >/dev/null; then uv venv -q -p 3.11 venv; else python3 -m venv venv; fi
python=venv/bin/python
[ -x "$python" ] || python=venv/Scripts/python.exe   # Windows
if command -v uv >/dev/null; then pip=(uv pip install -q --python "$python"); else pip=("$python" -m pip install -q); fi
"${pip[@]}" "setuptools<70" wheel "torch==2.4.1" "onnx==1.16.2" "onnxruntime==1.19.2" "numpy<2" \
  safetensors tiktoken more-itertools numba tqdm
"${pip[@]}" --no-build-isolation --no-deps "openai-whisper==20240930"

echo "Downloading NB-Whisper base…"
base="https://huggingface.co/NbAiLab/nb-whisper-base/resolve/$NB_REVISION"
curl -fL --retry 5 -o model.safetensors "$base/model.safetensors"
curl -fL --retry 5 -o config.json "$base/config.json"
curl -fL --retry 5 -o export-onnx.py \
  "https://raw.githubusercontent.com/k2-fsa/sherpa-onnx/$SHERPA_VERSION/scripts/whisper/export-onnx.py"

echo "Converting to the original Whisper layout…"
"$python" - <<'EOF'
import json, torch, whisper
from safetensors.torch import load_file

cfg = json.load(open("config.json"))
# The reverse of transformers' convert_openai_to_hf mapping, most specific first.
pairs = [
    ("encoder.embed_positions.weight", "encoder.positional_embedding"),
    ("decoder.embed_positions.weight", "decoder.positional_embedding"),
    ("encoder.layer_norm", "encoder.ln_post"),
    ("decoder.layer_norm", "decoder.ln"),
    (".self_attn.q_proj", ".attn.query"), (".self_attn.k_proj", ".attn.key"),
    (".self_attn.v_proj", ".attn.value"), (".self_attn.out_proj", ".attn.out"),
    (".self_attn_layer_norm", ".attn_ln"),
    (".encoder_attn.q_proj", ".cross_attn.query"), (".encoder_attn.k_proj", ".cross_attn.key"),
    (".encoder_attn.v_proj", ".cross_attn.value"), (".encoder_attn.out_proj", ".cross_attn.out"),
    (".encoder_attn_layer_norm", ".cross_attn_ln"),
    (".final_layer_norm", ".mlp_ln"), (".fc1", ".mlp.0"), (".fc2", ".mlp.2"),
    ("embed_tokens", "token_embedding"), (".layers.", ".blocks."),
]
state = {}
for k, v in load_file("model.safetensors").items():
    if k == "proj_out.weight":  # tied to the token embedding
        continue
    k = k.removeprefix("model.")
    for a, b in pairs:
        k = k.replace(a, b)
    state[k] = v.float()
dims = dict(
    n_mels=cfg["num_mel_bins"], n_vocab=cfg["vocab_size"],
    n_audio_ctx=cfg["max_source_positions"], n_audio_state=cfg["d_model"],
    n_audio_head=cfg["encoder_attention_heads"], n_audio_layer=cfg["encoder_layers"],
    n_text_ctx=cfg["max_target_positions"], n_text_state=cfg["d_model"],
    n_text_head=cfg["decoder_attention_heads"], n_text_layer=cfg["decoder_layers"],
)
model = whisper.model.Whisper(whisper.model.ModelDimensions(**dims))
missing, unexpected = model.load_state_dict(state, strict=False)
assert not unexpected, unexpected
assert all("alignment" in m for m in missing), missing
torch.save({"dims": dims, "model_state_dict": state}, "nb-base.pt")
EOF

echo "Exporting with sherpa-onnx's script…"
# Teach the script one more checkpoint name, loaded from the file just written.
"$python" - <<'EOF'
s = open("export-onnx.py").read()
s = s.replace('"medium-aishell",', '"medium-aishell", "nb-base",', 1)
s = s.replace('    elif name == "medium-aishell":',
              '    elif name == "nb-base":\n        return whisper.load_model("./nb-base.pt")\n    elif name == "medium-aishell":', 1)
open("export-onnx.py", "w").write(s)
EOF
"$python" export-onnx.py --model nb-base >export.log 2>&1 || { tail -20 export.log; exit 1; }

mkdir -p "$target/$NAME"
cp nb-base-encoder.int8.onnx nb-base-decoder.int8.onnx nb-base-tokens.txt "$target/$NAME/"
echo "Installed $target/$NAME"
echo "Mural uses it from the next Norwegian conversation."
