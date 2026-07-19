#!/usr/bin/env bash
# Transcription latency for a 10 s utterance, pinned to 4 cores (the bar's
# reference machine). Numbers land in docs/research/latency-m1.md.
# Models are read from ~/.cache/dictaforge/; missing ones are skipped.
set -euo pipefail
cd "$(dirname "$0")/.."

for model in ggml-tiny.en.bin ggml-small.bin ggml-large-v3-turbo.bin; do
    path="$HOME/.cache/dictaforge/$model"
    if [ ! -f "$path" ]; then
        echo "skip $model (not cached)"
        continue
    fi
    DICTAFORGE_TEST_MODEL="$path" taskset -c 0-3 \
        cargo test --release -p dictaforged -- --ignored --nocapture latency_10s \
        2>&1 | grep "latency model"
done
