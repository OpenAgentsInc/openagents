# 151. Waveforms and spectral features

> **Status: partly implemented.** Audio admission, windows, a direct DFT, an
> FFT of any size up to 4,096 and its inverse, spectrograms, mel filter
> banks, log-mel features and per-feature normalization run on the CPU and
> are checked against an independent oracle. Matching recorded features of
> the pinned Parakeet model is chapter 153's job and is not done yet.

Speech models do not read raw audio samples. They read spectral features,
and those features depend on exact conventions: the sample rate, the window
and where it sits in the frame, the hop, how the edges are padded, which mel
scale, how the filters are normalized, how a logarithm of zero is avoided,
and how each feature is standardized. Change any of them and a model trained
on the other convention hears something different. This chapter writes every
convention down as a field of one record and builds the features in a library
unit, [`spectral.coax`](../lib/spectral.coax):

- audio admission with a typed refusal for each way audio can be wrong;
- Hann, Hamming and rectangular windows, periodic or symmetric;
- a direct discrete Fourier transform (DFT) as the reference, then a fast
  Fourier transform (FFT) of any size from 1 to 4,096 and its inverse;
- a power spectrogram with zero or reflected padding and preemphasis;
- mel filter banks on the Slaney or HTK scale, with or without Slaney's area
  normalization;
- log-mel features with an added or clamped guard, and per-feature
  normalization.

The verse voice engine (`verse/voice.coax`) already resamples microphone
audio to 16 kHz and measures levels and voice activity; it has no transform.
The water renderer (`verse/water-spectrum.coax`) has a two-dimensional
single-precision inverse FFT tied to its ocean tiles. Neither fits here: this
unit works in binary64, in one dimension, on any size, with a reference to
check it against. Audio at another rate is refused, not resampled; the caller
resamples first, for example with the voice engine.

```shell
coax/bin/coax run psionic/examples/spectral-features.coax
coax/bin/coax test psionic/lib/spectral.coax
psionic/check.sh --only spectral
python3 -I psionic/tests/spectral_oracle.py
```

```text
x = 1 2 3 4
  DFT: 10.0 0.0 i -2.0 2.0 i -2.0 0.0 i -2.0 -2.0 i
  FFT: 10.0 0.0 i -2.0 2.0 i -2.0 0.0 i -2.0 -2.0 i
  inverse: 1.0 0.0 i 2.0 0.0 i 3.0 0.0 i 4.0 0.0 i
n = 6 (chirp): FFT within 1e-12 of the DFT: true; inverse returns the input within 1e-14: true
n = 400 (chirp): FFT within 1e-12 of the DFT: true; inverse returns the input within 1e-14: true
n = 512 (radix-2): FFT within 1e-12 of the DFT: true; inverse returns the input within 1e-14: true
zero padding: 3 frames x 3 bins, power x 64: 9.0 5.0 1.0 100.0 8.0 4.0 49.0 25.0 1.0
reflect padding: 3 frames x 3 bins, power x 64: 64.0 4.0 0.0 100.0 8.0 4.0 144.0 4.0 0.0
mel bank: 128 filters x 257 bins; filter 0 weights: 0.0 28.3775 0.0 (x 1000)
tone: 51 frames; frame 25 peaks in filter 42 (centre 1005.6453 Hz), log-mel 2.4581; filter 0 there: -16.6107
per-feature normalized, filter 42: mean 0.0, n - 1 deviation 1.0
48 kHz audio for a 16 kHz spec: rate
two channels: channels
a NaN sample: number
a sample of 1.5: number
no samples: empty
an 8192-point frame: size
64 mels on 33 bins: filter
FFT with 3 real and 2 imaginary parts: shape
```

The [example](../examples/spectral-features.coax) compiles and runs in a few
seconds.

## The transform, worked by hand

The DFT of n samples x[0..n−1] is

X[k] = Σⱼ x[j] · e^(−2πi·jk/n), for k = 0 … n−1.

For x = 1 2 3 4, n = 4 and the twiddle e^(−2πi/4) is −i, so its powers are
1, −i, −1, i:

- X[0] = 1 + 2 + 3 + 4 = 10
- X[1] = 1 + 2(−i) + 3(−1) + 4(i) = −2 + 2i
- X[2] = 1 − 2 + 3 − 4 = −2
- X[3] = 1 + 2(i) + 3(−1) + 4(−i) = −2 − 2i

The first lines of the transcript show the direct DFT and the FFT agreeing
exactly. They agree exactly because the twiddles at quarter turns are
written as 0 and ±1 rather than computed with cosine, which would give
6.1 × 10⁻¹⁷ instead of 0.

The FFT gets the same answer in two halving steps. It reorders the input by
reversed index bits (1 3 2 4), transforms the pairs (1, 3) → (4, −2) and
(2, 4) → (6, −2), and merges them with one butterfly per output:
X[0] = 4 + 6, X[2] = 4 − 6, X[1] = −2 + (−i)(−2) = −2 + 2i and
X[3] = −2 − (−i)(−2) = −2 − 2i.

## Twiddles

Every transform in the unit takes its twiddles from one function. It reduces
the turn r/d into (−1/2, 1/2] before the angle is formed, so the angle stays
small, and writes the four quarter turns exactly.

<!-- source: psionic/lib/spectral.coax turn -->
```jai
// e^(-2 pi i r/d): r is reduced into (-d/2, d/2] so the angle stays small,
// and the four quarter turns are written exactly.
sf_turn :: (r:int,d:int)->SfTurn {
    k:=r%d;if 2*k>d {k-=d;}
    if k==0 {return SfTurn.{re=1,im=0};}
    if 2*k==d {return SfTurn.{re=-1,im=0};}
    if 4*k==d {return SfTurn.{re=0,im=-1};}
    if 4*k== -d {return SfTurn.{re=0,im=1};}
    a:=-2*SF_PI*f64(k)/f64(d);
    return SfTurn.{re=cos(a),im=sin(a)};
}
```

## The reference: a direct DFT

The direct DFT costs n² multiplications, which is why it is only a
reference. It sums each output with chapter 4's compensated accumulator, so
its own rounding error is about one unit in the last place of the result.

<!-- source: psionic/lib/spectral.coax dft -->
```jai
// The reference: X[k] = sum over j of x[j] e^(-2 pi i jk/n), each sum in
// order with the compensated accumulator. The inverse turns the other way
// and divides by n. Output must not share storage with input.
sf_dft :: (re:[]f64,im:[]f64,out_re:[]f64,out_im:[]f64,inverse:bool)->SfStatus {
    status:=sf_complex_ok(re,im);if status!=SfStatus.ok {return status;}
    n:=re.count;
    if out_re.count<n || out_im.count<n {return SfStatus.capacity;}
    for k:0..<n {
        sr:PsionicAccumulator;si:PsionicAccumulator;
        for j:0..<n {
            r:=(j*k)%n;if inverse {r=(n-r)%n;}
            t:=sf_turn(r,n);
            sr=psionic_ops_accumulate(sr,re[j]*t.re);sr=psionic_ops_accumulate(sr,-im[j]*t.im);
            si=psionic_ops_accumulate(si,re[j]*t.im);si=psionic_ops_accumulate(si,im[j]*t.re);
        }
        out_re[k]=psionic_ops_total(sr);out_im[k]=psionic_ops_total(si);
        if inverse {out_re[k]/=f64(n);out_im[k]/=f64(n);}
    }
    return SfStatus.ok;
}
```

## The fast transform

A power of two uses radix-2 Cooley-Tukey in place: reverse the index bits,
then merge half transforms of length 1, 2, 4 and so on.

<!-- source: psionic/lib/spectral.coax radix2 -->
```jai
// Radix-2 Cooley-Tukey in place for a power of two: reverse the index bits,
// then merge pairs of half transforms with one butterfly each.
sf_radix2 :: (re:[]f64,im:[]f64) {
    n:=re.count;sr:[SF_CHIRP_MAX]f64;si:[SF_CHIRP_MAX]f64;
    for i:0..<n {r:=sf_reverse(i,n);sr[r]=re[i];si[r]=im[i];}
    sf_butterflies(sr[0..<n],si[0..<n]);
    for i:0..<n {re[i]=sr[i];im[i]=si[i];}
}
// i with its log2(n) bits in reverse order.
sf_reverse :: (i:int,n:int)->int {
    r:=0;k:=i;m:=n/2;
    while m>=1 {r+=(k%2)*m;k/=2;m/=2;}
    return r;
}
sf_butterflies :: (re:[]f64,im:[]f64) {
    n:=re.count;span:=2;
    while span<=n {
        half:=span/2;
        for k:0..<half {
            w:=sf_turn(k,span);start:=0;
            while start<n {
                a:=start+k;b:=a+half;
                tr:=re[b]*w.re-im[b]*w.im;ti:=re[b]*w.im+im[b]*w.re;
                re[b]=re[a]-tr;im[b]=im[a]-ti;re[a]+=tr;im[a]+=ti;
                start+=span;
            }
        }
        span*=2;
    }
}
```

Speech front ends do not always use powers of two. Whisper frames 400
samples. Bluestein's algorithm handles any size with power-of-two
transforms. It writes jk as (j² + k² − (k − j)²)/2, which turns the DFT into
a convolution with a chirp, and computes that convolution with three
transforms of length m ≥ 2n − 1. The chirp angle uses k² reduced modulo 2n,
so it stays accurate when k² is large.

<!-- source: psionic/lib/spectral.coax chirp -->
```jai
// Twice the smallest power of two at least n: at least 2n - 1 when n is not
// itself a power of two.
sf_chirp_length :: (n:int)->int {m:=1;while m<n {m*=2;}return 2*m;}
// Bluestein: jk = (j^2 + k^2 - (k - j)^2)/2 turns the DFT into a convolution
// with the chirp w[k] = e^(-pi i k^2/n), done with power-of-two transforms
// of length m >= 2n - 1. k^2 is reduced modulo 2n before the angle.
sf_chirp :: (re:[]f64,im:[]f64) {
    n:=re.count;m:=sf_chirp_length(n);
    ar:[SF_CHIRP_MAX]f64;ai:[SF_CHIRP_MAX]f64;br:[SF_CHIRP_MAX]f64;bi:[SF_CHIRP_MAX]f64;
    wr:[SF_FFT_MAX]f64;wi:[SF_FFT_MAX]f64;
    for k:0..<n {
        w:=sf_turn((k*k)%(2*n),2*n);wr[k]=w.re;wi[k]=w.im;
        ar[k]=re[k]*w.re-im[k]*w.im;ai[k]=re[k]*w.im+im[k]*w.re;
        br[k]=w.re;bi[k]= -w.im;
        if k>0 {br[m-k]=w.re;bi[m-k]= -w.im;}
    }
    sf_radix2(ar[0..<m],ai[0..<m]);sf_radix2(br[0..<m],bi[0..<m]);
    // Multiply, then invert by conjugating around a forward transform.
    for k:0..<m {
        r:=ar[k]*br[k]-ai[k]*bi[k];i:=ar[k]*bi[k]+ai[k]*br[k];
        ar[k]=r;ai[k]= -i;
    }
    sf_radix2(ar[0..<m],ai[0..<m]);
    for k:0..<n {
        cr:=ar[k]/f64(m);ci:= -ai[k]/f64(m);
        re[k]=cr*wr[k]-ci*wi[k];im[k]=cr*wi[k]+ci*wr[k];
    }
}
```

The public entry point checks its input and chooses the path. The inverse
conjugates, transforms forward, conjugates back and divides by n.

<!-- source: psionic/lib/spectral.coax fft -->
```jai
sf_forward :: (re:[]f64,im:[]f64) {
    if sf_power_of_two(re.count) {sf_radix2(re,im);} else {sf_chirp(re,im);}
}
// The FFT in place, any size from 1 to 4,096. The inverse conjugates,
// transforms forward, conjugates back and divides by n.
sf_fft :: (re:[]f64,im:[]f64,inverse:bool)->SfStatus {
    status:=sf_complex_ok(re,im);if status!=SfStatus.ok {return status;}
    n:=re.count;
    if inverse {for i:0..<n {im[i]= -im[i];}}
    sf_forward(re,im);
    if inverse {for i:0..<n {re[i]/=f64(n);im[i]= -im[i]/f64(n);}}
    return SfStatus.ok;
}
```

**Precision profile `cpu-f64-spectral-v1`.** The arithmetic is binary64.
On every check in this chapter the FFT is within 10⁻¹⁴ · (1 + Σ|x|) of the
direct DFT, at every size from 1 to 64 and at 400. The inverse returns the
input within 10⁻¹³ at those sizes. The example also shows sizes 6, 400 and
512 within 10⁻¹² of the DFT and returning within 10⁻¹⁴.

## Windows

A frame is multiplied by a window that tapers to zero at its edges, so a cut
in the middle of a waveform doesn't add false high frequencies. A periodic
window of N samples repeats with period N, which suits overlapping frames; a
symmetric one is symmetric about its middle sample. Torch's `hann_window`
defaults to periodic. NeMo asks it for a symmetric window.

<!-- source: psionic/lib/spectral.coax window -->
```jai
// Hann is 0.5 - 0.5 cos(2 pi i/D), Hamming 0.54 - 0.46 cos(2 pi i/D). A
// periodic window has D = N (the window that tiles); a symmetric one
// D = N - 1. A window of one sample is 1.
sf_window :: (kind:SfWindow,periodic:bool,out:[]f64)->SfStatus {
    n:=out.count;
    if n<1 || n>SF_FFT_MAX {return SfStatus.size;}
    sf_fill_window(kind,periodic,out);
    return SfStatus.ok;
}
sf_fill_window :: (kind:SfWindow,periodic:bool,out:[]f64) {
    n:=out.count;d:=n;if !periodic {d=n-1;}
    for i:0..<n {
        if kind==SfWindow.rectangle || n==1 {out[i]=1;continue;}
        a:=0.5;if kind==SfWindow.hamming {a=0.54;}
        t:=sf_turn(i,d);out[i]=a-(1-a)*t.re;
    }
}
```

For N = 4, periodic Hann is 0, 0.5, 1, 0.5. For N = 5, symmetric Hann is
0, 0.5, 1, 0.5, 0.

## Admitting audio

Every field of the spec has a range, and the audio must match it. Each
failure has its own status:

| Status | Cause |
| --- | --- |
| `rate` | the spec's rate is outside 8,000 to 48,000 Hz, or the audio's rate differs from it |
| `size` | n_fft is outside 2 to 4,096 (and, for a transform, a length outside 1 to 4,096) |
| `shape` | another spec field is out of range, or real and imaginary parts differ in length |
| `empty` | no samples |
| `channels` | anything but one channel |
| `length` | more than 480,000 samples, too few for one frame, or too few to reflect |
| `number` | a sample outside [−1, 1], including NaN and infinities |
| `capacity` | an output buffer is too small |
| `filter` | a mel filter falls between two bins and has no weight |

<!-- source: psionic/lib/spectral.coax admit -->
```jai
// A spec is checked field by field; then the audio: present, one channel,
// the spec's rate, at most 480,000 samples, enough samples for one frame,
// every sample in [-1, 1] (which also refuses NaN and infinities).
sf_spec_ok :: (s:SfSpec)->SfStatus {
    if s.rate<SF_RATE_MIN || s.rate>SF_RATE_MAX {return SfStatus.rate;}
    if s.n_fft<2 || s.n_fft>SF_FFT_MAX {return SfStatus.size;}
    if s.win_length<1 || s.win_length>s.n_fft || s.hop<1 || s.hop>s.n_fft || s.mels<1 || s.mels>SF_MELS_MAX {return SfStatus.shape;}
    if !(s.fmin>=0 && s.fmin<s.fmax && s.fmax<=f64(s.rate)/2) {return SfStatus.shape;}
    if !(s.guard>0 && s.guard<=1) || !(s.preemphasis>=0 && s.preemphasis<1) {return SfStatus.shape;}
    return SfStatus.ok;
}
sf_frames :: (n:int,s:SfSpec)->int {
    padded:=n;if s.center {padded+=2*(s.n_fft/2);}
    if padded<s.n_fft {return 0;}
    return 1+(padded-s.n_fft)/s.hop;
}
sf_admit :: (a:SfAudio,s:SfSpec)->SfStatus {
    status:=sf_spec_ok(s);if status!=SfStatus.ok {return status;}
    n:=a.samples.count;
    if n==0 {return SfStatus.empty;}
    if a.channels!=1 {return SfStatus.channels;}
    if a.rate!=s.rate {return SfStatus.rate;}
    if n>SF_SAMPLES_MAX || sf_frames(n,s)==0 {return SfStatus.length;}
    if s.center && s.pad==SfPad.reflect && n<=s.n_fft/2 {return SfStatus.length;}
    for v:a.samples {if !(v>= -1 && v<=1) {return SfStatus.number;}}
    return SfStatus.ok;
}
// 16-bit PCM to [-1, 1): divide by 32,768.
sf_from_pcm16 :: (pcm:[]s16,out:[]f64)->SfStatus {
    if pcm.count==0 {return SfStatus.empty;}
    if out.count<pcm.count {return SfStatus.capacity;}
    for i:0..<pcm.count {out[i]=f64(pcm[i])/32768;}
    return SfStatus.ok;
}
```

## Frames and padding, by hand

With centering, n_fft/2 samples are added to each side, so the first frame
is centred on the first sample. Frame t then starts at t · hop in the padded
signal, and there are 1 + (n + 2·⌊n_fft/2⌋ − n_fft) / hop frames.

Take x = 1 2 3 4 (stored in eighths, so it stays inside [−1, 1]),
n_fft = 4, hop = 2 and a rectangular window. There are 1 + 4/2 = 3 frames.

- **Zero padding** gives 0 0 1 2 3 4 0 0. Frame 0 is 0 0 1 2. Its bins are
  X[0] = 3, X[1] = 0 + 0 − 1 + 2i = −1 + 2i and X[2] = 0 − 0 + 1 − 2 = −1.
  The powers |X|² are 9, 5 and 1. Frame 1 is 1 2 3 4: 100, 8 and 4.
  Frame 2 is 3 4 0 0: X[1] = 3 − 4i, so the powers are 49, 25 and 1.
- **Reflect padding** mirrors the signal without repeating the end sample:
  3 2 1 2 3 4 3 2. Frame 0 is 3 2 1 2, with powers 64, 4 and 0.

Because the samples are eighths, the powers are these numbers divided by 64.
The transcript prints them multiplied back by 64, for both paddings.

When the window is shorter than n_fft, it sits in the middle of the frame,
starting at ⌊(n_fft − win_length)/2⌋, as `torch.stft` places it.
Preemphasis c replaces x[i] by x[i] − c·x[i−1] before padding; with
c = 0.5, the samples 1 2 3 4 become 1 1.5 2 2.5.

<!-- source: psionic/lib/spectral.coax frame -->
```jai
// Sample p of the padded, preemphasized signal. Padding adds n_fft/2 samples
// each side: zeros, or the signal reflected without repeating its end sample.
sf_emphasized :: (x:[]f64,c:f64,i:int)->f64 {
    if i==0 || c==0 {return x[i];}
    return x[i]-c*x[i-1];
}
sf_padded :: (x:[]f64,s:SfSpec,p:int)->f64 {
    i:=p;n:=x.count;
    if s.center {
        i-=s.n_fft/2;
        if i<0 || i>=n {
            if s.pad==SfPad.zero {return 0;}
            if i>=n {i=2*(n-1)-i;} else {i= -i;}
        }
    }
    return sf_emphasized(x,s.preemphasis,i);
}
// The window of win_length samples placed in the middle of n_fft.
sf_place_window :: (s:SfSpec,out:[]f64)->SfStatus {
    status:=sf_spec_ok(s);if status!=SfStatus.ok {return status;}
    if out.count<s.n_fft {return SfStatus.capacity;}
    sf_place(s,out);
    return SfStatus.ok;
}
sf_place :: (s:SfSpec,out:[]f64) {
    for i:0..<s.n_fft {out[i]=0;}
    left:=(s.n_fft-s.win_length)/2;
    sf_fill_window(s.window,s.periodic,out[left..<left+s.win_length]);
}
// The power |X[k]|^2 of frame t, bins 0 to n_fft/2.
sf_frame_power :: (x:[]f64,s:SfSpec,window:[]f64,t:int,out:[]f64) {
    re:[SF_FFT_MAX]f64;im:[SF_FFT_MAX]f64;n:=s.n_fft;
    for j:0..<n {re[j]=sf_padded(x,s,t*s.hop+j)*window[j];}
    sf_forward(re[0..<n],im[0..<n]);
    for k:0..<n/2+1 {out[k]=re[k]*re[k]+im[k]*im[k];}
}
// Frame-major: out[t * bins + k].
sf_spectrogram :: (a:SfAudio,s:SfSpec,out:[]f64)->SfResult {
    status:=sf_admit(a,s);if status!=SfStatus.ok {return SfResult.{status=status};}
    frames:=sf_frames(a.samples.count,s);bins:=s.n_fft/2+1;
    if out.count<frames*bins {return SfResult.{status=SfStatus.capacity};}
    w:[SF_FFT_MAX]f64;sf_place(s,w[..]);
    for t:0..<frames {sf_frame_power(a.samples,s,w[..],t,out[t*bins..<(t+1)*bins]);}
    return SfResult.{frames=frames,bins=bins,mels=s.mels};
}
```

## Mel filter banks

Hearing resolves low frequencies more finely than high ones, so features
pool the power spectrum through triangular filters spaced evenly on a mel
scale.

- **Slaney's scale** (librosa's default) is linear below 1 kHz, at 3 mels
  per 200 Hz, and logarithmic above it, at 27 mels per factor of 6.4. So
  1,000 Hz is 15 mels and 6,400 Hz is 42.
- **HTK's scale** is 2595 · log₁₀(1 + f/700).

For mels filters, mels + 2 edges are spaced evenly in mels from fmin to
fmax. Filter i rises from edge i to edge i+1 and falls to edge i+2. Its
weight at a bin centre f is max(0, min((f − eᵢ)/(eᵢ₊₁ − eᵢ),
(eᵢ₊₂ − f)/(eᵢ₊₂ − eᵢ₊₁))). Slaney normalization then multiplies by
2/(eᵢ₊₂ − eᵢ), so every filter has the same area. The unit writes min(a, b)
as (a + b − |a − b|)/2 and max(0, w) as (w + |w|)/2. The results are the
same to within a rounding, and the oracle comparison below holds within
10⁻¹⁷. Without comparisons, no mutant can change only what happens on a tie,
so the mutation check can't leave a survivor there.

A worked value: at 16 kHz with n_fft = 16, the bins are 1 kHz apart. Four
Slaney mels up to 8 kHz span 0 to 45.2456 mels, so the edges are 9.0491 mels
apart. The first three edges are 0, 603.28 and 1,237.40 Hz. At the 1 kHz bin,
filter 0 has rising slope 1000/603.28 = 1.658 and falling slope
(1237.40 − 1000)/(1237.40 − 603.28) = 0.3744. The smaller is 0.3744, and
normalization multiplies it by 2/1237.40, giving 0.000605. The oracle and the
unit both give 0.0006050957639020401.

<!-- source: psionic/lib/spectral.coax mel -->
```jai
// Slaney's scale is linear below 1 kHz (3 mels per 200 Hz) and logarithmic
// above, 27 mels per factor 6.4; HTK's is 2595 log10(1 + f/700).
sf_hz_to_mel :: (f:f64,scale:SfScale)->f64 {
    if scale==SfScale.htk {return 2595*log(1+f/700)/log(10.0);}
    if f>=1000 {return 15+log(f/1000)/(log(6.4)/27);}
    return f/(200.0/3);
}
sf_mel_to_hz :: (m:f64,scale:SfScale)->f64 {
    if scale==SfScale.htk {return 700*(pow(10.0,m/2595)-1);}
    if m>=15 {return 1000*exp((log(6.4)/27)*(m-15));}
    return (200.0/3)*m;
}
// mels + 2 edges evenly spaced in mels (as numpy's linspace places them) from fmin to fmax; filter i rises
// from edge i to edge i+1 and falls to edge i+2, sampled at the bin centres
// k rate/n_fft (computed as numpy's rfftfreq does). Slaney normalization divides by half the filter's width in
// Hz, so every filter has the same area. A filter with no weight is refused.
sf_mel_edges :: (s:SfSpec,edge:[]f64) {
    lo:=sf_hz_to_mel(s.fmin,s.scale);hi:=sf_hz_to_mel(s.fmax,s.scale);
    // edge.count is mels + 2.
    last:=edge.count-1;step:=(hi-lo)/f64(last);
    for i:0..<edge.count {
        m:=hi;if i<last {m=lo+f64(i)*step;}
        edge[i]=sf_mel_to_hz(m,s.scale);
    }
}
sf_mel_bank :: (s:SfSpec,out:[]f64)->SfResult {
    status:=sf_spec_ok(s);if status!=SfStatus.ok {return SfResult.{status=status};}
    bins:=s.n_fft/2+1;
    if out.count<s.mels*bins {return SfResult.{status=SfStatus.capacity};}
    edge:[SF_MELS_MAX+2]f64;sf_mel_edges(s,edge[0..<s.mels+2]);
    spacing:=1/(f64(s.n_fft)*(1/f64(s.rate)));
    for i:0..<s.mels {
        any:=false;
        for k:0..<bins {
            f:=f64(k)*spacing;
            lower:=(f-edge[i])/(edge[i+1]-edge[i]);upper:=(edge[i+2]-f)/(edge[i+2]-edge[i+1]);
            // min(lower, upper), then max(0, that), without comparisons.
            w:=(lower+upper-abs(lower-upper))/2;w=(w+abs(w))/2;
            if s.norm==SfNorm.slaney {w*=2/(edge[i+2]-edge[i]);}
            if w>0 {any=true;}
            out[i*bins+k]=w;
        }
        if !any {return SfResult.{status=SfStatus.filter};}
    }
    return SfResult.{bins=bins,mels=s.mels};
}
```

The unit refuses a bank in which some filter has no weight. That happens when
there are too many mels for the bins: 64 mels on 33 bins, in the
transcript.

## Log-mel features and normalization

Each frame's power spectrum is multiplied by the bank. The logarithm of each
result is taken with a guard: `add` takes log(v + guard), as NeMo does with
2⁻²⁴; `clamp` takes log(max(v, guard)). Per-feature normalization then
standardizes each mel row over the frames. It subtracts the mean and divides
by the n − 1 standard deviation plus 10⁻⁵, which are NeMo's constants.

By hand: one feature over three frames with values 1, 2 and 6 has mean 3,
deviations −2, −1 and 3, and variance 14/2 = 7. The normalized values are
−2/(√7 + 10⁻⁵), −1/(√7 + 10⁻⁵) and 3/(√7 + 10⁻⁵).

<!-- source: psionic/lib/spectral.coax features -->
```jai
// add: log(v + guard). clamp: log(max(v, guard)), the maximum written as
// (v + guard + |v - guard|)/2.
sf_log :: (v:f64,kind:SfLog,guard:f64)->f64 {
    if kind==SfLog.add {return log(v+guard);}
    return log((v+guard+abs(v-guard))/2);
}
// Per feature: subtract the mean over frames and divide by the n - 1
// standard deviation plus 1e-5.
sf_normalize :: (f:[]f64,mels:int,frames:int)->SfStatus {
    if frames<2 {return SfStatus.length;}
    sf_standardize(f,mels,frames);
    return SfStatus.ok;
}
sf_standardize :: (f:[]f64,mels:int,frames:int) {
    for m:0..<mels {
        row:=f[m*frames..<(m+1)*frames];
        s:PsionicAccumulator;for v:row {s=psionic_ops_accumulate(s,v);}
        mean:=psionic_ops_total(s)/f64(frames);
        d:PsionicAccumulator;for v:row {e:=v-mean;d=psionic_ops_accumulate(d,e*e);}
        deviation:=sqrt(psionic_ops_total(d)/f64(frames-1))+SF_DEVIATION_GUARD;
        for t:0..<frames {row[t]=(row[t]-mean)/deviation;}
    }
}
// Mel-major: out[m * frames + t] = log(sum over k of bank[m][k] power[t][k]),
// then normalized. bank is sf_mel_bank's output for the same spec.
sf_features :: (a:SfAudio,s:SfSpec,bank:[]f64,out:[]f64)->SfResult {
    status:=sf_admit(a,s);if status!=SfStatus.ok {return SfResult.{status=status};}
    frames:=sf_frames(a.samples.count,s);bins:=s.n_fft/2+1;
    if bank.count<s.mels*bins || out.count<s.mels*frames {return SfResult.{status=SfStatus.capacity};}
    if s.normalize==SfNormalize.per_feature && frames<2 {return SfResult.{status=SfStatus.length};}
    w:[SF_FFT_MAX]f64;sf_place(s,w[..]);p:[SF_BINS_MAX]f64;
    for t:0..<frames {
        sf_frame_power(a.samples,s,w[..],t,p[0..<bins]);
        for m:0..<s.mels {
            sum:=0.0;for k:0..<bins {sum+=bank[m*bins+k]*p[k];}
            out[m*frames+t]=sf_log(sum,s.log,s.guard);
        }
    }
    if s.normalize==SfNormalize.per_feature {sf_standardize(out,s.mels,frames);}
    return SfResult.{frames=frames,bins=bins,mels=s.mels};
}
```

`sf_nemo_spec()` holds the settings that NeMo's `FilterbankFeatures` uses
for Parakeet, as its configuration lists them:

- 16 kHz audio and a 512-point frame;
- a 400-sample symmetric Hann window and a 160-sample hop;
- zero-padded centering and preemphasis 0.97;
- 128 Slaney-normalized Slaney mels from 0 to 8 kHz;
- log with 2⁻²⁴ added, then per-feature normalization.

The example runs a 1 kHz tone through it. The loudest filter is the one
centred at 1,005.6 Hz, and after normalization that row has mean 0 and
deviation 1, to four places. These settings have not yet been compared with
features recorded from the real model. Chapter 153 does that.

## The program

<!-- source: psionic/examples/spectral-features.coax example -->
```jai
main :: () {
    // 1. Four points by hand: 1 2 3 4.
    re:=[4]f64.[1,2,3,4];im:[4]f64;dre:[4]f64;dim:[4]f64;
    #assert sf_dft(re[..],im[..],dre[..],dim[..],false)==SfStatus.ok && sf_fft(re[..],im[..],false)==SfStatus.ok;
    print("x = 1 2 3 4\n");show("DFT",dre[..],dim[..]);show("FFT",re[..],im[..]);
    #assert sf_fft(re[..],im[..],true)==SfStatus.ok;show("inverse",re[..],im[..]);

    // 2. Larger sizes against the direct DFT.
    compare(6);compare(400);compare(512);

    // 3. A spectrogram by hand: n_fft 4, hop 2, rectangle, centred.
    x:=[4]f64.[0.125,0.25,0.375,0.5];a:=SfAudio.{rate=16000,channels=1,samples=x[..]};power:[9]f64;
    s:=SfSpec.{rate=16000,n_fft=4,win_length=4,hop=2,window=SfWindow.rectangle,center=true,pad=SfPad.zero,mels=1,fmax=8000,guard=1};
    for pad:0..<2 {
        s.pad=cast(SfPad)pad;r:=sf_spectrogram(a,s,power[..]);
        print("% padding: % frames x % bins, power x 64:",s.pad,r.frames,r.bins);
        for v:power {print(" %",v*64);}print("\n");
    }

    // 4. Log-mel features of a 1 kHz tone, half a second at 16 kHz.
    tone:[8000]f64;for i:0..<8000 {tone[i]=0.5*sin(2*SF_PI*1000*f64(i)/16000);}
    nemo:=sf_nemo_spec();bank:[128*257]f64;features:[128*51]f64;
    b:=sf_mel_bank(nemo,bank[..]);#assert b.status==SfStatus.ok;
    print("mel bank: % filters x % bins; filter 0 weights:",b.mels,b.bins);for k:0..<3 {print(" %",r4(bank[k]*1000));}print(" (x 1000)\n");
    raw:=nemo;raw.normalize=SfNormalize.none;audio:=SfAudio.{rate=16000,channels=1,samples=tone[..]};
    f:=sf_features(audio,raw,bank[..],features[..]);#assert f.status==SfStatus.ok;
    peak:=0;for m:0..<128 {if features[m*f.frames+25]>features[peak*f.frames+25] {peak=m;}}
    lo:=sf_hz_to_mel(0,SfScale.slaney);hi:=sf_hz_to_mel(8000,SfScale.slaney);
    centre:=sf_mel_to_hz(lo+f64(peak+1)*(hi-lo)/129,SfScale.slaney);
    print("tone: % frames; frame 25 peaks in filter % (centre % Hz), log-mel %; filter 0 there: %\n",
        f.frames,peak,r4(centre),r4(features[peak*f.frames+25]),r4(features[25]));
    f=sf_features(audio,nemo,bank[..],features[..]);#assert f.status==SfStatus.ok;
    mean:=0.0;for t:0..<f.frames {mean+=features[peak*f.frames+t];}mean/=f64(f.frames);
    spread:=0.0;for t:0..<f.frames {d:=features[peak*f.frames+t]-mean;spread+=d*d;}
    print("per-feature normalized, filter %: mean %, n - 1 deviation %\n",peak,r4(mean),r4(sqrt(spread/f64(f.frames-1))));

    // 5. Broken inputs, each refused with its reason.
    fast:=SfAudio.{rate=48000,channels=1,samples=tone[..]};print("48 kHz audio for a 16 kHz spec: %\n",sf_features(fast,nemo,bank[..],features[..]).status);
    stereo:=SfAudio.{rate=16000,channels=2,samples=tone[..]};print("two channels: %\n",sf_features(stereo,nemo,bank[..],features[..]).status);
    zero:=0.0;tone[100]=zero/zero;print("a NaN sample: %\n",sf_features(audio,nemo,bank[..],features[..]).status);tone[100]=1.5;
    print("a sample of 1.5: %\n",sf_admit(audio,nemo));
    print("no samples: %\n",sf_admit(SfAudio.{rate=16000,channels=1,samples=tone[0..<0]},nemo));
    wide:=nemo;wide.n_fft=8192;print("an 8192-point frame: %\n",sf_admit(audio,wide));
    many:=nemo;many.n_fft=64;many.win_length=64;many.hop=16;many.mels=64;print("64 mels on 33 bins: %\n",sf_mel_bank(many,bank[..]).status);
    odd:=[3]f64.[1,2,3];one:[2]f64;print("FFT with 3 real and 2 imaginary parts: %\n",sf_fft(odd[..],one[..],false));
}
```

## Refusals

The end of the transcript shows eight broken inputs and their exact
statuses:

- 48 kHz audio for a 16 kHz spec: `rate`.
- Two channels: `channels`.
- A NaN sample: `number`.
- A sample of 1.5: `number`.
- No samples: `empty`.
- An 8,192-point frame: `size`.
- 64 mels on 33 bins: `filter`.
- An FFT given three real parts and two imaginary parts: `shape`.

The example exits with status 0 because each refusal is a returned status,
not a crash. The unit's checks cover every other bound in the table above,
at both sides of each limit, and a refused transform leaves its input
unchanged.

## Checks

- **An independent oracle.**
  [`spectral_oracle.py`](../tests/spectral_oracle.py) writes the formulas out
  in plain Python, without numpy, librosa or torch:
  - the direct DFT with `cmath`;
  - torch's framing;
  - librosa's filter bank construction, including numpy's `linspace` and
    `rfftfreq` arithmetic;
  - both mel scales;
  - NeMo's log guard and normalization.

  The unit's tests compare two complete 4 × 9 filter banks, Slaney with
  Slaney normalization and HTK without normalization, within 10⁻¹⁷ and
  10⁻¹³. They also compare 44 log-mel features of a test waveform (two
  tones and a step, 40 samples, n_fft 16, window 12, hop 4, preemphasis
  0.97, per-feature normalization) within 10⁻¹⁰.
- **Hand-computed transforms.** The 4-point transform above is exact in
  both directions. An 8-point delayed impulse gives the twiddles, and an
  8-point cosine puts 4 in bins 1 and 7. Sizes 3 and 6 go through the chirp
  path.
- **FFT against the DFT.** A `#forall` over every size from 1 to 64, plus
  size 400, compares the FFT with the direct DFT on a fixed pseudo-random
  complex signal, and checks that the inverse returns the input.
- **Properties.** A `#forall` over sizes 1 to 40 checks Parseval's identity,
  Σ|X|²/n = Σ|x|². A `#forall` over sizes 1 to 32 checks linearity: the
  transform of 2x − 3y equals 2X − 3Y.
- **Mutation.** `coax mutate psionic/lib/spectral.coax` leaves no
  surviving mutant. Tests that pin the test helpers' own output keep a
  changed helper from passing unnoticed.

The transcript is compared exactly in debug and release builds.

## What remains

- **Recorded reference features.** The draft's done-means asks for mel
  features of fixed audio to match recorded features of the model pinned in
  chapter 153. No features have been recorded from Parakeet yet, so
  `sf_nemo_spec()` is a statement of the configuration, not a qualified
  match.
- **Dither.** NeMo can add small random noise before framing. It is not
  implemented, and a spec has no field for it.
- **Two-dimensional transforms.** Chapter 156 needs a 2D FFT. It can be
  built from this unit's one-dimensional transform over rows and then
  columns, but that is not written here.

## What later chapters use

| Name | What it does |
| --- | --- |
| `sf_fft(re, im, inverse)` | in-place FFT or inverse, any size 1 to 4,096 |
| `sf_dft(re, im, out_re, out_im, inverse)` | the direct O(n²) reference |
| `sf_turn(r, d)` | e^(−2πi·r/d), exact at quarter turns |
| `sf_window(kind, periodic, out)` | Hann, Hamming or rectangle |
| `SfSpec`, `sf_spec_ok`, `sf_nemo_spec()` | every feature convention, checked |
| `SfAudio`, `sf_admit`, `sf_from_pcm16` | audio admission and 16-bit conversion |
| `sf_frames`, `sf_spectrogram(audio, spec, out)` | frame count and frame-major power spectrogram |
| `sf_hz_to_mel`, `sf_mel_to_hz`, `sf_mel_bank(spec, out)` | mel scales and the filter bank |
| `sf_features(audio, spec, bank, out)` | mel-major log-mel features, normalized |
| `sf_normalize(features, mels, frames)` | per-feature standardization |
