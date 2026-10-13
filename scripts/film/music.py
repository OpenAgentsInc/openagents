#!/usr/bin/env python3
"""Synthesizes the launch film's music bed from code (#11432).

The score sits on a 120 BPM grid, so every cut in docs/verse/film/cut.json can
land on a beat. Sections:
  0-2 s       a drone swelling into a reverse riser; impact on the title at 2 s
  4 s on      pads and an arpeggio pulse over Dm-Bb-F-C, with a kick from 12 s
  last 16 s   the full arrangement, and a riser into the end card
  last 8 s    an impact and a resolving chord under the end card
Whooshes sit on the product cuts given by --cuts.

Usage: scripts/film/music.py OUT.wav [--seconds 42] [--cuts 12,20,27] [--end 34]
Writes a 48 kHz stereo WAV. scripts/film/cut.py normalizes loudness when it
mixes the film.
"""

import argparse
import wave

import numpy as np

SR = 48_000
BEAT = 0.5  # 120 BPM
RNG = np.random.default_rng(290)


def note(name):
    names = {"C": 0, "C#": 1, "D": 2, "Eb": 3, "E": 4, "F": 5, "F#": 6, "G": 7, "A": 9, "Bb": 10, "B": 11}
    pitch, octave = name[:-1], int(name[-1])
    return 440.0 * 2 ** ((names[pitch] + 12 * (octave + 1) - 69) / 12)


CUTS = []  # the product cuts that get a whoosh; set from --cuts

CHORDS = [  # each chord lasts 4 s (two bars)
    ["D3", "A3", "D4", "F4", "A4"],
    ["Bb2", "F3", "Bb3", "D4", "F4"],
    ["F2", "C3", "F3", "A3", "C4"],
    ["C3", "G3", "C4", "E4", "G4"],
]


def t_axis(seconds):
    return np.arange(int(seconds * SR)) / SR


def saw(freq, seconds, bright, detune=0.0):
    """A band-limited saw whose harmonics roll off above `bright` Hz."""
    t = t_axis(seconds)
    f = freq * (1 + detune)
    out = np.zeros_like(t)
    n = 1
    while n * f < min(12_000, SR / 2 - 1000):
        rolloff = 1 / (1 + (n * f / bright) ** 2)
        out += np.sin(2 * np.pi * n * f * t + RNG.uniform(0, 2 * np.pi)) * rolloff / n
        n += 1
    return out


def env(seconds, attack, release, curve=1.0):
    t = t_axis(seconds)
    a = np.clip(t / max(attack, 1e-4), 0, 1)
    r = np.clip((seconds - t) / max(release, 1e-4), 0, 1)
    return (a * r) ** curve


class Track:
    def __init__(self, seconds):
        self.buf = np.zeros((int(seconds * SR) + SR, 2))
        self.seconds = seconds

    def add(self, at, sig, gain=1.0, pan=0.0):
        i = int(at * SR)
        sig = sig[: max(0, len(self.buf) - i)]
        left, right = np.cos((pan + 1) * np.pi / 4), np.sin((pan + 1) * np.pi / 4)
        self.buf[i : i + len(sig), 0] += sig * gain * left * np.sqrt(2)
        self.buf[i : i + len(sig), 1] += sig * gain * right * np.sqrt(2)

    def out(self):
        return self.buf[: int(self.seconds * SR)]


def fft_filter(sig, low=None, high=None):
    spec = np.fft.rfft(sig)
    freqs = np.fft.rfftfreq(len(sig), 1 / SR)
    gain = np.ones_like(freqs)
    if high:
        gain *= 1 / np.sqrt(1 + (freqs / high) ** 4)
    if low:
        gain *= 1 / np.sqrt(1 + (low / np.maximum(freqs, 1e-3)) ** 4)
    return np.fft.irfft(spec * gain, len(sig))


def swept_noise(seconds, f0, f1, q=0.6):
    """Noise through a band-pass that sweeps from f0 to f1 (a riser or a whoosh)."""
    hop, size = 1024, 4096
    n = int(seconds * SR)
    noise = RNG.standard_normal(n + size)
    out = np.zeros(n + size)
    win = np.hanning(size)
    freqs = np.fft.rfftfreq(size, 1 / SR)
    for start in range(0, n, hop):
        p = start / n
        center = f0 * (f1 / f0) ** p
        band = np.exp(-((np.log2(np.maximum(freqs, 1) / center)) ** 2) / (2 * q**2))
        chunk = np.fft.irfft(np.fft.rfft(noise[start : start + size] * win) * band, size)
        out[start : start + size] += chunk * win
    return out[:n] / (np.max(np.abs(out[:n])) + 1e-9)


def kick(seconds=0.5, top=110.0, bottom=42.0):
    t = t_axis(seconds)
    freq = bottom + (top - bottom) * np.exp(-t * 28)
    phase = 2 * np.pi * np.cumsum(freq) / SR
    click = fft_filter(RNG.standard_normal(len(t)), high=3500) * np.exp(-t * 300) * 0.15
    return (np.sin(phase) + click) * np.exp(-t * 7)


def impact(seconds=4.0):
    t = t_axis(seconds)
    boom = kick(seconds, 80, 32) * 1.2 + np.sin(2 * np.pi * 36 * t) * np.exp(-t * 1.1) * 0.7
    crack = fft_filter(RNG.standard_normal(len(t)), high=2500) * np.exp(-t * 9) * 0.5
    return boom + crack


def pluck(freq, seconds=0.45):
    t = t_axis(seconds)
    sig = sum(np.sin(2 * np.pi * freq * k * t) / k ** 1.6 * np.exp(-t * (6 + 3 * k)) for k in range(1, 7))
    return sig * np.minimum(t / 0.004, 1)


def reverb(stereo, seconds=2.6, wet=0.28):
    n = int(seconds * SR)
    t = t_axis(seconds)
    out = np.empty_like(stereo)
    size = 1 << int(np.ceil(np.log2(len(stereo) + n)))
    for ch in range(2):
        ir = RNG.standard_normal(n) * np.exp(-t * 6.9 / seconds)
        ir = fft_filter(ir, low=180, high=6000)
        ir /= np.sqrt(np.sum(ir**2))
        conv = np.fft.irfft(np.fft.rfft(stereo[:, ch], size) * np.fft.rfft(ir, size), size)
        out[:, ch] = stereo[:, ch] * (1 - wet) + conv[: len(stereo)] * wet * 3
    return out


def score(seconds, end=None):
    tr = Track(seconds)
    # The end card's impact; the chord resolves under it.
    end = seconds - 8.0 if end is None else end

    # 0-2 s: a low drone swelling into a reverse riser; the title lands at 2 s.
    for f, pan in [(note("D2"), -0.3), (note("A2"), 0.3), (note("D3"), 0.0)]:
        for d in (-0.004, 0.004):
            tr.add(0, saw(f, 6.0, 260, d) * env(6.0, 1.5, 3.0), 0.06, pan + d * 60)
    tr.add(0.0, swept_noise(2.0, 300, 7000) * np.linspace(0, 1, int(2.0 * SR)) ** 3, 0.22)
    tr.add(2.0, impact(), 0.9)

    # From 4 s: pads following the progression, opening up for the last stretch.
    for i, at in enumerate(np.arange(4.0, end, 4.0)):
        chord = CHORDS[i % 4]
        late = at >= end - 8.0
        for j, name in enumerate(chord):
            for d in (-0.005, 0.005):
                pad = saw(note(name), 4.6, 1800 if late else 700, d) * env(4.6, 0.6, 1.0)
                tr.add(at, pad, 0.045 if late else 0.035, (j - 2) * 0.25 + d * 50)

    # From 4 s: an eighth-note arpeggio pulse that grows toward the end card.
    for k, at in enumerate(np.arange(4.0, end, BEAT / 2)):
        chord = CHORDS[int((at - 4.0) // 4) % 4]
        pattern = [2, 3, 4, 3, 2, 4, 3, 4]
        f = note(chord[pattern[k % 8]]) * (2 if at >= end - 8.0 else 1)
        accent = 1.0 if k % 2 == 0 else 0.7
        ramp = min(1.0, 0.45 + (at - 4.0) / (end - 4.0))
        tr.add(at, pluck(f), 0.11 * accent * ramp, 0.35 if k % 2 else -0.35)

    # From 12 s: a kick on the beat, with off-beat ghosts in the last stretch.
    for at in np.arange(12.0, end, BEAT):
        late = at >= end - 8.0
        tr.add(at, kick(), 0.6 if late else 0.42)
        if late:
            tr.add(at + BEAT / 2, kick(0.25, 90, 50), 0.12)

    # A riser into the end card, and soft whooshes on the product cuts.
    tr.add(end - 4.0, swept_noise(4.0, 250, 9000) * np.linspace(0, 1, 4 * SR) ** 2.5, 0.25)
    for at in CUTS:
        tr.add(at - 0.35, swept_noise(0.7, 600, 5000) * np.hanning(int(0.7 * SR)), 0.12)

    # The end card: an impact and a resolving D major chord, fading out.
    tr.add(end, impact(6.0), 0.85)
    tail = seconds - end
    for j, name in enumerate(["D2", "A2", "D3", "F#3", "A3", "D4", "E4"]):
        for d in (-0.004, 0.004):
            tr.add(end, saw(note(name), tail, 1400, d) * env(tail, 0.08, 4.5, 1.3), 0.05, (j - 3) * 0.2 + d * 50)

    mix = reverb(tr.out())
    mix = np.tanh(mix * 1.2) / np.tanh(1.2)  # gentle glue
    return mix / np.max(np.abs(mix)) * 0.89


def write_wav(path, stereo):
    data = (np.clip(stereo, -1, 1) * 32767).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(data.tobytes())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--seconds", type=float, default=42.0)
    ap.add_argument("--cuts", default="12,20,27", help="seconds of the product cuts, comma-separated")
    ap.add_argument("--end", type=float, help="when the end card starts (default: 8 s before the end)")
    args = ap.parse_args()
    CUTS.extend(float(c) for c in args.cuts.split(",") if c)
    write_wav(args.out, score(args.seconds, args.end))
    print(args.out)


if __name__ == "__main__":
    main()
