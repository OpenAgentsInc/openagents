"""Independent reference values for chapter 151 (psionic/lib/spectral.coax).

Tooling only. Recomputes the mel filter banks and log-mel features that the
unit's #test blocks hard-code, from the published formulas and nothing else:
the direct DFT sum with Python's cmath, torch.stft's framing (center padding
of n_fft // 2, the window centred inside n_fft), librosa.filters.mel's
construction (numpy.linspace of mel points, numpy.fft.rfftfreq bin centres,
triangles max(0, min(lower, upper)), Slaney area normalisation 2 / (f[i+2] -
f[i])), the Slaney and HTK mel scales, NeMo's log zero guard and NeMo's
per-feature normalisation (n - 1 variance, 1e-5 added to the deviation).
No numpy, librosa or torch is imported: the formulas are written out here.
"""
import cmath
import math

LOGSTEP = math.log(6.4) / 27.0


def hz_to_mel(f, htk):
    if htk:
        return 2595.0 * math.log10(1.0 + f / 700.0)
    if f >= 1000.0:
        return 15.0 + math.log(f / 1000.0) / LOGSTEP
    return f / (200.0 / 3)


def mel_to_hz(m, htk):
    if htk:
        return 700.0 * (10.0 ** (m / 2595.0) - 1.0)
    if m >= 15.0:
        return 1000.0 * math.exp(LOGSTEP * (m - 15.0))
    return (200.0 / 3) * m


def linspace(a, b, n):
    step = (b - a) / (n - 1)
    out = [a + i * step for i in range(n)]
    out[-1] = b
    return out


def mel_bank(rate, n_fft, mels, fmin, fmax, htk, slaney_norm):
    bins = n_fft // 2 + 1
    freqs = [k * (1.0 / (n_fft * (1.0 / rate))) for k in range(bins)]
    edges = [mel_to_hz(m, htk) for m in linspace(hz_to_mel(fmin, htk), hz_to_mel(fmax, htk), mels + 2)]
    bank = []
    for i in range(mels):
        row = []
        for f in freqs:
            lower = (f - edges[i]) / (edges[i + 1] - edges[i])
            upper = (edges[i + 2] - f) / (edges[i + 2] - edges[i + 1])
            w = max(0.0, min(lower, upper))
            if slaney_norm:
                w *= 2.0 / (edges[i + 2] - edges[i])
            row.append(w)
        bank.append(row)
    return bank


def window(kind, n, periodic):
    d = n if periodic else n - 1
    if n == 1:
        return [1.0]
    a = {"hann": 0.5, "hamming": 0.54}[kind]
    return [a - (1 - a) * math.cos(2 * math.pi * i / d) for i in range(n)]


def power_frames(x, n_fft, win, hop, w, preemph):
    y = [x[0]] + [x[i] - preemph * x[i - 1] for i in range(1, len(x))]
    pad = n_fft // 2
    padded = [0.0] * pad + y + [0.0] * pad
    placed = [0.0] * n_fft
    left = (n_fft - win) // 2
    for i in range(win):
        placed[left + i] = w[i]
    frames = 1 + (len(padded) - n_fft) // hop
    out = []
    for t in range(frames):
        seg = [padded[t * hop + j] * placed[j] for j in range(n_fft)]
        row = []
        for k in range(n_fft // 2 + 1):
            s = sum(seg[j] * cmath.exp(-2j * math.pi * j * k / n_fft) for j in range(n_fft))
            row.append(abs(s) ** 2)
        out.append(row)
    return out


def features(x, rate, n_fft, win, hop, mels, preemph):
    p = power_frames(x, n_fft, win, hop, window("hann", win, False), preemph)
    bank = mel_bank(rate, n_fft, mels, 0.0, rate / 2, False, True)
    guard = 2.0 ** -24
    f = [[math.log(sum(b * v for b, v in zip(bank[m], row)) + guard) for row in p] for m in range(mels)]
    out = []
    for row in f:
        n = len(row)
        mean = sum(row) / n
        std = math.sqrt(sum((v - mean) ** 2 for v in row) / (n - 1)) + 1e-5
        out.append([(v - mean) / std for v in row])
    return out


def signal(n):
    # The chapter's test waveform: two tones and a step, kept inside [-1, 1].
    return [0.5 * math.sin(2 * math.pi * 1000 * i / 16000) + 0.25 * math.cos(2 * math.pi * 3000 * i / 16000)
            + (0.125 if i >= n // 2 else 0.0) for i in range(n)]


def show(name, rows):
    print(name)
    for r in rows:
        print("  " + ",".join(repr(v) for v in r))


if __name__ == "__main__":
    show("slaney bank 16 kHz, n_fft 16, 4 mels, slaney norm", mel_bank(16000, 16, 4, 0.0, 8000.0, False, True))
    show("htk bank 16 kHz, n_fft 16, 4 mels, no norm", mel_bank(16000, 16, 4, 0.0, 8000.0, True, False))
    show("signal 40", [signal(40)])
    show("features 16 kHz, n_fft 16, win 12, hop 4, 4 mels, preemph 0.97", features(signal(40), 16000, 16, 12, 4, 4, 0.97))
