# 184. A pipeline of models

> **Status: partly implemented.** A four-stage voice pipeline (profile
> vp-pipeline-v1) runs on the CPU with stand-in stages on simulated 10 ms
> ticks: the end of the user's turn, partial and final transcripts, a reply
> made token by token, and speech made sentence by sentence. Each handoff
> crosses a hop with its own delay. The language model can start early on
> a quiet partial transcript, speech starts on the first complete sentence,
> a bounded buffer stalls the model, the time to first audio is split into
> one share per stage and hop against a written budget, and a barge-in at
> any tick reports what each stage made, handed on, emitted and heard. Not
> built yet: the real recognizer, language model and speech stages, replicas
> and autoscaling for each stage, hops measured on a network, and P50, P95
> and P99 from [chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s
> benchmark. The last section lists them.

A spoken answer passes through several models. Voice activity detection
decides that the user has stopped talking. A recognizer turns the audio into
words. A language model writes a reply. A speech model turns the reply into
audio. Each stage is its own service, and each handoff between two stages
crosses a network hop.

Two numbers matter to the person speaking. The first is how long the
silence lasts between the end of their speech and the first sound of the
answer. The second is how quickly the answer stops when they talk over it.
Both numbers depend on how the stages hand work to each other, not only on
how fast each stage is. If every stage waits for the one before it to
finish, the delays add up. If the stages overlap, the first sound can come
while the language model is still writing.

This chapter builds the handoffs in an `#authority` file,
[`voice-pipeline.coax`](../lib/voice-pipeline.coax), because the file
decides when work starts, when work is thrown away and what a cancellation
reports. The stages are stand-ins with fixed timing, so every tick of a
turn can be worked out by hand and checked.

## The stages and their contracts

Time is counted in ticks of 10 ms. A turn is the user's words, as word
ids, the tick each word's audio ends, and the tick the user stops speaking.
The configuration gives every stage's timing and the delay of each of the
three hops: recognizer to language model, language model to speech, and
speech to the listener.

<!-- source: psionic/lib/voice-pipeline.coax types -->
```jai
VpWhy :: enum u8 {none;words;order;delay;policy;buffer;barge;}
// The user's turn: word ids (1 to 999), the tick each word's audio ends,
// and the tick the user stops speaking.
VpTurn :: struct {n:int;word:[16]int;heard:[16]int;end:int;}
// Every stage's timing, in ticks of 10 ms. hangover: silence before the
// end of the turn is declared. asr_lag: a partial word after its audio;
// asr_final: the final transcript after the end of the turn. quiet: ticks
// a partial transcript must stay unchanged before the language model
// starts on it (0: always wait for the final one). prefill, per_word: the
// language model's start cost, fixed and per input word; step: ticks a
// token. cap: tokens the buffer holds. tts_first: ticks from taking a
// sentence to its first audio; word_ticks, pause: audio a word and a
// period. hop: ASR to LM, LM to TTS, TTS to the listener.
VpConfig :: struct {hangover:int;asr_lag:int;asr_final:int;quiet:int;prefill:int;per_word:int;step:int;cap:int;
    tts_first:int;word_ticks:int;pause:int;hop:[3]int;}
// A reply: tokens (0 is a period) and where each sentence ends.
VpReply :: struct {n:int;sentences:int;token:[18]int;ends:[3]int;}
```

The four stages work like this:

- **Voice activity.** The end of the turn is declared `hangover` ticks
  after the user stops speaking.
- **Recognizer.** Each word appears as a partial word `asr_lag` ticks after
  its audio ends. The final transcript appears `asr_final` ticks after the
  end of the turn. As in
  [chapter 154](ch154-transcribe-while-someone-speaks.md), a partial
  transcript only grows: a word that has appeared never changes. A partial
  word may not arrive after the final transcript that holds it, so the
  configuration must keep `asr_lag` at or below `hangover + asr_final`.
- **Language model.** A run on `m` words costs `prefill + per_word*m` ticks
  before its first token, then `step` ticks a token. The reply is fixed for
  each input: 1 to 3 sentences of 1 to 5 words, each sentence ending in a
  period, token 0.
- **Speech.** The speech stage takes a whole sentence from the buffer. The
  sentence's first audio comes `tts_first` ticks later. Each word plays for
  `word_ticks` ticks, and each period adds `pause` ticks of silence.

<!-- source: psionic/lib/voice-pipeline.coax model -->
```jai
// The language model stand-in: a fixed reply for each input. h hashes
// the words; 1 to 3 sentences of 1 to 5 words, each ending in a period,
// token 0, which the zeroed reply already holds.
vp_reply :: (word:[16]int,m:int)->VpReply {
    h:=0;for i:0..<m {h=(h*31+word[i])%1009;}
    r:VpReply;r.sentences=1+h%3;
    for s:0..<r.sentences {
        for w:0..<1+(h/3+s)%5 {r.token[r.n]=1+(h+31*s+7*w)%999;r.n+=1;}
        r.n+=1;r.ends[s]=r.n;
    }
    return r;
}
// Ticks of audio a sentence plays: its words and its period.
vp_duration :: (c:VpConfig,rep:VpReply,s:int)->int {
    from:=0;if s>0 {from=rep.ends[s-1];}
    return (rep.ends[s]-from-1)*c.word_ticks+c.pause;
}
// The tick the recognizer emits word i, after its audio. vp_check makes
// sure that is never after the final transcript.
vp_asr :: (r:VpRun,i:int)->int {return r.turn.heard[i]+r.cfg.asr_lag;}
```

## The handoffs

Three decisions make the stages overlap.

**When the language model starts.** With `quiet` set to 0, the model waits
for the final transcript. With `quiet` above 0, it starts on the partial
transcript once no new word has arrived for `quiet` ticks. When the final
transcript arrives, the pipeline compares it with the words the model
started on. If they match, the work is kept. If the final transcript has
more words, the model's tokens are thrown away and the model restarts on
the final transcript. The pipeline speculates at most once. Speech never
plays tokens whose input is not yet confirmed. Confirmation happens on the
tick the final transcript reaches the model.

**When speech starts.** The speech stage takes the first sentence as soon
as its period has crossed the hop and the input is confirmed. The speech
stage does not wait for the rest of the reply.

**How much the buffer holds.** Tokens wait in a buffer of `cap` tokens
between the model and the speech stage. The speech stage takes the next
sentence only when the sentence playing is about to end: `tts_first` ticks
before the end, so the audio continues without a gap. Until then, the
sentence stays in the buffer, and a full buffer stops the model. The
smallest buffer allowed holds one whole sentence, 6 tokens. A smaller buffer
could leave the model waiting for room that the speech stage would free only
after it got a sentence the model could not finish.

Each tick runs the three stages in a fixed order:

1. The transcript arrives at the model.
2. The model makes a token.
3. The speech stage takes a sentence.

A stage that starts on a tick does its first work on the next tick.

<!-- source: psionic/lib/voice-pipeline.coax tick -->
```jai
// The language model starts on words: no token before the next tick.
vp_start :: (rv:[]VpRun,words:int,spec:bool) {
    c:=rv[0].cfg;
    rv[0].started=true;rv[0].spec=spec;rv[0].ok=!spec;rv[0].words=words;rv[0].began=rv[0].t;
    rv[0].left=c.prefill+c.per_word*words+c.step;rv[0].made=0;rv[0].sentences=0;
    rv[0].reply=vp_reply(rv[0].turn.word,words);
}
// One tick, three stages in order. First the transcript: partial words
// arrive; the final one confirms the running model, restarts one that ran
// on fewer words (its tokens are thrown away, and the buffer held only
// those), or starts it; a partial quiet for long enough starts it early.
// Then the model makes a token if its time has come and the buffer has
// room. Then the speaker takes the next sentence if its period has crossed
// the hop and the sentence before it is about to end.
vp_tick :: (rv:[]VpRun) {
    c:=rv[0].cfg;t:=rv[0].t;
    while rv[0].seen<rv[0].turn.n && vp_asr(rv[0],rv[0].seen)+c.hop[0]<=t {rv[0].seen+=1;rv[0].changed=t;}
    if t==rv[0].confirm {
        if !rv[0].started {vp_start(rv,rv[0].turn.n,false);}
        else if rv[0].words==rv[0].turn.n {rv[0].ok=true;}
        else {rv[0].wasted+=rv[0].made;vp_start(rv,rv[0].turn.n,false);}
    } else if c.quiet>0 && !rv[0].started && rv[0].seen>0 && t-rv[0].changed>=c.quiet {vp_start(rv,rv[0].seen,true);}
    if rv[0].started && rv[0].began<t && rv[0].made<rv[0].reply.n {
        if rv[0].left>0 {rv[0].left-=1;}
        if rv[0].left==0 && rv[0].made-rv[0].head<c.cap {
            m:=rv[0].made;rv[0].ring[m%c.cap]=rv[0].reply.token[m];rv[0].made=m+1;rv[0].left=c.step;
            n:=rv[0].sentences;
            if rv[0].reply.ends[n]==m+1 {rv[0].period[n]=t;rv[0].sentences=n+1;}
        }
    }
    if !rv[0].ready && rv[0].ok && rv[0].sentences>0 {rv[0].ready=true;rv[0].ready_at=t;}
    k:=rv[0].taken;
    if rv[0].ok && k<rv[0].sentences && rv[0].period[k]+c.hop[1]<=t {
        if k==0 || t+c.tts_first>=rv[0].audio[k-1]+vp_duration(c,rv[0].reply,k-1) {
            while rv[0].head<rv[0].reply.ends[k] {h:=rv[0].head;rv[0].spoken[h]=rv[0].ring[h%c.cap];rv[0].head=h+1;}
            rv[0].take[k]=t;rv[0].audio[k]=t+c.tts_first;rv[0].taken=k+1;
        }
    }
    rv[0].t=t+1;
}
vp_done :: (r:VpRun)->bool {return r.taken>0 && r.taken==r.reply.sentences;}
// Run every tick before b, or until the speaker has taken the last sentence.
vp_until :: (rv:[]VpRun,b:int) {while rv[0].t<b && !vp_done(rv[0]) {vp_tick(rv);}}
```

`vp_check` refuses a turn or configuration before anything runs. A refused
pipeline runs no stage.

<!-- source: psionic/lib/voice-pipeline.coax open -->
```jai
vp_check :: (c:VpConfig,u:VpTurn)->VpWhy #export_c {
    if u.n<1 || u.n>VP_WORDS {return VpWhy.words;}
    for i:0..<u.n {if u.word[i]<1 || u.word[i]>999 {return VpWhy.words;}}
    if u.heard[0]<0 || u.end<u.heard[u.n-1] || u.end>VP_TIME {return VpWhy.order;}
    for i:1..<u.n {if u.heard[i]<=u.heard[i-1] {return VpWhy.order;}}
    d:=[12]int.[c.hangover,c.asr_lag,c.asr_final,c.prefill,c.per_word,c.step,c.tts_first,c.word_ticks,c.pause,c.hop[0],c.hop[1],c.hop[2]];
    for k:0..<12 {if d[k]<0 || d[k]>VP_DELAY {return VpWhy.delay;}}
    if c.step<1 || c.word_ticks<1 {return VpWhy.delay;}
    // A partial word may not arrive after the final transcript that holds it.
    if c.asr_lag>c.hangover+c.asr_final {return VpWhy.delay;}
    if c.quiet<0 || c.quiet>VP_DELAY {return VpWhy.policy;}
    // A smaller buffer could never hold a whole sentence: the model and the speaker would wait on each other.
    if c.cap<VP_SENTENCE || c.cap>VP_CAP {return VpWhy.buffer;}
    return VpWhy.none;
}
vp_open :: (c:VpConfig,u:VpTurn)->VpRun {
    r:VpRun;r.why=vp_check(c,u);if r.why!=VpWhy.none {return r;}
    r.cfg=c;r.turn=u;r.eot=u.end+c.hangover;r.fin=r.eot+c.asr_final;r.confirm=r.fin+c.hop[0];
    return r;
}
```

## What a barge-in reports

A barge-in is the user speaking again at tick `b`. Every stage stops at
`b`, and the report counts what each stage did in the ticks before `b`:
words the recognizer emitted, tokens the model made (thrown-away tokens
included), tokens still in the buffer, tokens the speech stage took, and
audio made, emitted and heard. Audio emitted at a tick reaches the listener
`hop[2]` ticks later, so the listener has heard less than the speech stage
has sent. A word counts as heard only if its last tick was heard. The
report also counts the bytes that crossed each hop. Each word or token is
4 bytes, and each tick of audio is 320 bytes, which is 160 samples of
16-bit audio at 16 kHz.

<!-- source: psionic/lib/voice-pipeline.coax report-types -->
```jai
// What every stage had done when the pipeline stopped at tick at. asr:
// words the recognizer emitted. lm: tokens the language model made, wasted
// ones included; buffered: tokens in the buffer; handed: tokens the
// speaker took. synthesized, emitted, heard: ticks of audio made, sent
// and heard. words: words heard to their end; cut: a word was cut off.
// bytes: what crossed each hop.
VpStop :: struct {why:VpWhy;at:int;asr:int;lm:int;wasted:int;buffered:int;handed:int;synthesized:int;emitted:int;heard:int;
    words:int;cut:bool;bytes:[3]int;}
// The time from the end of the user's speech to the first audio heard,
// split along the path the first sentence took: vad, asr, hop 0, lm,
// hop 1, tts, hop 2. vp_over sets a bit for each share past its budget.
VpLatency :: struct {first:int;share:[7]int;}
```

<!-- source: psionic/lib/voice-pipeline.coax stop -->
```jai
// Barge-in at tick b: every stage stops, and the report counts what each
// had done in the ticks before b. Audio the speaker took is made in full
// once its first tick is due; a word is heard if its last tick was.
vp_stop :: (rv:[]VpRun,b:int)->VpStop {
    if rv[0].why!=VpWhy.none {return VpStop.{why=rv[0].why};}
    if b<=rv[0].turn.end || b<rv[0].t {return VpStop.{why=VpWhy.barge};}
    vp_until(rv,b);
    c:=rv[0].cfg;r:=rv[0];
    s:=VpStop.{at=b,lm=r.wasted+r.made,wasted=r.wasted,buffered=r.made-r.head,handed=r.head};
    partials:=0;
    for i:0..<r.turn.n {
        e:=vp_asr(r,i);
        if e<b {s.asr+=1;}
        if e<r.fin && e<b {partials+=1;}
    }
    s.bytes[0]=4*partials;if r.fin<b {s.bytes[0]+=4*r.turn.n;}
    s.bytes[1]=4*s.lm;
    for k:0..<r.taken {
        d:=vp_duration(c,r.reply,k);a:=r.audio[k];
        if a<=b {s.synthesized+=d;}
        for q:0..<d {
            if a+q<b {s.emitted+=1;}
            if a+q+c.hop[2]<b {s.heard+=1;}
        }
        from:=0;if k>0 {from=r.reply.ends[k-1];}
        for w:0..<r.reply.ends[k]-from-1 {
            w0:=a+c.hop[2]+w*c.word_ticks;
            if w0+c.word_ticks<=b {s.words+=1;} else if w0<b {s.cut=true;}
        }
    }
    s.bytes[2]=VP_AUDIO*s.emitted;
    return s;
}
// No barge-in: run to the end, and report at the tick the last audio is heard.
vp_finish :: (rv:[]VpRun)->VpStop {
    if rv[0].why!=VpWhy.none {return VpStop.{why=rv[0].why};}
    // vp_check refused every configuration that could stall for ever.
    while !vp_done(rv[0]) {vp_tick(rv);}
    k:=rv[0].reply.sentences-1;
    return vp_stop(rv,rv[0].audio[k]+vp_duration(rv[0].cfg,rv[0].reply,k)+rv[0].cfg.hop[2]);
}
```

## The latency budget

The time to first audio runs from the tick the user stops speaking to the
tick the listener hears the first audio. The first sentence takes one path
through the stages, and that path splits the time into seven shares:

| Share | What it is |
| --- | --- |
| vad | the hangover before the end of the turn |
| asr | from the end of the turn to the final transcript |
| hop 0 | the final transcript's trip to the model |
| lm | from then until the first sentence is made, if the model is still making it |
| hop 1 | the sentence's trip to the speech stage, after that |
| tts | from taking the sentence to its first audio |
| hop 2 | the audio's trip to the listener |

The shares add up to the whole time. With an early start, the model may
finish the first sentence before the final transcript arrives. The `lm`
share is then 0, and the `hop 1` share is whatever remains of the hop.
`vp_over` compares each share with a written budget and sets one bit for
each share that runs over, so the report names the stage that ran over.

<!-- source: psionic/lib/voice-pipeline.coax latency -->
```jai
// The first sentence's path: the end of the turn, the final transcript,
// its hop, the model until the sentence was made with its input
// confirmed, the sentence's hop past that, the speaker's first audio and
// the last hop.
vp_latency :: (rv:[]VpRun)->VpLatency {
    r:=rv[0];c:=r.cfg;l:VpLatency;
    l.share=[7]int.[r.eot-r.turn.end,c.asr_final,c.hop[0],r.ready_at-r.confirm,r.take[0]-r.ready_at,c.tts_first,c.hop[2]];
    l.first=r.audio[0]+c.hop[2]-r.turn.end;
    return l;
}
vp_over :: (l:VpLatency,budget:[7]int)->int {
    m:=0;for k:0..<7 {if l.share[k]>budget[k] {m+=1<<k;}}
    return m;
}
```

## A turn by hand

The user says three words, ids 12, 40 and 7. Their audio ends at ticks 62,
70 and 80, and the user stops at tick 85. The configuration:

| Stage | Setting |
| --- | --- |
| voice activity | hangover 20 |
| recognizer | asr_lag 5, asr_final 4 |
| model | quiet 10, prefill 6, per_word 2, step 3 |
| buffer | cap 6 |
| speech | tts_first 8, word_ticks 25, pause 15 |
| hops | 2, 3, 2 |

**The reply.** The hash of the three words is
`((12*31 + 40)*31 + 7) mod 1009 = 12779 mod 1009 = 671`. The reply has
`1 + 671 mod 3 = 3` sentences. With `671 / 3 = 223`, sentence `s` has
`1 + (223 + s) mod 5` words: 4, 5 and 1. Word `w` of sentence `s` is
`1 + (671 + 31s + 7w) mod 999`. The reply is 672 679 686 693 and a period,
703 710 717 724 731 and a period, then 734 and a period. That is 13 tokens,
with the sentences ending after tokens 5, 11 and 13.

**The transcript.** The end of the turn is `85 + 20 = 105`. The final
transcript comes at `105 + 4 = 109` and reaches the model at
`109 + 2 = 111`. The partial words reach the model at
`62 + 5 + 2 = 69`, `77` and `87`. After word 2 arrives, nothing changes for
10 ticks, so at tick 97 the model starts early on all three words.

**The model.** The prefill is `6 + 2*3 = 12` ticks, so the first token
comes at `97 + 12 + 3 = 112`, and one token follows every 3 ticks. At tick
111 the final transcript arrives with the same three words, so the work is
kept and nothing is thrown away. The first sentence's period is token 4, at
`112 + 4*3 = 124`.

**First audio.** The sentence crosses the hop at `124 + 3 = 127`, and the
speech stage takes it then. Its audio starts at `127 + 8 = 135` and reaches
the listener at 137. The time to first audio is `137 - 85 = 52` ticks, or
520 ms:

| vad | asr | hop 0 | lm | hop 1 | tts | hop 2 | total |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 105 − 85 = 20 | 4 | 2 | 124 − 111 = 13 | 127 − 124 = 3 | 8 | 2 | 52 |

If the model had waited for the final transcript, it would have started at
111. Its first period would come at `111 + 12 + 3 + 4*3 = 138`, and the
time to first audio would be `52 + 14 = 66` ticks. The early start saves
the 14 ticks between 97 and 111.

**The stall.** Sentence 0 plays `4*25 + 15 = 115` ticks, from 135 to 250.
The model makes tokens 5 to 10, which are sentence 1, at ticks 127, 130,
..., 142. Token 5 fits because the speech stage took sentence 0 at tick
127, which leaves 5 tokens and room. The buffer then holds sentence 1's 6
tokens and is full. The speech stage takes sentence 1 only at
`250 - 8 = 242`, so token 11 waits from tick 145 until tick 243, and token
12 follows at 246. Sentence 1 plays `5*25 + 15 = 140` ticks, from 250 to
390. Sentence 2 is taken at `390 - 8 = 382` and plays `25 + 15 = 40` ticks,
from 390 to 430. The listener hears the end at 432. The stall costs the
listener nothing, because the speech stage is the slower stage. A buffer of
32 tokens lets the model finish at tick 148, and the last audio still
arrives at 432.

**A barge-in at tick 200.** The model has made 11 tokens: sentences 0 and
1. Sentence 0's 5 tokens went to the speech stage, and sentence 1's 6
tokens are in the buffer. The speech stage has made sentence 0's 115 ticks
of audio and emitted `200 - 135 = 65` of them. The listener has heard
`200 - 137 = 63` ticks. That covers word 0 (ticks 137 to 162) and word 1
(162 to 187). Word 2, which runs from 187 to 212, is cut off. The bytes are
`3*4 + 3*4 = 24` over hop 0, for three partial words and the final
transcript. They are `11*4 = 44` over hop 1, and `65*320 = 20,800` over
hop 2.

**A partial that grows.** With `quiet` set to 4, word 0 reaches the model
at 69 and nothing changes until 73. The model starts on one word. Its
reply, for the hash 12, is one sentence of 5 words, made by tick 99. That
sentence waits unconfirmed. At tick 111 the final transcript has three
words, so the 6 tokens are thrown away and the model restarts on the final
transcript. The time to first audio is the same 66 ticks as waiting for
the final transcript. The early start cost 6 tokens of model work and saved
nothing.

The first test checks every number above against the pipeline, tick by
tick.

<!-- source: psionic/lib/voice-pipeline.coax check-hand -->
```jai
#test "the hand trace: a quiet partial, a stall, a barge-in" {
    c:=vpx_config();u:=vpx_turn();
    rep:=vp_reply(u.word,3);
    // A reply whose token ids wrap past 999.
    w:[16]int;w[0]=998;big:=vp_reply(w,1);
    #assert big.n==15 && big.sentences==3 && big.ends[0]==4 && big.ends[1]==9 && big.ends[2]==15;
    #assert big.token[0]==999 && big.token[1]==7 && big.token[2]==14 && big.token[3]==0 && big.token[14]==0;
    #assert rep.n==13 && rep.sentences==3 && rep.ends[0]==5 && rep.ends[1]==11 && rep.ends[2]==13;
    #assert rep.token[0]==672 && rep.token[3]==693 && rep.token[4]==0 && rep.token[5]==703 && rep.token[11]==734 && rep.token[12]==0;
    r:=[1]VpRun.[vp_open(c,u)];
    #assert r[0].why==VpWhy.none && r[0].eot==105 && r[0].fin==109 && r[0].confirm==111;
    // The partial transcript is quiet from tick 87; the model starts at 97.
    vp_until(r[..],97);#assert !r[0].started && r[0].seen==3 && r[0].changed==87;
    vp_until(r[..],98);#assert r[0].started && r[0].spec && !r[0].ok && r[0].began==97 && r[0].words==3 && r[0].left==15;
    // Tokens at 112, 115, ...; the final transcript confirms it at 111.
    vp_until(r[..],112);#assert r[0].ok && r[0].made==0;
    vp_until(r[..],113);#assert r[0].made==1 && r[0].ring[0]==672;
    // The first sentence's period at 124; the speaker takes it at 127.
    vp_until(r[..],127);#assert r[0].period[0]==124 && r[0].taken==0 && r[0].made==5;
    vp_until(r[..],128);#assert r[0].taken==1 && r[0].take[0]==127 && r[0].audio[0]==135 && r[0].head==5 && r[0].made==6;
    l:=vp_latency(r[..]);
    #assert r[0].ready_at==124 && l.first==52 && l.share[0]==20 && l.share[1]==4 && l.share[2]==2 && l.share[3]==13 && l.share[4]==3 && l.share[5]==8 && l.share[6]==2;
    // The second sentence fills the buffer at 142; token 11 waits until 243.
    vp_until(r[..],243);#assert r[0].made==11 && r[0].made-r[0].head==0 && r[0].taken==2 && r[0].take[1]==242;
    vp_until(r[..],244);#assert r[0].made==12;
    s:=vp_stop(r[..],200);#assert s.why==VpWhy.barge;
    r[0]=vp_open(c,u);s=vp_stop(r[..],200);#assert r[0].t==200 && vp_stop(r[..],200).lm==11;
    #assert s.why==VpWhy.none && s.at==200 && s.asr==3 && s.lm==11 && s.wasted==0 && s.buffered==6 && s.handed==5;
    #assert s.synthesized==115 && s.emitted==65 && s.heard==63 && s.words==2 && s.cut;
    #assert s.bytes[0]==24 && s.bytes[1]==44 && s.bytes[2]==65*320;
    // Without a barge-in: all of it, ending at 432.
    r[0]=vp_open(c,u);s=vp_finish(r[..]);
    #assert s.at==432 && s.lm==13 && s.handed==13 && s.buffered==0 && s.emitted==295 && s.heard==295 && s.words==10 && !s.cut;
    #assert r[0].take[2]==382 && r[0].audio[2]==390;
    for k:0..<13 {#assert r[0].spoken[k]==rep.token[k];}
}
```

## Running it

The example, [`voice-pipeline.coax`](../examples/voice-pipeline.coax),
runs the hand-worked turn and prints the timeline, the latency shares
against a budget that allows the model 10 ticks, the full run, the barge-in,
both start policies, and both buffer sizes.

<!-- source: psionic/examples/voice-pipeline.coax main -->
```jai
main :: () {
    c:=vpx_config();u:=vpx_turn();
    print("%: words % % %, heard until % % %, the user stops at %\n",VP_PROFILE,u.word[0],u.word[1],u.word[2],u.heard[0],u.heard[1],u.heard[2],u.end);
    rep:=vp_reply(u.word,u.n);
    print("reply: % tokens in % sentences:",rep.n,rep.sentences);
    for j:0..<rep.n {print(" %",rep.token[j]);}
    print("\n");
    budget:=[7]int.[20,4,2,10,3,8,2];

    r:=[1]VpRun.[vp_open(c,u)];
    s:=vp_finish(r[..]);timeline(r[0]);latency(r[..],budget);show("no barge-in",s);
    r[0]=vp_open(c,u);show("barge-in",vp_stop(r[..],200));

    w:=c;w.quiet=0;r[0]=vp_open(w,u);s=vp_finish(r[..]);
    print("waiting for the final transcript: ");latency(r[..],budget);
    w=c;w.quiet=4;r[0]=vp_open(w,u);s=vp_finish(r[..]);
    print("starting after 4 quiet ticks: on % word, % tokens thrown away; ",1,s.wasted);latency(r[..],budget);
    caps:=[2]int.[6,32];
    for k:0..<2 {
        w=c;w.cap=caps[k];r[0]=vp_open(w,u);s=vp_finish(r[..]);
        print("a %-token buffer: the model's last token at %, the last audio heard at %\n",caps[k],r[0].period[2],s.at);
    }
}
```

```shell
coax/bin/coax run psionic/examples/voice-pipeline.coax
```

```text
vp-pipeline-v1: words 12 40 7, heard until 62 70 80, the user stops at 85
reply: 13 tokens in 3 sentences: 672 679 686 693 0 703 710 717 724 731 0 734 0
end of turn 105, final transcript 109, at the model 111
model: started at 97 on 3 words (early: true)
sentence 0: period at 124, taken at 127, audio from 135 (heard from 137)
sentence 1: period at 142, taken at 242, audio from 250 (heard from 252)
sentence 2: period at 246, taken at 382, audio from 390 (heard from 392)
first audio 52 ticks after the user stopped: vad 20/20 asr 4/4 hop 0 2/2 lm 13/10 hop 1 3/3 tts 8/8 hop 2 2/2
  over budget: lm
no barge-in: at tick 432
  recognizer: 3 words
  model: 13 tokens made, 0 thrown away, 0 in the buffer, 13 handed to the speaker
  speaker: 295 ticks made, 295 emitted, 295 heard; 10 words heard, cut off mid-word: false
  bytes: 24 over hop 0, 52 over hop 1, 94400 over hop 2
barge-in: at tick 200
  recognizer: 3 words
  model: 11 tokens made, 0 thrown away, 6 in the buffer, 5 handed to the speaker
  speaker: 115 ticks made, 65 emitted, 63 heard; 2 words heard, cut off mid-word: true
  bytes: 24 over hop 0, 44 over hop 1, 20800 over hop 2
waiting for the final transcript: first audio 66 ticks after the user stopped: vad 20/20 asr 4/4 hop 0 2/2 lm 27/10 hop 1 3/3 tts 8/8 hop 2 2/2
  over budget: lm
starting after 4 quiet ticks: on 1 word, 6 tokens thrown away; first audio 66 ticks after the user stopped: vad 20/20 asr 4/4 hop 0 2/2 lm 27/10 hop 1 3/3 tts 8/8 hop 2 2/2
  over budget: lm
a 6-token buffer: the model's last token at 246, the last audio heard at 432
a 32-token buffer: the model's last token at 148, the last audio heard at 432
```

The model's 13-tick share runs over its budget of 10, and the report names
it. Every other share is within its budget.

## Independent checks

Three checks compare the pipeline with references written separately in
[`voice-pipeline-checks.coax`](../tests/voice-pipeline-checks.coax), on
random turns and configurations. The turns have 1 to 6 words, and
sometimes 16. The configurations include both start policies and buffers
from 6 to 12 tokens, and sometimes 32.

- **The stages one after another.** The reference runs the whole
  transcript, then the whole reply, then all of the speech, with no
  overlap. The pipeline must speak exactly the reply's tokens, in order,
  with the same audio and words. Its time to first audio and its time to
  last audio must be no later than the sequential ones. When the reply is
  one sentence and the model waits for the final transcript, there is
  nothing to overlap, and the two times must be equal.
- **An event-time oracle.** The oracle does not step through ticks. It
  computes when each partial word arrives, when the early start happens,
  when each token is made, when each sentence is taken and when its audio
  starts, each from the times before it. It searches tick by tick only
  while the buffer is full. It then counts what a barge-in at tick `b`
  would find: words, tokens, thrown-away tokens, buffered and handed
  tokens, audio made, emitted and heard tick by tick, whole words, a cut
  word, and bytes. The pipeline's report must equal the oracle's for four
  random barge-in times per turn. That includes times before the final
  transcript, while an early run is still unconfirmed. The latency shares
  must also equal the oracle's and add up to the whole time.
- **The buffer, tick by tick.** After every tick, the buffer holds
  between 0 and `cap` tokens, which are the reply's untaken tokens, in
  order. The speech stage holds the start of the reply, and only after the
  input was confirmed.

<!-- source: psionic/lib/voice-pipeline.coax check-random -->
```jai
#forall "the pipeline equals the stages run one after another" seed:0..<300 #steps 40000000 {
    vpx_sequential_check(seed);
}
#forall "a barge-in at any tick reports what the event-time oracle counts" seed:0..<400 #steps 400000000 {
    vpx_barge_check(seed);
}
#forall "the buffer never holds more than its capacity, and holds the reply in order" seed:0..<120 #steps 80000000 {
    vpx_buffer_check(seed);
}
```

`coax mutate` on the file kills all 463 of its mutants.

## Refusals

A turn with no words, more than 16 words, or a word id outside 1 to 999 is
refused as `words`. Word ends that do not strictly increase, a negative
tick, or a user who stops before the last word ends or after tick 1,000 is
refused as `order`. A delay outside 0 to 100 ticks, a step or word length
of 0, or a partial word that would come after the final transcript is
refused as `delay`. A quiet time outside 0 to 100 is refused as `policy`.
A buffer smaller than a sentence or larger than 32 tokens is refused as
`buffer`. A refused pipeline runs no tick. A barge-in at or before the tick
the user stopped speaking is not a barge-in, and neither is a barge-in
earlier than a tick already run. Both are refused as `barge` and stop
nothing.

<!-- source: psionic/lib/voice-pipeline.coax check-refuse -->
```jai
#test "turns, configurations and barge-ins the pipeline refuses" {
    c:=vpx_config();u:=vpx_turn();
    v:=u;v.n=0;#assert vp_open(c,v).why==VpWhy.words;
    v=u;for i:0..<16 {v.word[i]=i+1;v.heard[i]=i;}v.n=17;#assert vp_open(c,v).why==VpWhy.words;
    v=u;v.word[0]=0;#assert vp_open(c,v).why==VpWhy.words;
    v=u;v.word[1]=0;#assert vp_open(c,v).why==VpWhy.words;
    v=u;v.word[2]=1000;#assert vp_open(c,v).why==VpWhy.words;
    v=u;v.heard[1]=62;#assert vp_open(c,v).why==VpWhy.order;
    v=u;v.heard[0]= -1;#assert vp_open(c,v).why==VpWhy.order;
    v=u;v.end=79;#assert vp_open(c,v).why==VpWhy.order;
    v=u;v.end=1001;#assert vp_open(c,v).why==VpWhy.order;
    v=u;v.heard[2]=1001;v.end=1001;#assert vp_open(c,v).why==VpWhy.order;
    d:=c;d.step=0;#assert vp_open(d,u).why==VpWhy.delay;
    d=c;d.word_ticks=0;#assert vp_open(d,u).why==VpWhy.delay;
    e:=c;e.hangover=100;
    for k:0..<12 {
        #assert vp_open(vpx_set(c,k,-1),u).why==VpWhy.delay && vp_open(vpx_set(c,k,101),u).why==VpWhy.delay;
        #assert vp_open(vpx_set(e,k,100),u).why==VpWhy.none;
    }
    // A partial word after the final transcript: hangover 20 and 4 ticks to the final one.
    d=c;d.asr_lag=24;#assert vp_open(d,u).why==VpWhy.none;
    d=c;d.asr_lag=25;#assert vp_open(d,u).why==VpWhy.delay;
    d=c;d.quiet= -1;#assert vp_open(d,u).why==VpWhy.policy;
    d=c;d.quiet=101;#assert vp_open(d,u).why==VpWhy.policy;
    d=c;d.cap=5;#assert vp_open(d,u).why==VpWhy.buffer;
    d=c;d.cap=33;#assert vp_open(d,u).why==VpWhy.buffer;
    // The edges are accepted.
    v=u;v.word[0]=1;v.word[1]=999;v.heard[0]=0;v.end=80;#assert vp_open(c,v).why==VpWhy.none;
    v=u;v.n=16;for i:0..<16 {v.word[i]=i+1;v.heard[i]=i;}v.end=1000;#assert vp_open(c,v).why==VpWhy.none;
    d=c;d.step=1;d.word_ticks=1;d.cap=6;d.quiet=0;d.hop=[3]int.[0,0,0];#assert vp_open(d,u).why==VpWhy.none;
    d=c;d.cap=32;d.quiet=100;d.hop=[3]int.[100,100,100];d.hangover=100;d.pause=0;#assert vp_open(d,u).why==VpWhy.none;
    // A barge-in is the user speaking again: after the end of the turn.
    r:=[1]VpRun.[vp_open(c,u)];
    #assert vp_stop(r[..],85).why==VpWhy.barge && r[0].t==0;
    #assert vp_stop(r[..],86).why==VpWhy.none && r[0].t==86;
    // A refused pipeline runs nothing.
    d=c;d.cap=5;r[0]=vp_open(d,u);s:=vp_finish(r[..]);#assert s.why==VpWhy.buffer && r[0].t==0 && s.at==0;
    s=vp_stop(r[..],200);#assert s.why==VpWhy.buffer && r[0].t==0 && s.at==0;
    names:=[7]string.["none","words","order","delay","policy","buffer","barge"];
    for k:0..<7 {#assert vp_why_name(cast(VpWhy)cast(u8)k)==names[k];}
}
```

The second example,
[`voice-pipeline-refuse.coax`](../examples/voice-pipeline-refuse.coax),
prints each refusal and exits with status 0.

<!-- source: psionic/examples/voice-pipeline-refuse.coax main -->
```jai
main :: () {
    c:=vpx_config();u:=vpx_turn();
    v:=u;v.n=0;open("no words",c,v);
    v=u;v.word[1]=1000;open("word id 1000",c,v);
    v=u;v.heard[1]=62;open("two words ending at tick 62",c,v);
    v=u;v.end=79;open("the user stops before the last word ends",c,v);
    d:=c;d.step=0;open("0 ticks a token",d,u);
    d=c;d.hop[1]=101;open("a 101-tick hop",d,u);
    d=c;d.quiet= -1;open("a quiet time of -1",d,u);
    d=c;d.cap=5;open("a 5-token buffer",d,u);
    r:=[1]VpRun.[vp_open(c,u)];
    print("barge-in at 85, as the user stops: %\n",vp_why_name(vp_stop(r[..],85).why));
    s:=vp_stop(r[..],150);
    print("barge-in at 150: %, % tokens made\n",vp_why_name(s.why),s.lm);
    print("barge-in at 140, after 150: %\n",vp_why_name(vp_stop(r[..],140).why));
}
```

```shell
coax/bin/coax run psionic/examples/voice-pipeline-refuse.coax
```

```text
no words: words, 0 ticks run
word id 1000: words, 0 ticks run
two words ending at tick 62: order, 0 ticks run
the user stops before the last word ends: order, 0 ticks run
0 ticks a token: delay, 0 ticks run
a 101-tick hop: delay, 0 ticks run
a quiet time of -1: policy, 0 ticks run
a 5-token buffer: buffer, 0 ticks run
barge-in at 85, as the user stops: barge
barge-in at 150: none, 11 tokens made
barge-in at 140, after 150: barge
```

## What remains

- **Real stages.** The stand-ins have fixed timing. To replace them,
  [chapter 154](ch154-transcribe-while-someone-speaks.md)'s `ls_push` would
  supply the partial and final transcripts, a qualified decision model
  would write the reply, and
  [chapter 155](ch155-speak-the-answer.md)'s `spk_open`, `spk_next` and
  `spk_cancel` would make the audio. Their measured latencies would then
  set the configuration.
- **Separate scaling.** Each stage would get its own replicas and
  autoscaler settings, through
  [chapter 177](ch177-route-queue-and-scale-replicas.md)'s routing. The
  check would be that scaling one stage changes that stage's share and
  leaves the others within their budgets.
- **Measured hops.** The hops here have fixed delays and counted bytes. A
  cluster would measure them.
- **Percentiles.** [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s
  benchmark would run many turns and report the time to first audio at
  P50, P95 and P99, with each stage's share against the budget.
- **More than one early start.** The model speculates once. It could
  instead restart whenever a quiet partial transcript grows.

## Run the checks

```shell
coax/bin/coax test psionic/lib/voice-pipeline.coax
coax/bin/coax mutate psionic/lib/voice-pipeline.coax --root-only
psionic/check.sh --only voice-pipeline
```

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
