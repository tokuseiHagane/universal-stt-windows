#!/usr/bin/env bash
# Rebuilds tests/fixtures/two_speakers.wav from public-domain speech:
# JFK (whisper.cpp sample) and LJSpeech (Coqui TTS test data).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/tests/fixtures/two_speakers.wav"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

jfk_url="https://github.com/ggerganov/whisper.cpp/raw/master/samples/jfk.wav"
lj_url="https://github.com/coqui-ai/TTS/raw/dev/tests/data/ljspeech/wavs/LJ001-0001.wav"
jfk_sha="59dfb9a4acb36fe2a2affc14bacbee2920ff435cb13cc314a08c13f66ba7860e"
lj_sha="4d2bd83ff0f7fe33491b03193abdf623f1b4cc2d8103972b33638d3607d86dc5"

curl -L --fail --retry 3 -o "$tmp/jfk.wav" "$jfk_url"
curl -L --fail --retry 3 -o "$tmp/lj.wav" "$lj_url"
echo "$jfk_sha  $tmp/jfk.wav" | sha256sum -c -
echo "$lj_sha  $tmp/lj.wav" | sha256sum -c -

ffmpeg -y -v error -i "$tmp/jfk.wav" -af "atrim=start=1:end=5,asetpts=PTS-STARTPTS" \
  -ar 16000 -ac 1 -c:a pcm_s16le "$tmp/a.wav"
ffmpeg -y -v error -i "$tmp/lj.wav" -af "atrim=start=1:end=5,asetpts=PTS-STARTPTS" \
  -ar 16000 -ac 1 -c:a pcm_s16le "$tmp/b.wav"
ffmpeg -y -v error -f lavfi -i anullsrc=r=16000:cl=mono -t 0.5 \
  -c:a pcm_s16le "$tmp/g.wav"

for part in a g b g a g b; do echo "file '$part.wav'"; done > "$tmp/list.txt"
ffmpeg -y -v error -f concat -safe 0 -i "$tmp/list.txt" -c:a pcm_s16le \
  -ar 16000 -ac 1 "$out"
echo "wrote $out"
