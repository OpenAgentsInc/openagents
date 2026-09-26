---
id: method.audio-note-transcription
version: 1
kind: method
title: Transcribe audio to notes with onsets, pitch tracking, and note-level scoring
summary: >-
  Turn audio into notes by detecting onsets, tracking fundamental frequency
  with a probabilistic pitch tracker, converting Hz to MIDI numbers, and
  quantizing timing to the beat. Separate simultaneous voices by range and
  continuity, and score the result with note-level onset and pitch tolerances.
tags: [audio, music, transcription, pitch-tracking, midi, librosa, musicxml]
applies_when: >-
  Converting recorded or synthesized music to notes (MIDI, MusicXML, or a note
  list), including separating several voices or instruments.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Mauch and Dixon, pYIN: A Fundamental Frequency Estimator Using Probabilistic Threshold Distributions (ICASSP 2014)"
    - "McFee et al., librosa: Audio and Music Signal Analysis in Python (SciPy 2015); librosa docs for pyin, onset_detect, beat_track, cqt"
    - "Raffel et al., mir_eval: A Transparent Implementation of Common MIR Metrics (ISMIR 2014), transcription metrics"
    - "Bittner et al., A Lightweight Instrument-Agnostic Model for Polyphonic Note Transcription and Multipitch Estimation (ICASSP 2022)"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**Single voice.** Load mono audio at a known rate. Track f0 with
`librosa.pyin(y, fmin, fmax, sr=sr)` using a range that matches the voice or
instrument (a narrow range cuts octave errors); it returns f0 per frame plus a
voiced flag and probability. Detect note starts with
`librosa.onset.onset_detect(..., backtrack=True)`, segment between onsets and
unvoiced gaps, take the median f0 of each segment, and convert with
`midi = 69 + 12 * log2(f / 440)`, rounding to the nearest semitone (unless the
tuning reference is not A440; estimate it with `librosa.estimate_tuning`).
Repeated notes at the same pitch are split only by onsets or amplitude dips,
not by pitch change.

**Several simultaneous voices.** A monophonic tracker follows the loudest or
lowest partial and jumps between voices. Options, in rising cost: if voices
are on separate channels or stems, transcribe each; else compute a constant-Q
transform (`librosa.cqt`) or chroma and pick peaks per frame, then assign
pitches to voices by register limits and by minimal pitch movement between
consecutive notes (voices rarely cross); or use a pretrained multipitch model
if one is installed. Harmonics at 2f and 3f are the classic false positives;
suppress a peak when a stronger peak sits an octave or a twelfth below.

**Rhythm.** Estimate tempo and beats (`librosa.beat.beat_track`) or use a
given tempo, then express onsets and durations in beats and quantize to the
smallest grid the music uses (for example sixteenths). Write MIDI with
`pretty_midi` or `mido`, and MusicXML with `music21`, setting time signature,
key, and a part per voice.

## How to check

Score against any reference you have with note-level metrics
(`mir_eval.transcription.precision_recall_f1_overlap`, default onset tolerance
50 ms and pitch tolerance 50 cents). Without a reference, resynthesize the
transcription (`pretty_midi` synthesis or a SoundFont) and compare its chroma
or CQT with the original frame by frame, listen for octave jumps, and check
every voice stays inside its expected range.
