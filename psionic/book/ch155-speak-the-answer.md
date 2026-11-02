# 155. Speak the answer

> **Status: partly implemented.** Text becomes audio on the CPU with fixed
> rules and no trained weights: numbers and symbols are written out as
> words, a letter-to-sound table gives phones with fixed durations, a
> rule-based formant acoustic model gives every frame a magnitude spectrum,
> and Griffin-Lim recovers the phase one block of frames at a time. The
> audio streams out as each block is solved, the stream matches the same
> blocks solved offline bit for bit, and barge-in reports exactly how much
> audio was emitted. Still to do: a learned acoustic model or a pinned
> text-to-speech checkpoint (none is surveyed with its terms yet), speech
> that sounds natural, intelligibility measured with chapter 153's
> transcriber, playback through the voice engine's `verse/platform/voice.h`,
> and wall-clock time to first audio and real-time factor on a named host.

A voice interface has to answer out loud, and it has to start talking
before the whole answer is synthesized. This chapter builds the smallest
text-to-speech path that has every stage a real one has, and checks the
parts that can be checked exactly: the transforms, the phase recovery, the
streaming, and the accounting when someone interrupts.

Everything here is in [`speak.coax`](../lib/speak.coax). It reuses chapter
151's FFT, mel filter bank and log, and chapter 153's edit
distance. The profile is `psionic-speak-v1`: binary64 throughout, FFT sizes
from 8 to 512 with 2 to 8 frames over each sample, blocks of 1 to 32
frames, at most 64 iterations a block, 512 bytes of text, 256 phones and
2,048 frames.

## What this is, and what it is not

The pipeline has four stages:

1. **Text normalization** writes `42` as `forty two` and `%` as `percent`.
2. **Phonemization** turns words into phones, each with a fixed number of
   frames.
3. **The acoustic model** gives each frame a magnitude spectrum.
4. **The vocoder** recovers a phase for those magnitudes and adds the
   frames up into a waveform.

Real systems learn stages 2 to 4 from recorded speech. Here they are rules,
so that every number can be traced by hand. The letter-to-sound table is
wrong for much of English spelling; the acoustic model is a formant
synthesizer drawn in the frequency domain, not a voice; and the recognizer
used to check the output is a template matcher built from the same rules,
not chapter 153's transcriber. The output is buzzy, robotic speech at
8 kHz. What the chapter does establish is the machinery a learned model
would plug into: streaming in blocks, a vocoder whose convergence is
checked, and exact accounting of first audio and of barge-in.

The types:

<!-- source: psionic/lib/speak.coax types -->
```jai
SpkStatus :: enum u8 {ok;shape;empty;symbol;number;length;capacity;closed;cancelled;finished;}
// rate in Hz; n the FFT size; hop the samples between frames; block the
// frames Griffin-Lim solves at a time; iters its iterations per block.
SpkShape :: struct {rate:int;n:int;hop:int;block:int;iters:int;}
// Normalized text: status, bytes written, and the byte a refusal points at.
SpkText :: struct {status:SpkStatus;count:int;at:int;}
// Phones in order, the frame each starts on (start[phones] is the total),
// and the word each belongs to (-1 for silence and pauses).
SpkUtterance :: struct {status:SpkStatus;phones:int;words:int;id:[SPK_PHONES_MAX]int;start:[SPK_PHONES_MAX+1]int;word:[SPK_PHONES_MAX]int;}
// Offsets into the caller's work array: the context frames and the samples
// not yet final keep their values between blocks; the rest is scratch.
SpkPlan :: struct {status:SpkStatus;context:int;span:int;pending:int;at_cre:int;at_cim:int;at_num:int;at_den:int;
    at_re:int;at_im:int;at_mag:int;at_x:int;at_xden:int;at_base:int;at_w:int;at_tr:int;at_ti:int;work:int;}
SpkStream :: struct {open:bool;stopped:bool;shape:SpkShape;plan:SpkPlan;next:int;emitted:int;
    ffts:int;first_ffts:int;first_samples:int;}
// One call's audio: samples written to out, the stream's first sample
// index for them, and whether the utterance is complete.
SpkChunk :: struct {status:SpkStatus;samples:int;at:int;frames:int;ffts:int;final:bool;}
// Barge-in: samples emitted, samples begun but not final, samples never
// started; the phone and word under the last emitted sample (-1 for none
// or silence) and the words heard to their end.
SpkStop :: struct {status:SpkStatus;emitted:int;pending:int;unmade:int;phone:int;word:int;heard:int;}
```

## Text to words

Normalization reads bytes and writes lowercase words separated by single
spaces. Letters are lowercased, apostrophes dropped, runs of up to three
digits written as words, `%`, `&` and `+` written as `percent`, `and` and
`plus`, sentence punctuation written as a pause `|`, and `-` read as a
space. A pause never starts or ends the text and never repeats. Anything
else is refused at its byte: a run of four digits is refused as `number`
at its first digit, and any other symbol, including every non-ASCII byte,
as `symbol`. Dates, ordinals, abbreviations and numbers past 999 are not
handled; they are refused or read letter by letter.

Work the chapter's sentence by hand. `The answer is 42.` lowercases to
`the answer is`, then `42` is 42 < 100, 42 >= 20: `SPK_TENS[4]` is
`forty`, 42 mod 10 is 2, `SPK_UNITS[2]` is `two`. The closing `.` would
write a pause, but a pause does not end the text, so it is dropped:

```text
the answer is forty two
```

<!-- source: psionic/lib/speak.coax normalize -->
```jai
SPK_UNITS :: [20]string.["zero","one","two","three","four","five","six","seven","eight","nine","ten","eleven","twelve",
    "thirteen","fourteen","fifteen","sixteen","seventeen","eighteen","nineteen"];
SPK_TENS :: [10]string.["","","twenty","thirty","forty","fifty","sixty","seventy","eighty","ninety"];
// The words for 0 to 999, each followed by a space.
spk_number :: (v:int,out:[]u8,n:int)->int {
    m:=n;
    if v>=100 {m=spk_put(out,m,SPK_UNITS[v/100]);m=spk_put(out,m,"hundred");if v%100==0 {return m;}}
    r:=v%100;
    if r<20 {return spk_put(out,m,SPK_UNITS[r]);}
    m=spk_put(out,m,SPK_TENS[r/10]);
    if r%10==0 {return m;}
    return spk_put(out,m,SPK_UNITS[r%10]);
}
// Append a word and a space; -1 when out is full or already failed.
spk_put :: (out:[]u8,n:int,s:string)->int {
    if n<0 || n+s.count+1>out.count {return -1;}
    for i:0..<s.count {out[n+i]=s[i];}
    out[n+s.count]=' ';return n+s.count+1;
}
// Letters are lowercased, apostrophes dropped, digit runs of up to three
// written as words, % & + as words, , . ! ? ; : as a pause "|" and - as a
// space. Words end with one space; pauses never repeat or start or end the
// text. Anything else is refused at its byte.
spk_normalize :: (text:string,out:[]u8)->SpkText {
    if text.count>SPK_TEXT_MAX {return SpkText.{status=SpkStatus.length,at=SPK_TEXT_MAX};}
    n:=0;i:=0;word:=false;
    // The end of the text reads as a space: it ends the last word.
    while i<=text.count {
        c:u8=' ';if i<text.count {c=text[i];}
        // Setting bit 5 lowercases a letter (A is 65, a is 97).
        if (c|32)>='a' && (c|32)<='z' {
            if n==out.count {return SpkText.{status=SpkStatus.capacity,at=i};}
            out[n]=c|32;n+=1;word=true;i+=1;continue;
        }
        if c==39 {i+=1;continue;}
        if word {n=spk_put(out,n,"");word=false;}
        start:=i;
        if c>='0' && c<='9' {
            v:=0;while i<text.count && text[i]>='0' && text[i]<='9' {v=v*10+cast(int)(text[i]-'0');i+=1;if i-start>3 {return SpkText.{status=SpkStatus.number,at=start};}}
            n=spk_number(v,out,n);
        } else {
            i+=1;
            if c=='%' {n=spk_put(out,n,"percent");}
            else if c=='&' {n=spk_put(out,n,"and");}
            else if c=='+' {n=spk_put(out,n,"plus");}
            else if c==',' || c=='.' || c=='!' || c=='?' || c==';' || c==':' {if n>=2 && out[n-2]!='|' {n=spk_put(out,n,"|");}}
            else if c!=' ' && c!='-' {return SpkText.{status=SpkStatus.symbol,at=start};}
        }
        if n<0 {return SpkText.{status=SpkStatus.capacity,at=start};}
    }
    if n==0 {return SpkText.{status=SpkStatus.empty};}
    // Drop a closing pause and the last space.
    if out[n-2]=='|' {n-=2;}
    return SpkText.{count=n-1};
}
```

## Words to phones

There are 26 phones: silence and a pause, five vowels, six sonorants
(m n l r w y), five fricatives (f s sh th h), two voiced fricatives (v z),
three stops (p t k) and three voiced stops (b d g). Each has a fixed
duration in frames:

<!-- source: psionic/lib/speak.coax phones -->
```jai
// Phones 0 and 1 are silence (6 frames) and a pause (20); 2-6 vowels, 7-12
// sonorants, 13-17 fricatives, 18-19 voiced fricatives, 20-22 stops,
// 23-25 voiced stops. Formants in Hz, voicing and noise gains, the noise
// band in Hz, and the frames each lasts.
SPK_NAMES :: [26]string.["_","|","a","e","i","o","u","m","n","l","r","w","y","f","s","sh","th","h","v","z","p","t","k","b","d","g"];
SPK_F1 :: [26]f64.[0,0,730,530,270,570,300,280,280,360,420,300,280,0,0,0,0,0,300,300,0,0,0,300,300,300];
SPK_F2 :: [26]f64.[0,0,1090,1840,2290,840,870,1300,1700,1300,1300,610,2250,0,0,0,0,0,1400,1700,0,0,0,1000,1700,2000];
SPK_F3 :: [26]f64.[0,0,2440,2480,3010,2410,2240,2500,2600,2700,1600,2200,2900,0,0,0,0,0,2500,2600,0,0,0,2400,2600,2500];
SPK_VOICE :: [26]f64.[0,0,1,1,1,1,1,0.5,0.5,0.5,0.5,0.5,0.5,0,0,0,0,0,0.3,0.3,0,0,0,0.2,0.2,0.2];
SPK_NOISE :: [26]f64.[0,0,0,0,0,0,0,0,0,0,0,0,0,0.08,0.25,0.25,0.06,0.08,0.06,0.15,0.3,0.35,0.3,0.2,0.25,0.25];
SPK_LO :: [26]f64.[0,0,0,0,0,0,0,0,0,0,0,0,0,1000,3000,1800,1500,500,1000,3000,500,2500,1500,500,2500,1500];
SPK_HI :: [26]f64.[0,0,0,0,0,0,0,0,0,0,0,0,0,4000,4000,3500,4000,3500,4000,4000,2500,4000,3000,2000,4000,3000];
SPK_FRAMES :: [26]int.[6,20,14,14,14,14,14,8,8,8,8,8,8,10,10,10,10,10,8,8,7,7,7,6,6,6];
```

The letter-to-sound rules try a digraph first (`sh`, `ch`, `th`, `ph`,
`ng`, `ck`, `qu`, `ee`, `ea`, `oo`, `ai`, `ay`), then one letter. An
utterance starts and ends with silence:

<!-- source: psionic/lib/speak.coax letters -->
```jai
spk_vowel_letter :: (c:u8)->bool {return c=='a' || c=='e' || c=='i' || c=='o' || c=='u';}
// The phone index of a name in SPK_NAMES, or -1.
spk_phone :: (name:string)->int {for p:0..<26 {if SPK_NAMES[p]==name {return p;}}return -1;}
// Letter-to-sound for one word, digraphs before single letters. c is s
// before e, i or y and k elsewhere; x is k s; y is a vowel after the first
// letter; a final e after a consonant is silent in words of four letters or
// more. Wrong for much of English spelling, and meant only to be fixed and
// checkable.
spk_word :: (w:string,u:[]SpkUtterance,index:int)->SpkStatus {
    i:=0;n:=w.count;
    while i<n {
        c:=w[i];two:="";if i+1<n {two=w[i..<i+2];}
        a:=w[i..<i+1];b:="";step:=2;
        if two=="sh" || two=="ch" {a="sh";}
        else if two=="th" {a="th";}
        else if two=="ph" {a="f";}
        else if two=="ng" {}   // n: a is already the first letter
        else if two=="ck" {a="k";}
        else if two=="qu" {a="k";b="w";}
        else if two=="ee" || two=="ea" {a="i";}
        else if two=="oo" {a="u";}
        else if two=="ai" || two=="ay" {a="e";}
        else {
            step=1;
            if c=='e' && i==n-1 && n>=4 && !spk_vowel_letter(w[i-1]) {break;}
            if c=='c' {a="k";if two=="ce" || two=="ci" || two=="cy" {a="s";}}
            if c=='x' {a="k";b="s";}
            if c=='q' {a="k";}
            if c=='j' {a="z";}
            if c=='y' && i>0 {a="i";}
        }
        i+=step;
        st:=spk_add(u,spk_phone(a),index);
        if st==SpkStatus.ok && b.count>0 {st=spk_add(u,spk_phone(b),index);}
        if st!=SpkStatus.ok {return st;}
    }
    return SpkStatus.ok;
}
// Append phone p of word w. A consonant does not repeat inside a word (ll,
// ss); a phone past 256 or a frame past 2,048 is refused. The opening
// silence is always phone 0.
spk_add :: (u:[]SpkUtterance,p:int,w:int)->SpkStatus {
    k:=u[0].phones;
    if p>=7 && u[0].id[k-1]==p && u[0].word[k-1]==w {return SpkStatus.ok;}
    if k==SPK_PHONES_MAX {return SpkStatus.length;}
    t:=u[0].start[k]+SPK_FRAMES[p];
    if t>SPK_FRAMES_MAX {return SpkStatus.length;}
    u[0].id[k]=p;u[0].word[k]=w;u[0].start[k+1]=t;u[0].phones=k+1;
    return SpkStatus.ok;
}
// Normalized text to phones: silence, each word's phones, a pause for each
// "|", silence. Words must be lowercase letters.
spk_phonemize :: (text:string,u:[]SpkUtterance)->SpkStatus {
    u[0]=SpkUtterance.{};
    st:=spk_add(u,0,-1);i:=0;
    while i<text.count && st==SpkStatus.ok {
        start:=i;while i<text.count && text[i]!=' ' {i+=1;}
        w:=text[start..<i];i+=1;
        if w=="|" {st=spk_add(u,1,-1);continue;}
        for k:0..<w.count {if !(w[k]>='a' && w[k]<='z') {st=SpkStatus.symbol;}}
        if st==SpkStatus.ok {st=spk_word(w,u,u[0].words);u[0].words+=1;}
    }
    if st==SpkStatus.ok {st=spk_add(u,0,-1);}
    if st==SpkStatus.ok && u[0].words==0 {st=SpkStatus.empty;}
    u[0].status=st;
    return st;
}
spk_frames :: (u:[]SpkUtterance)->int {return u[0].start[u[0].phones];}
```

By hand, `the answer is forty two`: `th` is a digraph, then `e`; `answer`
is `a n s w e r`; `is` is `i s`; `forty` is `f o r t` and `y`, which is a
vowel `i` after the first letter; `two` is `t w o`. With silence at both
ends that is 20 phones. Their frames, from the table: 6 + (10 + 14) +
(14 + 8 + 10 + 8 + 14 + 8) + (14 + 10) + (10 + 14 + 8 + 7 + 14) + (7 + 8 +
14) + 6 = 204 frames. The second `e` of `answer` starts at frame 6 + 24 +
14 + 8 + 10 + 8 = 70 and ends before frame 84.

## Phones to spectra

The acoustic model draws each frame's magnitude spectrum from its phone's
row of the table. Voicing is a harmonic comb at the pitch, shaped by three
formant resonances; friction is a band of flat noise that falls off over
300 Hz outside its edges. A stop is silent (or, if voiced, only voiced)
until its last two frames, the burst. The pitch falls from 140 Hz to 100 Hz
over the utterance, and the first two frames of each phone blend in 2/3 and
then 1/3 of the phone before it:

<!-- source: psionic/lib/speak.coax acoustic -->
```jai
spk_peak :: (f:f64,center:f64,width:f64)->f64 {d:=(f-center)/width;return 1/(1+d*d);}
// Three formant resonances: widths 90, 110 and 170 Hz, gains 1, 1/2, 1/4.
spk_envelope :: (p:int,f:f64)->f64 {
    return spk_peak(f,SPK_F1[p],90)+0.5*spk_peak(f,SPK_F2[p],110)+0.25*spk_peak(f,SPK_F3[p],170);
}
// Harmonics of the pitch: 1 at each multiple of f0, 0 halfway between.
spk_comb :: (x:f64)->f64 {c:=(1+cos(2*SF_PI*x))/2;c*=c;return c*c;}
// 1 inside the noise band, falling off over 300 Hz outside it: a peak
// centred on f clamped into [lo, hi], max and min written with abs.
spk_band :: (f:f64,lo:f64,hi:f64)->f64 {
    up:=(f+lo+abs(f-lo))/2;
    return spk_peak(f,(up+hi-abs(up-hi))/2,300);
}
// Phone p at frequency f, pitch f0: voicing shaped by the formants on the
// comb, plus the noise band unless the phone is a stop still closed.
spk_level :: (p:int,closed:bool,f0:f64,f:f64)->f64 {
    v:=SPK_VOICE[p]*spk_envelope(p,f)*spk_comb(f/f0);
    if closed {return v;}
    return v+SPK_NOISE[p]*spk_band(f,SPK_LO[p],SPK_HI[p]);
}
// The phone that frame t falls in.
spk_phone_at :: (u:[]SpkUtterance,t:int)->int {k:=0;while u[0].start[k+1]<=t {k+=1;}return k;}
// Frame t's magnitudes, bins 0 to n/2 (bin b is b rate/n Hz). The pitch
// falls from 140 to 100 Hz over the utterance. A stop is closed (only its
// voicing) until its last two frames, the burst. Frame j of a phone is
// (j + 1)/3 of itself, at most all of it, and the rest the phone before as
// it ended: 2/3, then 1/3, then nothing. A level of 1 is a sinusoid of
// amplitude SPK_GAIN: n/4 under the window.
spk_frame :: (sh:SpkShape,u:[]SpkUtterance,t:int,mag:[]f64) {
    k:=spk_phone_at(u,t);p:=u[0].id[k];j:=t-u[0].start[k];
    f0:=140-40*f64(t)/f64(spk_frames(u));scale:=SPK_GAIN*f64(sh.n)/4;
    closed:=p>=20 && j<SPK_FRAMES[p]-2;a:=f64(tdt_min(j+1,3))/3;
    for b:0..<sh.n/2+1 {
        f:=f64(b*sh.rate)/f64(sh.n);v:=spk_level(p,closed,f0,f);
        if k>0 {v=a*v+(1-a)*spk_level(u[0].id[k-1],false,f0,f);}
        mag[b]=scale*v;
    }
}
```

A hand value: `a` at 840 Hz with the pitch at 140 Hz. 840 is the sixth
harmonic, so the comb is 1. The resonances give 1/(1 + (110/90)^2) =
0.4010, 0.5/(1 + (250/110)^2) = 0.0811 and 0.25/(1 + (1600/170)^2) =
0.0028, so the level is 0.4849. The checks compare `spk_level` with this sum
written out.

## Frames to samples, and back

The vocoder works on a short-time Fourier transform: frame t holds the n
samples from t x hop, multiplied by a window w. The window is Hann's,
0.5 - 0.5 cos(2 pi (m + 1/2)/n), sampled half a sample off the usual grid
so that no sample of it is 0: then every sample of the signal, the first
included, is reached by some frame with a nonzero weight. The inverse used
here is the least-squares one: each sample is the sum, over the frames that
cover it, of w times that frame's inverse transform, divided by the sum of
w^2 over the same frames.

By hand with n = 8 and hop = 2: w is 0.038, 0.309, 0.691, 0.962, 0.962,
0.691, 0.309, 0.038. An interior sample is covered by four frames, at window
positions m, m + 2, m + 4 and m + 6. For even m the squares are 0.0015 +
0.4775 + 0.9253 + 0.0955; for odd m, 0.0955 + 0.9253 + 0.4775 + 0.0015.
Both add to 1.5 (the cosine terms cancel over four quarter-turn shifts). So
in the interior the inverse divides by a constant 3/2, and the STFT of a
signal followed by this inverse gives the signal back exactly. The checks do
that for every FFT size from 8 to 64 and 2, 4 and 8 frames over a sample,
with the forward transform from chapter 151's direct DFT: every sample comes
back within 1e-12.

<!-- source: psionic/lib/speak.coax stft -->
```jai
// FFT sizes are powers of two from 8 to 512; hop divides n with 2 to 8
// frames over each sample; the context is the n/hop - 1 frames before a
// block that still overlap it.
spk_shape_ok :: (sh:SpkShape)->SpkStatus {
    if sh.rate<SF_RATE_MIN || sh.rate>SF_RATE_MAX || sh.hop<1 {return SpkStatus.shape;}
    // n is 8 doubled until it reaches n or 512; n/hop is 2 doubled until it
    // reaches n/hop or 8.
    m:=8;while m<sh.n && m<SPK_N_MAX {m*=2;}
    r:=2;while r<sh.n/sh.hop && r<8 {r*=2;}
    if m!=sh.n || r*sh.hop!=sh.n {return SpkStatus.shape;}
    if sh.block<1 || sh.block>SPK_BLOCK_MAX || sh.iters<0 || sh.iters>SPK_ITERS_MAX {return SpkStatus.shape;}
    return SpkStatus.ok;
}
spk_plan :: (sh:SpkShape)->SpkPlan {
    p:SpkPlan;p.status=spk_shape_ok(sh);if p.status!=SpkStatus.ok {return p;}
    n:=sh.n;c:=n/sh.hop-1;f:=c+sh.block;p.context=c;p.span=(f-1)*sh.hop+n;p.pending=sh.block*sh.hop+n;
    p.at_cim=c*n;p.at_num=2*c*n;p.at_den=p.at_num+p.pending;p.at_re=p.at_den+p.pending;p.at_im=p.at_re+f*n;
    p.at_mag=p.at_im+f*n;p.at_x=p.at_mag+sh.block*(n/2+1);p.at_xden=p.at_x+p.span;p.at_base=p.at_xden+p.span;
    p.at_w=p.at_base+p.span;p.at_tr=p.at_w+n;p.at_ti=p.at_tr+n;p.work=p.at_ti+n;
    return p;
}
spk_part :: (a:[]f64,at:int,n:int)->[]f64 {return a[at..<at+n];}
// A Hann window sampled half a sample off the grid: w[m] = 0.5 - 0.5
// cos(2 pi (m + 1/2)/n). No sample is 0, so every sample of a span is
// reached; with 3 or more frames over a sample the squares add to 3n/(8 hop).
spk_window :: (w:[]f64) {n:=w.count;for m:0..<n {w[m]=0.5-0.5*sf_turn(2*m+1,2*n).re;}}
// Add the windowed inverse transform of spectrum (re, im) to num at at.
spk_synthesize :: (re:[]f64,im:[]f64,w:[]f64,tr:[]f64,ti:[]f64,num:[]f64,at:int) {
    n:=w.count;for m:0..<n {tr[m]=re[m];ti[m]=im[m];}
    spk_inverse(tr,ti);
    for m:0..<n {num[at+m]+=w[m]*tr[m];}
}
spk_inverse :: (re:[]f64,im:[]f64) {#assert sf_fft(re,im,true)==SfStatus.ok;}
spk_window_add :: (w:[]f64,den:[]f64,at:int) {for m:0..<w.count {den[at+m]+=w[m]*w[m];}}
// The transform of the windowed n samples of x from at.
spk_analyze :: (x:[]f64,at:int,w:[]f64,tr:[]f64,ti:[]f64) {
    for m:0..<w.count {tr[m]=x[at+m]*w[m];ti[m]=0;}
    #assert sf_fft(tr,ti,false)==SfStatus.ok;
}
// The least-squares signal: num / den.
spk_divide :: (num:[]f64,den:[]f64,x:[]f64) {for s:0..<x.count {x[s]=num[s]/den[s];}}
```

## Griffin-Lim, one block at a time

The acoustic model gives magnitudes only. Griffin-Lim finds a phase by
alternating two projections: make a signal from the frames (the
least-squares inverse), transform it again, and keep the new phase with the
target magnitude (at a bin where the new transform is exactly 0 the old
estimate stays). Griffin and Lim showed that the distance between the
frames and the transform of their signal never rises from one iteration to
the next. It can stall in a poor phase, and it often does: the checks find
it non-increasing on every fixture, and falling, but not to zero.

A stream cannot wait for the whole utterance, so it solves one block of
frames at a time. The n/hop - 1 frames before a block still overlap it;
they are already final, so they go into the block as fixed context. That
keeps the block's edges consistent with what has already been played, and
the argument still holds, because holding frames fixed is one more
projection. The first estimate turns each bin's phase by 2 pi k hop/n from
the frame before, as a sinusoid at bin k turns in one hop:

<!-- source: psionic/lib/speak.coax solve -->
```jai
spk_fold :: (k:int,n:int)->int {return tdt_min(k,n-k);}
// First estimate of block frame f from target magnitudes mag: phase 0 for
// the block's first frame; otherwise frame f - 1's phase (0 where it has no
// energy) turned by 2 pi k hop/n at bin k, as a sinusoid at bin k turns in
// one hop.
spk_guess :: (sh:SpkShape,re:[]f64,im:[]f64,f:int,mag:[]f64) {
    n:=sh.n;
    for k:0..<n {
        m:=mag[spk_fold(k,n)];re[f*n+k]=m;im[f*n+k]=0;
        if f==0 {continue;}
        ur:=1.0;ui:=0.0;pr:=re[(f-1)*n+k];pi:=im[(f-1)*n+k];a:=sqrt(pr*pr+pi*pi);if a>0 {ur=pr/a;ui=pi/a;}
        t:=sf_turn((k*(n-sh.hop))%n,n);
        re[f*n+k]=m*(ur*t.re-ui*t.im);im[f*n+k]=m*(ur*t.im+ui*t.re);
    }
}
// Griffin-Lim on one block. Frames 0 .. c - 1 of the block are fixed
// context; frames c .. c + b - 1 hold first estimates and have target
// magnitudes mag (b rows of n/2 + 1). Each iteration makes the
// least-squares signal of all c + b frames, transforms it again, and gives
// each free frame its target magnitude with the new phase (keeping the old
// one at a bin where the new transform is exactly 0). curve[i] is the
// distance from iteration i's frames to the transform of their signal, over
// the frames' norm; Griffin and Lim's argument makes it non-increasing.
// Returns the transforms done.
spk_solve :: (sh:SpkShape,p:SpkPlan,c:int,b:int,work:[]f64,curve:[]f64)->int {
    n:=sh.n;h:=sh.hop;span:=(c+b-1)*h+n;bins:=n/2+1;
    re:=spk_part(work,p.at_re,(c+b)*n);im:=spk_part(work,p.at_im,(c+b)*n);mag:=spk_part(work,p.at_mag,b*bins);
    x:=spk_part(work,p.at_x,span);den:=spk_part(work,p.at_xden,span);base:=spk_part(work,p.at_base,span);
    w:=spk_part(work,p.at_w,n);tr:=spk_part(work,p.at_tr,n);ti:=spk_part(work,p.at_ti,n);
    if sh.iters==0 {return 0;}
    norm:=0.0;
    for s:0..<span {base[s]=0;den[s]=0;}
    for f:0..<c+b {spk_window_add(w,den,f*h);}
    for f:0..<c {
        spk_synthesize(spk_part(re,f*n,n),spk_part(im,f*n,n),w,tr,ti,base,f*h);
        for k:0..<n {norm+=re[f*n+k]*re[f*n+k]+im[f*n+k]*im[f*n+k];}
    }
    for f:0..<b {for k:0..<n {m:=mag[f*bins+spk_fold(k,n)];norm+=m*m;}}
    for i:0..<sh.iters {
        for s:0..<span {x[s]=base[s];}
        for f:c..<c+b {spk_synthesize(spk_part(re,f*n,n),spk_part(im,f*n,n),w,tr,ti,x,f*h);}
        spk_divide(x,den,x);
        d:=0.0;
        for f:0..<c+b {
            spk_analyze(x,f*h,w,tr,ti);
            for k:0..<n {
                at:=f*n+k;
                if f<c {dr:=re[at]-tr[k];di:=im[at]-ti[k];d+=dr*dr+di*di;continue;}
                m:=mag[(f-c)*bins+spk_fold(k,n)];a:=sqrt(tr[k]*tr[k]+ti[k]*ti[k]);d+=(a-m)*(a-m);
                if a>0 {re[at]=m*(tr[k]/a);im[at]=m*(ti[k]/a);}
            }
        }
        curve[i]=0;if norm>0 {curve[i]=sqrt(d/norm);}
    }
    return c+sh.iters*(2*b+c);
}
```

Each iteration costs b inverse transforms for the free frames and c + b
forward ones (the context frames are transformed back only to measure the
distance). The context's inverse transforms are done once per block, and
adding the solved frames to the output costs b more. So a block of b frames
with c frames of context and I iterations costs c + I(2b + c) + b
transforms, or b + 2Ib with no context.

## The stream

`spk_next` solves the next block and adds its frames to the samples that
are not yet final. Once frames up to t have been added, no later frame
reaches below sample (t + 1) x hop, so every sample before that is final
and is written out. At the end of the utterance the rest is final too:

<!-- source: psionic/lib/speak.coax stream -->
```jai
spk_open :: (sh:SpkShape,u:[]SpkUtterance,work:[]f64,st:[]SpkStream)->SpkStatus {
    p:=spk_plan(sh);if p.status!=SpkStatus.ok {return p.status;}
    if u[0].status!=SpkStatus.ok || u[0].phones==0 {return SpkStatus.empty;}
    if work.count<p.work {return SpkStatus.capacity;}
    st[0]=SpkStream.{open=true,shape=sh,plan=p};
    for i:0..<p.pending {work[p.at_num+i]=0;work[p.at_den+i]=0;}
    spk_window(spk_part(work,p.at_w,sh.n));
    return SpkStatus.ok;
}
// The next block of the utterance spk_open was given (the caller keeps it
// unchanged): its frames from the acoustic model, Griffin-Lim with the
// kept context, its frames added to the samples not yet final, and every
// sample no later frame reaches written to out. Refused with nothing
// changed when out or curve is too small.
spk_next :: (st:[]SpkStream,u:[]SpkUtterance,work:[]f64,out:[]f64,curve:[]f64)->SpkChunk {
    if !st[0].open {return SpkChunk.{status=SpkStatus.closed};}
    if st[0].stopped {return SpkChunk.{status=SpkStatus.cancelled};}
    sh:=st[0].shape;p:=st[0].plan;n:=sh.n;h:=sh.hop;bins:=n/2+1;
    T:=spk_frames(u);next:=st[0].next;
    if next==T {return SpkChunk.{status=SpkStatus.finished};}
    b:=tdt_min(sh.block,T-next);end:=next+b==T;
    count:=b*h;if end {count=(b-1)*h+n;}
    if out.count<count || curve.count<sh.iters {return SpkChunk.{status=SpkStatus.capacity};}
    c:=tdt_min(p.context,next);
    for i:0..<c*n {work[p.at_re+i]=work[i];work[p.at_im+i]=work[p.at_cim+i];}
    re:=spk_part(work,p.at_re,(c+b)*n);im:=spk_part(work,p.at_im,(c+b)*n);
    for j:0..<b {
        mag:=spk_part(work,p.at_mag+j*bins,bins);
        spk_frame(sh,u,next+j,mag);spk_guess(sh,re,im,c+j,mag);
    }
    ffts:=spk_solve(sh,p,c,b,work,curve)+b;
    num:=spk_part(work,p.at_num,p.pending);den:=spk_part(work,p.at_den,p.pending);
    w:=spk_part(work,p.at_w,n);tr:=spk_part(work,p.at_tr,n);ti:=spk_part(work,p.at_ti,n);
    for j:0..<b {
        spk_synthesize(spk_part(re,(c+j)*n,n),spk_part(im,(c+j)*n,n),w,tr,ti,num,j*h);spk_window_add(w,den,j*h);
    }
    spk_divide(num,den,out[0..<count]);
    for i:0..<p.pending {
        num[i]=0;den[i]=0;
        if i+count<p.pending {num[i]=num[i+count];den[i]=den[i+count];}
    }
    // Keep the last frames as the next block's context.
    k:=tdt_min(p.context,c+b);
    for i:0..<k*n {work[i]=re[(c+b-k)*n+i];work[p.at_cim+i]=im[(c+b-k)*n+i];}
    r:=SpkChunk.{samples=count,at=st[0].emitted,frames=b,ffts=ffts,final=end};
    st[0].next+=b;st[0].emitted+=count;st[0].ffts+=ffts;
    if st[0].first_samples==0 {st[0].first_ffts=st[0].ffts;st[0].first_samples=count;}
    return r;
}
```

By hand for the chapter's sentence, with n = 256 (32 ms at 8 kHz), hop 64
(8 ms), blocks of 8 frames and 24 iterations: the context is 256/64 - 1 = 3
frames. Block 0 has no context: 8 + 2 x 24 x 8 = 392 transforms, and then
8 x 64 = 512 samples (64 ms) are final. That is the first audio. Every
later full block costs 3 + 24 x (16 + 3) + 8 = 467 transforms. 204 frames
make 25 blocks of 8 and one of 4; the last costs 3 + 24 x (8 + 3) + 4 = 271
transforms and releases (4 - 1) x 64 + 256 = 448 samples. In all: 392 +
24 x 467 + 271 = 11,871 transforms and (204 - 1) x 64 + 256 = 13,248
samples, 1.656 s.

The first audio's cost is counted in transforms, not measured in time. A
block's compute must take less than its own audio (64 ms here) for the
stream to keep up; this chapter does not yet measure that on a named host.

## Barge-in

When the person starts talking over the answer, the voice engine
(`verse/voice.coax`) cuts playback and opens a new media epoch. The stream stops
too, and says exactly where:

<!-- source: psionic/lib/speak.coax cancel -->
```jai
// Barge-in: stop now. Of the utterance's (T - 1) hop + n samples, emitted
// were handed out; pending have some frames added but are not final and
// are dropped; unmade were never started. The phone and word under the
// last emitted sample (the last frame added: emitted samples end at its
// hop, or at the end), and the words heard to their end: all words but
// those with a phone whose last hop is not emitted. Phone 0 is silence.
spk_cancel :: (st:[]SpkStream,u:[]SpkUtterance)->SpkStop {
    if !st[0].open {return SpkStop.{status=SpkStatus.closed};}
    if st[0].stopped {return SpkStop.{status=SpkStatus.cancelled};}
    st[0].stopped=true;
    h:=st[0].shape.hop;T:=spk_frames(u);E:=st[0].emitted;next:=st[0].next;
    total:=(T-1)*h+st[0].shape.n;pending:=0;
    if next>0 {pending=(next-1)*h+st[0].shape.n-E;}
    r:=SpkStop.{emitted=E,pending=pending,unmade=total-E-pending,phone=-1,word=-1};
    if next>0 {k:=spk_phone_at(u,next-1);r.phone=u[0].id[k];r.word=u[0].word[k];}
    r.heard=u[0].words;
    for k:1..<u[0].phones {w:=u[0].word[k];if w>=0 && u[0].start[k+1]*h>E {r.heard=tdt_min(r.heard,w);}}
    return r;
}
```

By hand, stopping after 10 blocks: 80 frames are added and 80 x 64 = 5,120
samples (640 ms) emitted. The added frames reach (80 - 1) x 64 + 256 =
5,312 samples, so 192 samples are pending and dropped, and 13,248 - 5,312
= 7,936 were never made. The last emitted sample, 5,119, lies in frame
5,119 / 64 = 79, inside the second `e` of `answer` (frames 70 to 83), word
1. Word 0, `the`, ends with frame 29, and 30 x 64 = 1,920 samples are
emitted, so one word was heard to its end. A replay would restart from word 1.

## Checking the output

There is no recorded reference audio for these rules, so the checks are
about the machinery. A template recognizer checks that the spectra survive
the vocoder: each phone's template is chapter 151's log-mel energies of
its spectrum, and each frame of output audio takes the nearest template.
Runs of one label at least two frames long, with silence dropped, are the
phones heard; chapter 153's edit distance counts the errors against the
phones meant:

<!-- source: psionic/lib/speak.coax hear -->
```jai
// A template recognizer, to check that the spectra survive the vocoder.
// Features are chapter 151's log-mel energies: n/8 Slaney mels (at most
// 24) from 0 Hz to half the rate, on the power of a frame, log(v + 1e-4).
// A phone's template is its spectrum at 120 Hz (a stop's burst); each frame of audio takes the nearest template, silence
// standing for the pause too. It knows nothing the acoustic model does
// not; it is not a speech recognizer.
spk_mel_spec :: (sh:SpkShape)->SfSpec {
    return SfSpec.{rate=sh.rate,n_fft=sh.n,win_length=sh.n,hop=sh.hop,window=SfWindow.hann,periodic=true,center=false,pad=SfPad.zero,
        preemphasis=0,mels=tdt_min(24,sh.n/8),fmin=0,fmax=f64(sh.rate)/2,scale=SfScale.slaney,norm=SfNorm.none,log=SfLog.add,
        guard=0.0001,normalize=SfNormalize.none};
}
// Log-mel features of one frame's power spectrum (bins 0 to n/2). Every
// filter is 0 at 0 Hz, so bin 0 is skipped.
spk_mel :: (s:SfSpec,bank:[]f64,power:[]f64,out:[]f64) {
    bins:=power.count;
    for i:0..<s.mels {v:=0.0;for k:1..<bins {v+=bank[i*bins+k]*power[k];}out[i]=sf_log(v,s.log,s.guard);}
}
// The bank and the 26 templates; the mels, or -1 if the bank is refused.
spk_templates :: (sh:SpkShape,bank:[]f64,tmpl:[]f64)->int {
    s:=spk_mel_spec(sh);if sf_mel_bank(s,bank).status!=SfStatus.ok {return -1;}
    bins:=sh.n/2+1;scale:=SPK_GAIN*f64(sh.n)/4;power:[SPK_N_MAX/2+1]f64;
    for p:0..<26 {
        for b:1..<bins {v:=scale*spk_level(p,false,120,f64(b*sh.rate)/f64(sh.n));power[b]=v*v;}
        spk_mel(s,bank,power[0..<bins],tmpl[p*s.mels..<(p+1)*s.mels]);
    }
    return s.mels;
}
// Frame t's log-mel features (the mels of spk_mel_spec).
spk_features :: (sh:SpkShape,audio:[]f64,t:int,bank:[]f64,out:[]f64) {
    n:=sh.n;bins:=n/2+1;w:[SPK_N_MAX]f64;tr:[SPK_N_MAX]f64;ti:[SPK_N_MAX]f64;power:[SPK_N_MAX/2+1]f64;
    spk_window(w[0..<n]);spk_analyze(audio,t*sh.hop,w[0..<n],tr[0..<n],ti[0..<n]);
    for k:1..<bins {power[k]=tr[k]*tr[k]+ti[k]*ti[k];}
    spk_mel(spk_mel_spec(sh),bank,power[0..<bins],out);
}
// Label frames 0 .. frames - 1 of audio (at least (frames - 1) hop + n
// samples) with the nearest template, skipping the pause's; ties go to the
// lower phone.
spk_label :: (sh:SpkShape,audio:[]f64,frames:int,bank:[]f64,tmpl:[]f64,labels:[]int) {
    m:=spk_mel_spec(sh).mels;f:[24]f64;
    for t:0..<frames {
        spk_features(sh,audio,t,bank,f[0..<m]);
        best:=-1;bd:=0.0;
        for p:0..<26 {
            if p==1 {continue;}
            d:=spk_distance(f[0..<m],tmpl,p);
            if best<0 || d<bd {best=p;bd=d;}
        }
        labels[t]=best;
    }
}
spk_distance :: (f:[]f64,tmpl:[]f64,p:int)->f64 {d:=0.0;for i:0..<f.count {e:=f[i]-tmpl[p*f.count+i];d+=e*e;}return d;}
// Phones as heard: runs of one label at least two frames long, silence
// dropped, repeats merged. Phones as meant: the utterance's, the same way.
// Each written to out as indices; the counts.
spk_runs :: (labels:[]int,out:[]int)->int {
    m:=0;t:=0;
    while t<labels.count {
        s:=t;while t<labels.count && labels[t]==labels[s] {t+=1;}
        p:=labels[s];
        if t-s>=2 && p>=2 && (m==0 || out[m-1]!=p) {out[m]=p;m+=1;}
    }
    return m;
}
spk_meant :: (u:[]SpkUtterance,out:[]int)->int {
    m:=0;
    for k:1..<u[0].phones {p:=u[0].id[k];if p>=2 && (m==0 || out[m-1]!=p) {out[m]=p;m+=1;}}
    return m;
}
// Phone error rate's numerator: chapter 153's edit distance over the names.
spk_phone_errors :: (meant:[]int,heard:[]int)->int {
    a:[SPK_PHONES_MAX]string;b:[SPK_FRAMES_MAX]string;row:[SPK_FRAMES_MAX+1]int;
    for i:0..<meant.count {a[i]=SPK_NAMES[meant[i]];}
    for i:0..<heard.count {b[i]=SPK_NAMES[heard[i]];}
    return tdt_distance(a[0..<meant.count],b[0..<heard.count],row[..]);
}
```

This is not intelligibility. The recognizer knows exactly the spectra the
model draws, so it measures how much Griffin-Lim's phase errors smear them,
nothing about whether a person would understand the words.

## Run it

The example, [`speak.coax`](../examples/speak.coax), speaks the sentence:

<!-- source: psionic/examples/speak.coax text -->
```jai
    out:[256]u8;r:=spk_normalize(TEXT,out[..]);#assert r.status==SpkStatus.ok;
    text:=cast(string)out[0..<r.count];
    print("text: %\nnormalized: %\n",TEXT,text);
    u:[1]SpkUtterance;#assert spk_phonemize(text,u[..])==SpkStatus.ok;
    T:=spk_frames(u[..]);
    print("% phones in % words:",u[0].phones,u[0].words);show(u[..]);
```

<!-- source: psionic/examples/speak.coax stream -->
```jai
    // 8 kHz, 256-point frames every 64 samples (8 ms), blocks of 8 frames
    // (64 ms), 24 Griffin-Lim iterations a block.
    sh:=SpkShape.{rate=8000,n=256,hop=64,block=8,iters=24};p:=spk_plan(sh);L:=(T-1)*64+256;
    print("% frames, % samples (% ms); context % frames, work % values\n",T,L,L/8,p.context,p.work);
    work:[40000]f64;st:[1]SpkStream;audio:[40000]f64;curve:[24]f64;
    #assert spk_open(sh,u[..],work[..],st[..])==SpkStatus.ok;
    at:=0;chunks:=0;
    while true {
        c:=spk_next(st[..],u[..],work[..],audio[at..<40000],curve[..]);
        if c.status!=SpkStatus.ok {print("then: %\n",c.status);break;}
        if chunks==0 {print("first audio: % samples (% ms) after % frames and % transforms\n",c.samples,c.samples/8,c.frames,c.ffts);}
        if chunks==4 {print("block 4 (frames 32 to 39): distance % after 1 iteration, % after 24\n",curve[0],curve[23]);}
        at+=c.samples;chunks+=1;
        if c.final {print("last block: % frames, % samples; % blocks, % samples, % transforms in all\n",c.frames,c.samples,chunks,at,st[0].ffts);}
    }
    peak:=0.0;for s:0..<at {if abs(audio[s])>peak {peak=abs(audio[s]);}}
    print("peak amplitude %\n",peak);
```

<!-- source: psionic/examples/speak.coax offline -->
```jai
    work2:[40000]f64;re:[100000]f64;im:[100000]f64;want:[40000]f64;
    #assert spk_ref_blocks(sh,u[..],work2[..],re[..],im[..],want[..])==L;
    print("stream equals its blocks solved offline, bit for bit: %\n",spk_same(audio[0..<at],want[0..<L]));
```

<!-- source: psionic/examples/speak.coax hear -->
```jai
    bank:[24*129]f64;tmpl:[26*24]f64;#assert spk_templates(sh,bank[..],tmpl[..])==24;
    labels:[600]int;spk_label(sh,audio[0..<at],T,bank[..],tmpl[..],labels[0..<T]);
    meant:[256]int;heard:[600]int;m:=spk_meant(u[..],meant[..]);h:=spk_runs(labels[0..<T],heard[..]);
    print("meant:");showp(meant[0..<m]);print("heard:");showp(heard[0..<h]);
    print("% phone errors in % (template recognizer)\n",spk_phone_errors(meant[0..<m],heard[0..<h]),m);
```

<!-- source: psionic/examples/speak.coax barge -->
```jai
    #assert spk_open(sh,u[..],work[..],st[..])==SpkStatus.ok;
    at=0;for i:0..<10 {at+=spk_next(st[..],u[..],work[..],audio[at..<40000],curve[..]).samples;}
    s:=spk_cancel(st[..],u[..]);
    print("barge-in after 10 blocks: % samples emitted (% ms), % pending dropped, % never made\n",s.emitted,s.emitted/8,s.pending,s.unmade);
    print("  stopped in phone '%' of word % (%); words heard to their end: %\n",SPK_NAMES[s.phone],s.word,word_at(text,s.word),s.heard);
    print("  next block: %\n",spk_next(st[..],u[..],work[..],audio[..],curve[..]).status);
```

```shell
coax/bin/coax run psionic/examples/speak.coax
```

```text
text: The answer is 42.
normalized: the answer is forty two
20 phones in 5 words: _ th e a n s w e r i s f o r t i t w o _
204 frames, 13248 samples (1656 ms); context 3 frames, work 13192 values
first audio: 512 samples (64 ms) after 8 frames and 392 transforms
block 4 (frames 32 to 39): distance 0.4577088631412432 after 1 iteration, 0.3755840448696475 after 24
last block: 4 frames, 448 samples; 26 blocks, 13248 samples, 11871 transforms in all
then: finished
peak amplitude 0.7833820100678568
stream equals its blocks solved offline, bit for bit: true
meant: th e a n s w e r i s f o r t i t w o
heard: th e a n z s z i w e r i z s f o r d i y t i w o w
8 phone errors in 18 (template recognizer)
barge-in after 10 blocks: 5120 samples emitted (640 ms), 192 pending dropped, 7936 never made
  stopped in phone 'e' of word 1 (answer); words heard to their end: 1
  next block: cancelled
```

The counts match the hand trace: 204 frames, 13,248 samples, 392
transforms before the first 512 samples, 11,871 in all, and the barge-in
numbers. Block 4 holds the start of `answer`; its distance falls from 0.458
to 0.376 over 24 iterations and stalls there. The template recognizer's
phones are 8 edits from the 18 meant: voiceless `s` turned voiced `z`,
the `t` of `forty` heard as `d`, extra glides. The peak amplitude is 0.78,
so nothing clips.

## What is refused

The second example, [`speak-refuse.coax`](../examples/speak-refuse.coax), sends text and calls that should not be taken:

```shell
coax/bin/coax run psionic/examples/speak-refuse.coax
```

<!-- source: psionic/examples/speak-refuse.coax refuse -->
```jai
main :: () {
    out:[64]u8;
    texts:=[5]string.["call 5551234 now","50 € off","...","","a very long answer"];
    caps:=[5]int.[64,64,64,64,8];
    for i:0..<5 {r:=spk_normalize(texts[i],out[0..<caps[i]]);print("\"%\" in % bytes: % at byte %\n",texts[i],caps[i],r.status,r.at);}
    u:[1]SpkUtterance;
    print("phonemize \"Hello\": %\n",spk_phonemize("Hello",u[..]));
    #assert spk_phonemize("hi",u[..])==SpkStatus.ok;
    sh:=SpkShape.{rate=8000,n=256,hop=64,block=8,iters=24};work:[20000]f64;st:[1]SpkStream;audio:[2000]f64;curve:[24]f64;
    print("next before open: %\n",spk_next(st[..],u[..],work[..],audio[..],curve[..]).status);
    print("FFT of 384: %\n",spk_open(SpkShape.{rate=8000,n=384,hop=96,block=8,iters=24},u[..],work[..],st[..]));
    print("hop of 16 (16 frames over a sample): %\n",spk_open(SpkShape.{rate=8000,n=256,hop=16,block=8,iters=24},u[..],work[..],st[..]));
    print("blocks of 33: %\n",spk_open(SpkShape.{rate=8000,n=256,hop=64,block=33,iters=24},u[..],work[..],st[..]));
    print("work one value short: %\n",spk_open(sh,u[..],work[0..<spk_plan(sh).work-1],st[..]));
    #assert spk_open(sh,u[..],work[..],st[..])==SpkStatus.ok;
    print("room for 511 of 512 samples: %\n",spk_next(st[..],u[..],work[..],audio[0..<511],curve[..]).status);
    print("room for 23 of 24 distances: %\n",spk_next(st[..],u[..],work[..],audio[..],curve[0..<23]).status);
    print("then the block: % samples\n",spk_next(st[..],u[..],work[..],audio[..],curve[..]).samples);
    s:=spk_cancel(st[..],u[..]);print("barge-in: %, % samples emitted\n",s.status,s.emitted);
    print("next after barge-in: %\n",spk_next(st[..],u[..],work[..],audio[..],curve[..]).status);
    print("barge-in again: %\n",spk_cancel(st[..],u[..]).status);
}
```

```text
"call 5551234 now" in 64 bytes: number at byte 5
"50 € off" in 64 bytes: symbol at byte 3
"..." in 64 bytes: empty at byte 0
"" in 64 bytes: empty at byte 0
"a very long answer" in 8 bytes: capacity at byte 8
phonemize "Hello": symbol
next before open: closed
FFT of 384: shape
hop of 16 (16 frames over a sample): shape
blocks of 33: shape
work one value short: capacity
room for 511 of 512 samples: capacity
room for 23 of 24 distances: capacity
then the block: 512 samples
barge-in: ok, 512 samples emitted
next after barge-in: cancelled
barge-in again: cancelled
```

A refused call changes nothing: the checks refuse a block twice, then
take it, and compare it with the first block of a fresh stream bit for
bit.

## How it is checked

The checks in `speak.coax` use small shapes (16-point frames) so they run
in the interpreter, and compare against references in
[`speak-checks.coax`](../tests/speak-checks.coax):

- the tables are pinned by weighted sums, and normalization, letter-to-sound
  and the acoustic model are checked against hand values;
- the Hann overlap sums to 3/2, and an STFT made with the direct DFT comes
  back through the least-squares inverse within 1e-12;
- Griffin-Lim's distance never rises, for consistent and inconsistent
  targets with 0 to 3 context frames, and every free frame ends with its
  target magnitudes; one iteration's distance equals an independent
  computation from the least-squares inverse and the direct DFT, and the
  first estimate's phase turn is checked against cos and sin;
- a stream with one block equals textbook Griffin-Lim over the whole
  utterance (every frame at once, one inverse STFT at the end) bit for bit;
- a stream with blocks of 1 to 7 frames equals the same blocks solved
  offline, every frame kept and added up at the end, bit for bit;
- the transforms before first audio and in each block match the formula;
- for barge-in at nine points of a 26-block utterance (before the first
  block, after the last, and around the pause between its words), the emitted audio is a
  bit-for-bit prefix of the full stream, emitted, pending and never-made
  samples add up to the utterance, and the phone, word and words heard
  match a recount;
- refused calls leave the stream unchanged, and a stream reopened after a
  barge-in equals a fresh one;
- the recognizer's features equal the direct DFT through the mel bank, a
  frame drawn from a phone's spectrum is heard as that phone, and ties go to
  the lower phone;
- `coax mutate --root-only` leaves no surviving mutant in `speak.coax`.

## What remains

- **A real model.** The rules stand in for a learned acoustic model and
  vocoder. A pinned checkpoint needs a survey entry with its weights, code
  and data terms first; Voxtral-4B-TTS is a candidate, but its CC BY-NC 4.0
  weights exclude commercial use. Running one also needs its architecture
  in Coax, which no chapter has yet.
- **Natural speech and intelligibility.** The output is recognizably
  structured but robotic. The draft's measure, the word error rate of
  chapter 153's transcription of the output, needs a real model on both
  sides.
- **Playback.** The stream hands out samples; feeding them to the voice
  engine's jitter buffer, gain and duck through `verse/platform/voice.h`,
  and taking barge-in from it, is not wired.
- **Time.** First audio and real time are counted in transforms, not
  measured on a named host.
- **Incremental text.** The whole text is phonemized before the first
  block; a language model's answer arriving word by word is not handled.
