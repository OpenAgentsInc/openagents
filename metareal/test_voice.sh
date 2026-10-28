#!/usr/bin/env bash
# The voice engine end to end through its C face, under ASan/UBSan:
#   metareal/test_voice.sh [file.wav ...]
# Runs the Coax checks (and mutation gates for the #authority files), builds
# each voice*.coax as a C library, compiles metareal/platform/voice.c and
# metareal/voice_smoke.c against them, runs synthetic frames (silence, a tone
# burst, speech-like noise) at every input rate, barge-in, decisions, the
# jitter buffer, encode->decode of every Frame kind, the floor and the meter,
# then any WAVs given. If protoc is on PATH, every encoded frame is also
# decoded by protoc against voice.proto, an independent implementation.
# Generated names: voice.coax -> VoiceEngine.{c,h} (not Voice.h: on a
# case-insensitive disk that is platform/voice.h), voice-wire -> VoiceWire,
# voice-floor -> VoiceFloor, voice-meter -> VoiceMeter.
set -euo pipefail
cd "$(dirname "$0")/.."
work=$(mktemp -d "${TMPDIR:-/tmp}/metareal-voice.XXXXXX")
trap 'rm -rf "$work"' EXIT
coax=coax/bin/coax
for f in voice voice-wire voice-floor voice-meter; do
  "$coax" test "metareal/$f.coax" --steps 20000000 | tail -1 | sed "s|^|$f: |"
done
for f in voice-floor voice-meter; do
  "$coax" mutate "metareal/$f.coax" | tail -1 | sed "s|^|$f mutate: |"
done
for pair in voice:VoiceEngine voice-wire:VoiceWire voice-floor:VoiceFloor voice-meter:VoiceMeter; do
  src=${pair%%:*}; base=${pair##*:}
  "$coax" c "metareal/$src.coax" --library --header "$work/$base.h" > "$work/$base.c"
done
cflags=(-std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=undefined)
"${CC:-cc}" "${cflags[@]}" -w -I "$work" -c "$work/VoiceEngine.c" -o "$work/e.o"
"${CC:-cc}" "${cflags[@]}" -w -I "$work" -c "$work/VoiceWire.c" -o "$work/w.o"
"${CC:-cc}" "${cflags[@]}" -w -I "$work" -c "$work/VoiceFloor.c" -o "$work/f.o"
"${CC:-cc}" "${cflags[@]}" -w -I "$work" -c "$work/VoiceMeter.c" -o "$work/m.o"
"${CC:-cc}" "${cflags[@]}" -Wall -Wextra -Wno-missing-field-initializers -I "$work" -I metareal/platform \
  metareal/platform/voice.c metareal/voice_smoke.c "$work"/e.o "$work"/w.o "$work"/f.o "$work"/m.o -lm -o "$work/smoke"
mkdir -p "$work/frames"
"$work/smoke" --frames "$work/frames" "$@"
if command -v protoc >/dev/null; then
  for k in 1 2 3 4 5 6 7 8 9 10 11; do
    protoc -I proto --decode=openagents.voice.v1alpha1.Frame proto/openagents/voice/v1alpha1/voice.proto \
      < "$work/frames/$k.bin" > "$work/frames/$k.txt"
  done
  grep -q 'token: "session-token"' "$work/frames/1.txt"
  grep -q 'owed_msat: 31' "$work/frames/8.txt"
  grep -q 'decider: "voice.coax"' "$work/frames/11.txt"
  grep -q 'gain_db: 0' "$work/frames/11.txt"   # explicit zero kept by presence
  grep -q 'text: "You said: what' "$work/frames/6.txt"
  echo "protoc decodes all 11 frame kinds"
fi
