# 185. A decision model steers voice

> **Status: partly implemented.** A stand-in decision model (profile
> vs-steer-v1) steers [chapter 184](ch184-a-pipeline-of-models.md)'s voice
> pipeline on the CPU. It answers typed questions about turn-taking: end the
> user's turn now or hold it, start the language model early, and stop the
> reply for sound over it or ignore the sound. The actions come from costs
> written down first, through [chapter 216](ch216-thresholds-from-written-costs.md)'s
> cheapest answer. Every answer passes a gate with the voice engine's order
> (shape, range, confidence floor, epoch, deadline), and every dropped answer
> has a recorded reason. A missing, late or dropped answer leaves the fixed
> rules in charge, and a model that can't answer inside its share of the
> latency budget is refused. On 400 drawn conversations the model is measured
> against the fixed rules, and [chapter 241](ch241-gates-that-can-say-unverifiable.md)'s
> gate gives the verdict. Not built yet: Elephant as the decider behind
> [chapter 20](ch020-serve-systemone.md)'s `/v1/systemone`, a trained model,
> real audio, and wall-clock decision latency from
> [chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s benchmark. The
> last section lists them.

The voice engine in `verse/voice.coax` makes its turn-taking decisions with
fixed rules. A turn ends after 700 ms of silence. The reply stops when speech
over it has lasted 240 ms, counted by a counter that rises on a loud,
speech-like frame and falls on any other. Each rule is cheap and never late,
but each is wrong in a predictable way. A 700 ms rule cuts off a person who
pauses to think, and it makes everyone else wait 700 ms for an answer. A
counter can't tell a cough from a word.

A decision model can do better, but it is slower than a rule, and it can be
wrong or absent. This chapter keeps the rules and lets the model steer them.
The model answers questions at fixed points, and each answer is a proposal.
The engine never waits for an answer. An answer that arrives late, fails the
gate or never arrives changes nothing, and the rule acts as it would have.

The work is in an `#authority` file,
[`voice-steer.coax`](../lib/voice-steer.coax), because it decides when the
user's turn is over and when the assistant stops talking.

```shell
coax/bin/coax run psionic/examples/voice-steer.coax
coax/bin/coax run psionic/examples/voice-steer-refuse.coax
psionic/check.sh --only voice-steer
```

## Conversations, questions and answers

Time is counted in [chapter 184](ch184-a-pipeline-of-models.md)'s ticks of
10 ms. A conversation, a `VsLine`, is chapter 184's user turn plus what the
model listens to: when each word starts, whether it sounds complete, whether
its pitch falls, and the sound that comes over the reply. Each tick of that
sound has three bits: speech-like, broadband (a cough is broadband) and
louder than the reply's own echo.

<!-- source: psionic/lib/voice-steer.coax types -->
```jai
VsKind :: enum u8 {turn_end;early_start;barge;}
VsAct :: enum u8 {none;end;hold;start;stop;ignore;}
VsDrop :: enum u8 {kept;absent;shape;clamp;floor;epoch;late;}
VsWhy :: enum u8 {none;costs;timing;budget;floor;pipe;line;size;}
VsEvent :: enum u8 {cough;backchannel;speech;}
// What each mistake costs, written before anything runs: cutting the user
// off, each tick of silence or of talking over the user, a language-model
// run thrown away, stopping the reply for nothing, and never stopping it.
VsCosts :: struct {cut:f64;tick:f64;restart:f64;false_stop:f64;missed:f64;}
// The stand-in model's timing: an answer arrives latency ticks after the
// question, plus 0 to jitter ticks drawn per call. on: false is a stopped model.
VsModel :: struct {on:bool;latency:int;jitter:int;seed:u64;}
// share: ticks the decision may take, its share of the end-of-turn budget.
// floor: the gate's confidence floor. rule_end: the fixed rule's silence;
// hold_to: the silence a held turn waits for; ask_end: when the model is
// asked. window: when it is asked about sound over the reply; rule_barge:
// the voice engine's counter that stops the reply. pipe: chapter 184.
VsConfig :: struct {costs:VsCosts;model:VsModel;share:int;floor:f64;rule_end:int;hold_to:int;ask_end:int;
    window:[2]int;rule_barge:int;pipe:VpConfig;}
// One conversation: the user's turn (chapter 184's words and ticks), when
// each word starts, whether it sounds complete and ends on a falling
// pitch, and the sound over the reply: what it is, how long, and each
// tick's bits (1 speech-like, 2 broadband, 4 louder than the reply's echo).
VsLine :: struct {turn:VpTurn;start:[16]int;complete:[16]bool;fall:[16]bool;event:VsEvent;dur:int;bits:[120]u8;}
```

The model is asked three kinds of question:

- **The end of the turn.** At `ask_end` (16) ticks into a silence: is the
  user done? The actions are `end` (end the turn now) and `hold` (let the
  rule wait until `hold_to`, 100 ticks, before it ends the turn).
- **An early start.** When the answer is `end`: start the language model on
  the partial transcript now, before the final transcript arrives?
- **Sound over the reply.** At 8 and 16 ticks into the sound: is it speech?
  The actions are `stop` (stop the reply) and, at 16 ticks only, `ignore`
  (overrule the counter for this sound).

Each answer becomes a proposal with the time it was asked, the time it
arrives, its deadline and its epoch. The log counts the questions, the
drops by reason and the answers that came on time.

<!-- source: psionic/lib/voice-steer.coax proposal -->
```jai
VsProposal :: struct {kind:VsKind;act:VsAct;p:f64;epoch:int;asked:int;arrives:int;deadline:int;}
// Questions asked, answers dropped by reason (kept counts the admitted),
// answers on time, and the ticks all answers waited.
VsLog :: struct {asked:int;drops:[7]int;on_time:int;wait:int;}
```

## The costs come first

Before anything runs, the consumer writes down what each mistake costs, in
one unit:

| Mistake | Cost |
| --- | --- |
| Ending the turn while the user is still talking (a cut-off) | 4 |
| Each tick of silence before the answer, or of talking over the user | 0.02 (2 a second) |
| A language-model run thrown away after an early start | 0.5 |
| Stopping the reply for a cough or a backchannel | 2 |
| Never stopping the reply for speech | 3 |

The configuration holds the costs, the stand-in model's timing (an answer
arrives 2 to 5 ticks after its question), the decision's share of the
latency budget (4 ticks), the gate's confidence floor and the rules'
timing. Its `pipe` is chapter 184's configuration; the end of the turn
replaces its `hangover`.

<!-- source: psionic/lib/voice-steer.coax config -->
```jai
vs_written :: ()->VsCosts {return VsCosts.{cut=4,tick=0.02,restart=0.5,false_stop=2,missed=3};}
vs_config :: ()->VsConfig {
    return VsConfig.{costs=vs_written(),model=VsModel.{on=true,latency=2,jitter=3,seed=1},share=4,floor=0.4,
        rule_end=70,hold_to=100,ask_end=16,window=[2]int.[8,16],rule_barge=24,
        pipe=VpConfig.{asr_lag=5,asr_final=4,quiet=0,prefill=6,per_word=2,step=3,cap=6,
            tts_first=8,word_ticks=25,pause=15,hop=[3]int.[2,3,2]}};
}
```

Each decision is a yes or no at a probability p. As in
[chapter 216](ch216-thresholds-from-written-costs.md), a wrong yes costs
`wrong_yes`, a wrong no costs `wrong_no`, and the answer is yes when
`(1 - p) wrong_yes < p wrong_no`, that is, when p is above
`wrong_yes / (wrong_yes + wrong_no)`. `vs_yes` calls chapter 216's
`th_answer` with asking a person priced out of reach.

<!-- source: psionic/lib/voice-steer.coax decide -->
```jai
// Yes when chapter 216's cheapest answer at p is yes: a wrong yes costs
// wrong_yes, a wrong no wrong_no, and asking a person is never cheaper.
vs_yes :: (p:f64,wrong_yes:f64,wrong_no:f64)->bool {
    return th_answer(ThCosts.{wrong_yes=wrong_yes,wrong_no=wrong_no,ask=TH_COST_MAX},p).action==ThAction.yes;
}
// At k ticks of silence: ending now is wrong if the user goes on; not
// ending is wrong by the ticks until the rule ends the turn. Holding is
// wrong by the extra ticks if the user is done, and not holding by a
// cut-off if the user goes on.
vs_end_act :: (c:VsConfig,p:f64,k:int)->VsAct {
    if vs_yes(p,c.costs.cut,f64(c.rule_end-k)*c.costs.tick) {return VsAct.end;}
    if vs_yes(1-p,f64(c.hold_to-c.rule_end)*c.costs.tick,c.costs.cut) {return VsAct.hold;}
    return VsAct.none;
}
// Starting on the partial transcript is wrong if more words come (the run
// is thrown away); waiting is wrong by the final transcript's delay.
vs_start_act :: (c:VsConfig,p:f64)->bool {
    return vs_yes(p,c.costs.restart,f64(c.pipe.asr_final+c.pipe.hop[0])*c.costs.tick);
}
// Stopping is wrong if the sound is not speech. At window 0, not stopping
// is wrong by the ticks until window 1; at window 1 it is a missed barge-in,
// and the answer is ignore: the voice engine's counter is overruled.
vs_barge_act :: (c:VsConfig,p:f64,which:int)->VsAct {
    if which==0 {
        if vs_yes(p,c.costs.false_stop,f64(c.window[1]-c.window[0])*c.costs.tick) {return VsAct.stop;}
        return VsAct.none;
    }
    if vs_yes(p,c.costs.false_stop,c.costs.missed) {return VsAct.stop;}
    return VsAct.ignore;
}
```

Worked by hand, the thresholds are:

- **End at 16 ticks.** Ending is wrong if the user goes on: 4. Not ending
  is wrong by the 70 - 16 = 54 ticks until the rule ends the turn:
  54 × 0.02 = 1.08. End when p > 4 / (4 + 1.08) = 0.7874.
- **Hold.** Holding is wrong by 30 extra ticks if the user is done:
  30 × 0.02 = 0.6. Not holding is a cut-off if the user goes on: 4. Hold
  when 1 - p > 0.6 / 4.6, that is, when p < 0.8696.
- **Start early.** Starting is wrong if more words come: 0.5. Waiting is
  wrong by the final transcript's delay, 4 + 2 = 6 ticks: 0.12. Start when
  p > 0.5 / 0.62 = 0.8065.
- **Stop at 8 ticks.** Stopping is wrong if the sound isn't speech: 2. Not
  stopping is wrong by the 8 ticks until the next question: 0.16. Stop when
  p > 2 / 2.16 = 0.9259.
- **Stop at 16 ticks.** Not stopping is a missed barge-in: 3. Stop when
  p > 2 / 5 = 0.4; otherwise ignore.

The first check asserts each threshold from both sides.

<!-- source: psionic/lib/voice-steer.coax check-costs -->
```jai
#test "the written costs give these thresholds" {
    c:=vs_config();
    // End at 16 ticks of silence: yes when (1 - p) 4 < p (70 - 16) 0.02, p > 4 / 5.08.
    #assert !vs_yes(4/5.08,c.costs.cut,54*0.02) && vs_yes(4/5.08+1e-9,c.costs.cut,54*0.02);
    #assert vs_end_act(c,0.787,16)==VsAct.hold && vs_end_act(c,0.788,16)==VsAct.end;
    // Hold: (1 - p) 4 > p 30 * 0.02, so hold while p < 4 / 4.6 = 0.8696.
    c.ask_end=60;
    #assert vs_end_act(c,0.869,60)==VsAct.hold && vs_end_act(c,0.8696,60)==VsAct.none && vs_end_act(c,0.9523,60)==VsAct.none;
    #assert vs_end_act(c,0.9524,60)==VsAct.end;
    // Start early: p 6 ticks 0.02 > (1 - p) 0.5, p > 0.5 / 0.62 = 0.8065.
    #assert !vs_start_act(c,0.8064) && vs_start_act(c,0.8065);
    // Stop at window 0: p 8 ticks 0.02 > (1 - p) 2, p > 2 / 2.16 = 0.9259.
    #assert vs_barge_act(c,0.9259,0)==VsAct.none && vs_barge_act(c,0.926,0)==VsAct.stop;
    // Stop at window 1: p 3 > (1 - p) 2, p > 0.4; otherwise ignore.
    #assert vs_barge_act(c,0.3999,1)==VsAct.ignore && vs_barge_act(c,0.4001,1)==VsAct.stop;
}
```

## The stand-in model

The model here isn't trained. It is the exact posterior of the generator
that draws this chapter's conversations, so its probabilities are
calibrated by construction and every one can be checked by hand.

A silence that reaches 16 ticks follows either the last word or a
hesitation; normal gaps between words last 5 to 15 ticks. Four
conversations in 10 have one hesitation, so the odds are 10 to 4. The last
word sounds complete 900 times in 1000 and falls 800 times; a hesitation's
word 150 and 300 times. For a complete word with a falling pitch:

```text
a = 10 × 900 × 800 = 7,200,000      (the turn is over)
b =  4 × 150 × 300 =   180,000      (a hesitation)
p = a / (a + b) = 7,200,000 / 7,380,000 = 0.9756
```

The four cases give 0.9756 and 0.8108 (complete), and 0.4396 and 0.0775 (not
complete). At 16 ticks the first two clear 0.7874 and end the turn; the last
two are under 0.8696 and hold it.

Sound over the reply is a cough (3 in 10), a backchannel such as "mm-hm"
(3 in 10) or speech (4 in 10). Each tick's bits are drawn independently at
fixed rates per mille: a cough is speech-like 600, broadband 800 and loud
900; a backchannel 900, 100 and 600; speech 900, 100 and 900. Coughs last 10
to 40 ticks, backchannels 15 to 40 and speech 60 to 120. The weight of each
kind is its prior, times the chance it is still going, times the chance of
each tick's bits.

<!-- source: psionic/lib/voice-steer.coax model -->
```jai
// The stand-in model is the exact posterior of the generator (vs_lines).
// A silence of 16 ticks follows the last word or a hesitation, 10 to 4;
// the last word sounds complete 900 times in 1000 and falls 800; a
// hesitation's word 150 and 300. p: the turn is over.
vs_turn_p :: (complete:bool,fall:bool)->f64 {
    a:=10*(ifx complete then 900 else 100)*(ifx fall then 800 else 200);
    b:=4*(ifx complete then 150 else 850)*(ifx fall then 300 else 700);
    return f64(a)/f64(a+b);
}
// Sound over the reply: a cough (3 in 10), a backchannel (3) or speech (4).
// Per tick, per mille: speech-like, broadband, loud.
vs_rate :: (e:int,f:int)->int {
    r:=[9]int.[600,800,900,900,100,600,900,100,900];
    return r[3*e+f];
}
// The chance the sound is still going after w ticks: coughs last 10 to 40
// ticks, backchannels 15 to 40, speech 60 to 120.
vs_still :: (e:VsEvent,w:int)->f64 {
    lo:=60;hi:=120;
    if e==VsEvent.cough {lo=10;hi=40;} else if e==VsEvent.backchannel {lo=15;hi=40;}
    n:=0;for d:lo..<hi+1 {if d>w {n+=1;}}
    return f64(n)/f64(hi-lo+1);
}
// Unnormalized weights of each kind of sound, given its first w ticks.
vs_barge_post :: (bits:[120]u8,w:int)->[3]f64 {
    q:[3]f64;
    for e:0..<3 {
        v:=f64(ifx e==2 then 4 else 3)*vs_still(cast(VsEvent)cast(u8)e,w);
        for t:0..<w {for f:0..<3 {
            r:=vs_rate(e,f);if ((cast(int)bits[t])>>f)&1==0 {r=1000-r;}
            v*=f64(r)/1000;
        }}
        q[e]=v;
    }
    return q;
}
vs_barge_p :: (bits:[120]u8,w:int)->f64 {q:=vs_barge_post(bits,w);return q[2]/(q[0]+q[1]+q[2]);}
// The most probable kind; a tie keeps the earlier.
vs_class :: (q:[3]f64)->VsEvent {
    k:=0;if q[1]>q[k] {k=1;}
    if q[2]>q[k] {k=2;}
    return cast(VsEvent)cast(u8)k;
}
```

For 8 ticks of loud, speech-like, narrow-band sound (bits 5), each tick has
chance 0.6 × 0.2 × 0.9 = 0.108 under a cough, 0.486 under a backchannel and
0.729 under speech:

```text
cough       3 × 0.108^8 = 0.0000000555
backchannel 3 × 0.486^8 = 0.009338
speech      4 × 0.729^8 = 0.319064
p = 0.319064 / 0.328402 = 0.9716
```

That is above 0.9259, so the model says stop at 8 ticks.

<!-- source: psionic/lib/voice-steer.coax check-model -->
```jai
#test "the model's probabilities by hand" {
    #assert vs_turn_p(true,true)==7200000.0/7380000 && vs_turn_p(true,false)==1800000.0/2220000;
    #assert vs_turn_p(false,true)==800000.0/1820000 && vs_turn_p(false,false)==200000.0/2580000;
    b:[120]u8;for k:0..<16 {b[k]=5;}
    q:=vs_barge_post(b,8);
    #assert abs(q[0]-3*pow(0.108,8))<1e-15 && abs(q[1]-3*pow(0.486,8))<1e-15 && abs(q[2]-4*pow(0.729,8))<1e-15;
    #assert abs(vs_barge_p(b,8)-0.971569)<1e-6;
    // At 16 ticks a cough has lasted past 16 with 24 chances in 31, a backchannel 24 in 26.
    b[0]=3;q=vs_barge_post(b,16);
    #assert abs(q[0]-3*24.0/31*pow(0.108,15)*0.6*0.8*0.1)<1e-18 && abs(q[1]-3*24.0/26*pow(0.486,15)*0.9*0.1*0.4)<1e-12;
    #assert abs(q[2]-4*pow(0.729,15)*0.9*0.1*0.1)<1e-12;
    // Before 9 ticks no cough has ended, before 14 no backchannel.
    #assert vs_still(VsEvent.cough,8)==1 && vs_still(VsEvent.cough,9)==1 && vs_still(VsEvent.cough,10)==30.0/31;
    #assert vs_still(VsEvent.backchannel,14)==1 && vs_still(VsEvent.backchannel,15)==25.0/26 && vs_still(VsEvent.backchannel,40)==0;
    #assert vs_still(VsEvent.speech,59)==1 && vs_still(VsEvent.speech,60)==60.0/61 && vs_still(VsEvent.cough,39)==1.0/31;
    #assert vs_class(vs_barge_post(b,8))==VsEvent.speech;
    for k:0..<16 {b[k]=7;}#assert vs_class(vs_barge_post(b,8))==VsEvent.cough;
    for k:0..<16 {b[k]=1;}#assert vs_class(vs_barge_post(b,8))==VsEvent.backchannel;
    q[0]=1;q[1]=1;q[2]=1;#assert vs_class(q)==VsEvent.cough;q[1]=2;q[2]=2;#assert vs_class(q)==VsEvent.backchannel;
    q[0]=2;q[1]=3;q[2]=1;#assert vs_class(q)==VsEvent.backchannel;
}
```

## The gate

An answer reaches the engine some ticks after its question. The gate checks
it in the order `decide` in `verse/voice.coax` uses: the action must fit the
question, the probability must lie in [0, 1], the confidence must clear the
floor, the epoch must still be current, and the deadline must not have
passed. The confidence is the probability of what the action bets on: p for
`end`, `start` and `stop`, and 1 - p for `hold` and `ignore`. The floor is
0.4, the lowest threshold the written costs produce, so the gate never
overrules a cost-derived action that the model is entitled to take.

<!-- source: psionic/lib/voice-steer.coax gate -->
```jai
vs_fits :: (k:VsKind,a:VsAct)->bool {
    if k==VsKind.turn_end {return a==VsAct.end || a==VsAct.hold;}
    if k==VsKind.early_start {return a==VsAct.start;}
    return a==VsAct.stop || a==VsAct.ignore;
}
// The voice engine's order: shape, range, confidence floor, epoch, deadline.
// The confidence is the probability of what the action bets on.
vs_gate :: (c:VsConfig,d:VsProposal,epoch:int)->VsDrop {
    if !vs_fits(d.kind,d.act) {return VsDrop.shape;}
    if !tb_in(d.p,0,1) {return VsDrop.clamp;}
    conf:=ifx d.act==VsAct.hold || d.act==VsAct.ignore then 1-d.p else d.p;
    if conf<c.floor {return VsDrop.floor;}
    if d.epoch!=epoch {return VsDrop.epoch;}
    if d.arrives>d.deadline {return VsDrop.late;}
    return VsDrop.kept;
}
// The question is sent and the audio goes on; the answer arrives some
// ticks later. call numbers the question so its delay is reproducible.
vs_propose :: (c:VsConfig,lg:[]VsLog,k:VsKind,a:VsAct,p:f64,epoch:int,asked:int,call:int)->VsProposal {
    lg[0].asked+=1;
    w:=c.model.latency+cast(int)(tb_bits(c.model.seed,cast(u64)call)%cast(u64)(c.model.jitter+1));
    return VsProposal.{kind=k,act=a,p=p,epoch=epoch,asked=asked,arrives=asked+w,deadline=asked+c.share};
}
// At the answer's arrival, with the epoch then current: kept or dropped,
// and why. A stopped model's answer never arrives.
vs_admit :: (c:VsConfig,lg:[]VsLog,d:VsProposal,epoch:int)->bool {
    if !c.model.on {lg[0].drops[cast(int)VsDrop.absent]+=1;return false;}
    lg[0].wait+=d.arrives-d.asked;
    if d.arrives<=d.deadline {lg[0].on_time+=1;}
    g:=vs_gate(c,d,epoch);lg[0].drops[cast(int)g]+=1;
    return g==VsDrop.kept;
}
```

The deadline is the question's tick plus the decision's share. The epoch
for the turn is the last word started: an answer about a silence that
arrives after the user spoke again is stale. The epoch for sound over the
reply changes when the counter stops the reply.

<!-- source: psionic/lib/voice-steer.coax check-gate -->
```jai
#test "the gate drops each kind of bad proposal, in the voice engine's order" {
    c:=vs_config();
    d:=VsProposal.{kind=VsKind.turn_end,act=VsAct.end,p=0.9,epoch=3,asked=100,arrives=104,deadline=104};
    #assert vs_gate(c,d,3)==VsDrop.kept;
    e:=d;e.act=VsAct.stop;#assert vs_gate(c,e,3)==VsDrop.shape;
    e=d;e.p=1.0000001;#assert vs_gate(c,e,3)==VsDrop.clamp;
    e=d;e.p= -0.0000001;#assert vs_gate(c,e,3)==VsDrop.clamp;
    z:=0.0;e=d;e.p=z/z;#assert vs_gate(c,e,3)==VsDrop.clamp;
    e=d;e.p=0.3999;#assert vs_gate(c,e,3)==VsDrop.floor;
    e=d;e.p=0.4;#assert vs_gate(c,e,3)==VsDrop.kept;
    e=d;e.act=VsAct.hold;e.p=0.6001;#assert vs_gate(c,e,3)==VsDrop.floor;
    e.p=0.6;#assert vs_gate(c,e,3)==VsDrop.kept;
    #assert vs_gate(c,d,4)==VsDrop.epoch;
    e=d;e.arrives=105;#assert vs_gate(c,e,3)==VsDrop.late;
    // The first failing check names the drop.
    e.epoch=2;e.p=0.1;#assert vs_gate(c,e,3)==VsDrop.floor;
    e.p=0.9;#assert vs_gate(c,e,3)==VsDrop.epoch;
    for k:0..<3 {for a:0..<6 {
        f:=d;f.kind=cast(VsKind)cast(u8)k;f.act=cast(VsAct)cast(u8)a;
        fits:=(k==0 && (a==1 || a==2)) || (k==1 && a==3) || (k==2 && (a==4 || a==5));
        #assert (vs_gate(c,f,3)!=VsDrop.shape)==fits;
    }}
    // A stopped model answers nothing: every question is logged absent.
    lg:=[1]VsLog.[VsLog.{}];m:=c;m.model.on=false;
    p:=vs_propose(m,lg[..],VsKind.barge,VsAct.stop,0.99,1,8,0);
    #assert !vs_admit(m,lg[..],p,1) && lg[0].asked==1 && lg[0].drops[1]==1 && lg[0].on_time==0 && lg[0].wait==0;
    // An answer at its deadline is on time; one tick later it is late.
    m=c;m.model.latency=4;m.model.jitter=0;
    p=vs_propose(m,lg[..],VsKind.barge,VsAct.stop,0.99,1,8,0);#assert p.arrives==12 && p.deadline==12;
    #assert vs_admit(m,lg[..],p,1) && lg[0].on_time==1 && lg[0].wait==4 && lg[0].drops[0]==1;
    p.arrives=13;#assert !vs_admit(m,lg[..],p,1) && lg[0].on_time==1 && lg[0].wait==9 && lg[0].drops[6]==1;
}
```

## The budget is checked before anything runs

`vs_check` refuses a configuration whose costs, timing or floor are broken.
It also refuses one whose decision can't fit inside its share: a declared
latency over the share, or a share so long that an answer on time could
arrive after the rule it is meant to steer. A configuration that passes has
answers that are either on time, and so early enough to matter, or dropped.

<!-- source: psionic/lib/voice-steer.coax validate -->
```jai
vs_cost_ok :: (x:f64)->bool {return x>0 && x<=TH_COST_MAX;}
vs_check :: (c:VsConfig)->VsWhy #export_c {
    k:=c.costs;
    all:=[5]f64.[k.cut,k.tick,k.restart,k.false_stop,k.missed];
    for x:all {if !vs_cost_ok(x) {return VsWhy.costs;}}
    if c.ask_end<1 || c.rule_end<=c.ask_end || c.hold_to<=c.rule_end || c.hold_to>VP_DELAY {return VsWhy.timing;}
    if c.window[0]<1 || c.window[1]<=c.window[0] || c.rule_barge<=c.window[1] || c.rule_barge>40 {return VsWhy.timing;}
    if c.model.latency<0 || c.model.jitter<0 || c.model.jitter>100 {return VsWhy.timing;}
    // The decision fits inside its share, and an answer on time arrives before either rule acts.
    if c.model.latency>c.share || c.ask_end+c.share>=c.rule_end || c.window[1]+c.share>=c.rule_barge {return VsWhy.budget;}
    if !tb_in(c.floor,0,1) {return VsWhy.floor;}
    // The earliest end of a turn is ask_end ticks after its last word.
    u:VpTurn;u.n=1;u.word[0]=999;
    if vp_check(vs_pipe(c),u)!=VpWhy.none {return VsWhy.pipe;}
    return VsWhy.none;
}
// Chapter 184's configuration with the earliest end of a turn the model can choose.
vs_pipe :: (c:VsConfig)->VpConfig {p:=c.pipe;p.hangover=c.ask_end;return p;}
vs_line_check :: (c:VsConfig,x:VsLine)->VsWhy {
    u:=x.turn;
    if vp_check(vs_pipe(c),u)!=VpWhy.none || u.end!=u.heard[u.n-1] || x.start[0]<0 {return VsWhy.line;}
    for w:0..<u.n {
        if x.start[w]>=u.heard[w] {return VsWhy.line;}
        if w>0 && x.start[w]<=u.heard[w-1] {return VsWhy.line;}
    }
    if x.dur<1 || x.dur>VS_FRAMES {return VsWhy.line;}
    for b:x.bits {if cast(int)b>7 {return VsWhy.line;}}
    return VsWhy.none;
}
```

## Steering the turn and the reply

`vs_turn` walks the user's silences. At 16 ticks into each one it asks the
model, sends the answer through the gate when it arrives, and ends the turn
on an admitted `end`, or at the rule's 70 ticks (100 after an admitted
`hold`). A turn that ends before the last word is a cut-off.

<!-- source: psionic/lib/voice-steer.coax turn -->
```jai
// The turn, silence by silence. At ask_end ticks into a silence the model
// is asked; the epoch is the last word started, so speech resuming before
// the answer makes it stale. The rule ends the turn at rule_end ticks, or
// hold_to when a hold was admitted; an admitted end ends it when it arrives.
vs_epoch :: (x:VsLine,t:int)->int {
    e:=0;for w:1..<x.turn.n {if x.start[w]<=t {e=w;}}
    return e;
}
vs_turn :: (c:VsConfig,x:VsLine,id:int,o:[]VsOutcome) {
    u:=x.turn;lg:=[1]VsLog.[o[0].log];
    for i:0..<u.n {
        last:=i==u.n-1;rule:=c.rule_end;ended:=false;
        if last || c.ask_end<x.start[i+1]-u.heard[i] {
            p:=vs_turn_p(x.complete[i],x.fall[i]);a:=vs_end_act(c,p,c.ask_end);
            if a!=VsAct.none {
                d:=vs_propose(c,lg[..],VsKind.turn_end,a,p,i,u.heard[i]+c.ask_end,id*64+2*i);
                if vs_admit(c,lg[..],d,vs_epoch(x,d.arrives)) {
                    if a==VsAct.end {ended=true;o[0].eot=d.arrives;} else {rule=c.hold_to;o[0].hold=true;}
                }
            }
            if a==VsAct.end && vs_start_act(c,p) && !o[0].early {
                d:=vs_propose(c,lg[..],VsKind.early_start,VsAct.start,p,i,u.heard[i]+c.ask_end,id*64+2*i+1);
                if vs_admit(c,lg[..],d,vs_epoch(x,d.arrives)) {o[0].early=true;o[0].start=d.arrives;}
            }
        }
        if !ended && (last || rule<x.start[i+1]-u.heard[i]) {ended=true;o[0].eot=u.heard[i]+rule;}
        if ended {o[0].cut=!last;break;}
    }
    o[0].log=lg[0];
}
```

`vs_barge` runs the voice engine's counter over the sound and asks the
model at each window until it says stop.

<!-- source: psionic/lib/voice-steer.coax barge -->
```jai
// Sound over the reply. The voice engine's counter rises on a loud
// speech-like tick and falls on any other; at rule_barge it stops the
// reply. The model is asked at each window while the sound lasts, until it
// says stop. The reply is the epoch: an answer that arrives after the
// counter stopped it is stale: the epoch becomes the tick it stopped. An
// admitted stop stops the reply; an admitted ignore overrules the counter.
// The tick the counter stops the reply, or 0 if it never does.
vs_counter :: (c:VsConfig,x:VsLine)->int {
    run:=0;
    for t:0..<x.dur {
        if (cast(int)x.bits[t])&5==5 {run+=1;} else if run>0 {run-=1;}
        if run==c.rule_barge {return t+1;}
    }
    return 0;
}
vs_barge :: (c:VsConfig,x:VsLine,id:int,o:[]VsOutcome) {
    rule:=vs_counter(c,x);fired:=rule!=0;
    lg:=[1]VsLog.[o[0].log];veto:=false;said:=false;
    for which:0..<2 {
        w:=c.window[which];
        if said || x.dur<=w {break;}
        p:=vs_barge_p(x.bits,w);a:=vs_barge_act(c,p,which);
        said=a==VsAct.stop;
        if a!=VsAct.none {
            d:=vs_propose(c,lg[..],VsKind.barge,a,p,0,w,id*64+40+which);
            now:=ifx fired && rule<=d.arrives then rule else 0;
            if vs_admit(c,lg[..],d,now) {
                if a==VsAct.stop {o[0].stopped=true;o[0].stop=d.arrives;} else {veto=true;}
            }
        }
    }
    // An answer on time arrives before the counter can fire.
    if !o[0].stopped && !veto && fired {o[0].stopped=true;o[0].stop=rule;}
    o[0].log=lg[0];
}
```

`vs_line` puts them together. The end of the turn becomes chapter 184's
`hangover`, an admitted early start becomes chapter 184's `vp_start` at
that tick, and chapter 184's `vp_latency` gives the time to the first audio.
The cost is the written price of what happened.

<!-- source: psionic/lib/voice-steer.coax line -->
```jai
// One line: the turn, then chapter 184's pipeline from the end the policy
// chose (an early start is chapter 184's vp_start at that tick), then the
// sound over the reply. The cost is the written price of what happened.
vs_line :: (c:VsConfig,x:VsLine,id:int)->VsOutcome {
    o:=[1]VsOutcome.[VsOutcome.{why=vs_check(c)}];
    if o[0].why!=VsWhy.none {return o[0];}
    o[0].why=vs_line_check(c,x);if o[0].why!=VsWhy.none {return o[0];}
    k:=c.costs;
    vs_turn(c,x,id,o[..]);
    if o[0].cut {o[0].cost=k.cut;}
    else {
        p:=c.pipe;p.hangover=o[0].eot-x.turn.end;
        r:=[1]VpRun.[vp_open(p,x.turn)];
        if o[0].early {vp_until(r[..],o[0].start);if !r[0].started {vp_start(r[..],r[0].seen,true);}}
        o[0].done=vp_finish(r[..]).at;
        o[0].first=vp_latency(r[..]).first;o[0].wasted=r[0].wasted;
        o[0].cost=f64(o[0].first)*k.tick;if o[0].wasted!=0 {o[0].cost+=k.restart;}
    }
    vs_barge(c,x,id,o[..]);
    if x.event==VsEvent.speech {o[0].cost+=ifx o[0].stopped then f64(o[0].stop)*k.tick else k.missed;}
    else if o[0].stopped {o[0].cost+=k.false_stop;}
    return o[0];
}
```

## One conversation by hand

The user says three words whose audio ends at ticks 62, 70 and 80, with gaps
of 1 and 2 ticks. The last word sounds complete and falls. The model answers
2 ticks after each question.

- **Fixed rules.** The turn ends at 80 + 70 = 150. Chapter 184's final
  transcript reaches the language model at 156, its first sentence is made
  by 183, taken at 186, played from 194 and heard at 196: 116 ticks after the
  user stopped.
- **Model.** Asked at 80 + 16 = 96, p = 0.9756: `end` and `start`, both
  arriving at 98, inside the 4-tick share. The language model starts at 98
  on all three partial words and makes its first token at 98 + 6 + 2 × 3 +
  3 = 113 and the sentence's period at 125. The speaker takes it at 128 and
  plays it from 136, and it is heard at 138: 58 ticks after the user stopped.
- **Sound over the reply.** The user then talks over the reply: loud,
  speech-like, narrow-band ticks. The counter reaches 24 at tick 24. The
  model says stop at 8 ticks (p = 0.9716) and the answer arrives at 10.

The cost under the rules is (116 + 24) × 0.02 = 2.8; under the model
(58 + 10) × 0.02 = 1.36.

<!-- source: psionic/lib/voice-steer.coax check-hand -->
```jai
#test "the hand trace: a complete sentence, then speech over the reply" {
    c:=vs_config();c.model.jitter=0;x:=vsx_line();
    o:=vs_line(c,x,0);
    // Asked at 80 + 16 = 96, p = 0.9756: end and start early, both at 98.
    #assert o.why==VsWhy.none && !o.cut && o.eot==98 && o.early && o.start==98 && o.first==58 && o.done==433 && o.wasted==0;
    // Speech over the reply: p = 0.9716 at 8 ticks, stopped at 10.
    #assert o.stopped && o.stop==10 && o.log.asked==3 && o.log.drops[0]==3 && o.log.on_time==3 && o.log.wait==6;
    #assert abs(o.cost-(58+10)*0.02)<1e-12;
    m:=c;m.model.on=false;r:=vs_line(m,x,0);
    #assert !r.cut && r.eot==150 && !r.early && r.first==116 && r.done==491 && r.stop==24 && r.log.asked==3 && r.log.drops[1]==3;
    #assert abs(r.cost-(116+24)*0.02)<1e-12;
}
```

## Drawn conversations

`vs_lines` draws conversations from a seed: 2 to 7 words, and in 4
conversations of 10 one hesitation of 75 to 95 ticks, longer than the rule's
70. `vs_score` runs each and adds up the outcomes.

<!-- source: psionic/lib/voice-steer.coax set -->
```jai
// A set of lines drawn from a seed: 2 to 7 words of 20 to 40 ticks with
// gaps of 5 to 15, and in 4 lines of 10 one hesitation of 75 to 95 ticks;
// then one sound over the reply.
vs_count_ok :: (n:int)->bool {return n>=1 && n<=VS_LINES;}
vs_draw :: (seed:u64,i:int,k:int,m:int)->int {return cast(int)(tb_bits(seed,cast(u64)(i*1024+k))%cast(u64)m);}
vs_lines :: (seed:u64,out:[]VsLine)->VsWhy #export_c {
    if !vs_count_ok(out.count) {return VsWhy.size;}
    for i:0..<out.count {
        x:VsLine;u:VpTurn;
        u.n=2+vs_draw(seed,i,0,6);t:=20+vs_draw(seed,i,1,30);
        hes:= -1;if vs_draw(seed,i,2,10)<4 {hes=vs_draw(seed,i,3,u.n-1);}
        for w:0..<u.n {
            x.start[w]=t;u.heard[w]=t+20+vs_draw(seed,i,10+w,21);u.word[w]=1+vs_draw(seed,i,30+w,999);
            // In twentieths: 18 and 16 for the last word, 3 and 6 for a hesitation's.
            pc:=10;pf:=10;gap:=5+vs_draw(seed,i,90+w,11);
            if w==u.n-1 {pc=18;pf=16;} else if w==hes {pc=3;pf=6;gap=75+vs_draw(seed,i,90+w,21);}
            x.complete[w]=vs_draw(seed,i,50+w,20)<pc;x.fall[w]=vs_draw(seed,i,70+w,20)<pf;
            t=u.heard[w]+gap;
        }
        u.end=u.heard[u.n-1];x.turn=u;
        e:=vs_draw(seed,i,120,10);ev:=ifx e<3 then 0 else ifx e<6 then 1 else 2;
        lo:=[3]int.[10,15,60];span:=[3]int.[31,26,61];
        x.event=cast(VsEvent)cast(u8)ev;x.dur=lo[ev]+vs_draw(seed,i,121,span[ev]);
        for f:0..<x.dur {
            b:=0;
            for q:0..<3 {if vs_draw(seed,i,200+3*f+q,1000)<vs_rate(ev,q) {b+=1<<q;}}
            x.bits[f]=cast(u8)b;
        }
        out[i]=x;
    }
    return VsWhy.none;
}
// Every line, its cost into cost, and the totals.
vs_score :: (c:VsConfig,lines:[]VsLine,cost:[]f64)->VsScore #export_c {
    if !vs_count_ok(lines.count) || cost.count!=lines.count {return VsScore.{why=VsWhy.size};}
    s:=VsScore.{why=vs_check(c),lines=lines.count};if s.why!=VsWhy.none {return s;}
    for x:lines {if vs_line_check(c,x)!=VsWhy.none {return VsScore.{why=VsWhy.line};}}
    for x,i:lines {
        o:=vs_line(c,x,i);cost[i]=o.cost;s.cost+=o.cost;
        if o.cut {s.cut+=1;} else {s.first_sum+=o.first;s.first_n+=1;}
        if o.early {s.early+=1;}
        s.wasted+=o.wasted;
        if x.event==VsEvent.speech {if o.stopped {s.react_sum+=o.stop;s.react_n+=1;} else {s.missed+=1;}}
        else if o.stopped {s.false_stop+=1;}
        s.log.asked+=o.log.asked;s.log.on_time+=o.log.on_time;s.log.wait+=o.log.wait;
        for k:0..<7 {s.log.drops[k]+=o.log.drops[k];}
    }
    return s;
}
```

The example, [`voice-steer.coax`](../examples/voice-steer.coax), draws 400
conversations, runs them under the fixed rules (the
model stopped), the model, and a model whose answers arrive 4 to 10 ticks
after each question, and gives chapter 241's verdict on the per-conversation
cost. The floor for the verdict is the model's own spread over four seeds of
answer delays.

<!-- source: psionic/examples/voice-steer.coax main -->
```jai
main :: () {
    c:=vs_config();k:=c.costs;
    print("%: costs cut-off %, a tick of silence or of talking over %, a run thrown away %, a false stop %, a missed barge-in %\n",
        VS_PROFILE,k.cut,k.tick,k.restart,k.false_stop,k.missed);
    print("thresholds: end at % ticks of silence above %, hold below %, start early above %, stop at % ticks above %, at % above %\n",
        c.ask_end,r4(k.cut/(k.cut+f64(c.rule_end-c.ask_end)*k.tick)),r4(1-f64(c.hold_to-c.rule_end)*k.tick/(f64(c.hold_to-c.rule_end)*k.tick+k.cut)),
        r4(k.restart/(k.restart+6*k.tick)),c.window[0],r4(k.false_stop/(k.false_stop+f64(c.window[1]-c.window[0])*k.tick)),
        c.window[1],r4(k.false_stop/(k.false_stop+k.missed)));
    for q:0..<4 {
        complete:=q<2;fall:=q%2==0;p:=vs_turn_p(complete,fall);
        print("  last word complete %, falling %: p = % -> %\n",complete,fall,r4(p),ACT[cast(int)vs_end_act(c,p,c.ask_end)]);
    }

    print("one conversation, answers 2 ticks after each question:\n");
    h:=c;h.model.jitter=0;x:=vsx_line();m:=h;m.model.on=false;
    show("fixed rules",vs_line(m,x,0));show("model",vs_line(h,x,0));
    x=vsx_pause(x,1,80,false,false);
    print("the same words with an 80-tick hesitation after word 1:\n");
    show("fixed rules",vs_line(m,x,0));show("model",vs_line(h,x,0));

    xs:[400]VsLine;#assert vs_lines(2026,xs[..])==VsWhy.none;
    kinds:=[3]int.[0,0,0];for y:xs {kinds[cast(int)y.event]+=1;}speech:=kinds[2];
    print("400 drawn conversations; over the reply, % coughs, % backchannels and % times speech\n",kinds[0],kinds[1],speech);
    off:=c;off.model.on=false;late:=c;late.model.latency=4;late.model.jitter=6;
    base:[400]f64;runs:[1600]f64;cl:[400]f64;
    s:=vs_score(off,xs[..],base[..]);show_score("fixed rules (the model stopped)",s,speech);
    for r:0..<4 {
        e:=c;e.model.seed=cast(u64)(1+r);t:=vs_score(e,xs[..],cl[..]);
        for i:0..<400 {runs[r*400+i]= -cl[i];}
        if r==0 {show_score("model",t,speech);}
    }
    t:=vs_score(late,xs[..],cl[..]);show_score("model answering 4 to 10 ticks after each question",t,speech);

    // How the model names sound that lasts past its second question.
    n:[3][3]int;
    for y:xs {if y.dur>c.window[1] {n[cast(int)y.event][cast(int)vs_class(vs_barge_post(y.bits,c.window[1]))]+=1;}}
    print("sound lasting past % ticks, named by the model:\n",c.window[1]);
    for e:0..<3 {print("  %: cough %, backchannel %, speech %\n",EVENT[e],n[e][0],n[e][1],n[e][2]);}

    // Higher is better: the negated written cost of each conversation.
    for i:0..<400 {base[i]= -base[i];cl[i]= -cl[i];}
    zero:[400]int;f:=ns_floor(runs[..],zero[..],0,4,1);#assert f.status==NsStatus.ok;
    print("chapter 241's decision gate, the model's own seed-to-seed spread as the floor:\n");
    show_verdict("model against the fixed rules",base[..],runs[0..<400],f);
    show_verdict("late model against the fixed rules",base[..],cl[..],f);
}
```

```text
vs-steer-v1: costs cut-off 4.0, a tick of silence or of talking over 0.02, a run thrown away 0.5, a false stop 2.0, a missed barge-in 3.0
thresholds: end at 16 ticks of silence above 0.7874, hold below 0.8696, start early above 0.8065, stop at 8 ticks above 0.9259, at 16 above 0.4
  last word complete true, falling true: p = 0.9756 -> end
  last word complete true, falling false: p = 0.8108 -> end
  last word complete false, falling true: p = 0.4396 -> hold
  last word complete false, falling false: p = 0.0775 -> hold
one conversation, answers 2 ticks after each question:
  fixed rules: turn over at 150, first audio 116 ticks after the user stopped; reply stopped 24 ticks into the overlap; cost 2.8
  model: turn over at 98, first audio 58 ticks after the user stopped, language model started early at 98; reply stopped 10 ticks into the overlap; cost 1.36
the same words with an 80-tick hesitation after word 1:
  fixed rules: turn over at 140, cutting the user off; reply stopped 24 ticks into the overlap; cost 4.48
  model: turn over at 176, first audio 58 ticks after the user stopped, held, language model started early at 176; reply stopped 10 ticks into the overlap; cost 1.36
400 drawn conversations; over the reply, 129 coughs, 116 backchannels and 155 times speech
fixed rules (the model stopped):
  cut off 172 of 400; first audio 116.0833 ticks on average over 228 turns; 0 early starts, 0 tokens thrown away
  stopped for a cough or backchannel 0 of 245, missed speech 1 of 155, stopped for speech after 38.8312 ticks on average
  questions 1000: absent 1000
  cost per conversation 3.3499
model:
  cut off 60 of 400; first audio 73.2559 ticks on average over 340 turns; 262 early starts, 0 tokens thrown away
  stopped for a cough or backchannel 18 of 245, missed speech 0 of 155, stopped for speech after 19.8194 ticks on average
  questions 1215: kept 933 late 282; on time 933 of 1215 (0.7679), 3.4617 ticks after the question on average
  cost per conversation 2.089
model answering 4 to 10 ticks after each question:
  cut off 142 of 400; first audio 106.0659 ticks on average over 258 turns; 45 early starts, 0 tokens thrown away
  stopped for a cough or backchannel 2 of 245, missed speech 2 of 155, stopped for speech after 35.3856 ticks on average
  questions 1057: kept 168 late 889; on time 168 of 1057 (0.1589), 7.0132 ticks after the question on average
  cost per conversation 3.084
sound lasting past 16 ticks, named by the model:
  cough: cough 97, backchannel 0, speech 0
  backchannel: cough 0, backchannel 96, speech 12
  speech: cough 0, backchannel 10, speech 145
chapter 241's decision gate, the model's own seed-to-seed spread as the floor:
  model against the fixed rules: gain 1.2609 per conversation, floor 0.1155, 0 of 1000 replicates at or below it -> passed (beyond the floor)
  late model against the fixed rules: gain 0.2659 per conversation, floor 0.1155, 0 of 1000 replicates at or below it -> passed (beyond the floor)
```

The model cuts off 60 users where the rule cuts off 172, and on the turns
each did not cut off, the first audio comes 73 ticks after the user stopped
instead of 116. It stops the reply for speech after 20
ticks instead of 39, and in exchange stops for a cough or a backchannel 18
times where the counter never does. The costs say that trade is worth it.
About 1 answer in 4 arrives after the 4-tick share and is dropped, and the
rule acts on that question as it would have without the model. With answers
4 to 10 ticks after each question, 84 percent are dropped and the model
helps only a little. The checks confirm that a stopped model gives exactly the rule's
outcome on every conversation.

Both verdicts pass because both gains are far beyond the floor. The floor
measures only the spread of the answer delays; the conversations are drawn
from the model's own generator, so the verdict says nothing about real
speech.

The frozen numbers are checked:

<!-- source: psionic/lib/voice-steer.coax check-set -->
```jai
#test "the drawn set's frozen numbers" #steps 400000000 {
    xs:[400]VsLine;#assert vs_lines(2026,xs[..])==VsWhy.none;costs:[400]f64;
    c:=vs_config();c.model.on=false;r:=vs_score(c,xs[..],costs[..]);
    #assert r.cut==172 && r.first_sum==26467 && r.first_n==228 && r.early==0 && r.false_stop==0 && r.missed==1;
    #assert r.react_sum==5980 && r.react_n==154 && r.log.asked==1000 && r.log.drops[1]==1000 && abs(r.cost-1339.94)<1e-9;
    m:=vs_score(vs_config(),xs[..],costs[..]);
    #assert m.cut==60 && m.first_sum==24907 && m.first_n==340 && m.early==262 && m.wasted==0 && m.false_stop==18 && m.missed==0;
    #assert m.react_sum==3072 && m.react_n==155 && m.log.asked==1215 && m.log.on_time==933 && m.log.wait==4206;
    #assert m.log.drops[0]==933 && m.log.drops[6]==282 && abs(m.cost-835.58)<1e-9;
}
```

## What is refused

The second example, [`voice-steer-refuse.coax`](../examples/voice-steer-refuse.coax),
breaks one thing at a time. Each broken configuration or conversation is
refused before a question is asked, and each bad answer is dropped with its
reason. It exits with status 0, and the checks assert each refusal.

<!-- source: psionic/examples/voice-steer-refuse.coax main -->
```jai
main :: () {
    c:=vs_config();
    d:=c;d.model.latency=5;open("an answer 5 ticks after the question, with a 4-tick share",d);
    d=c;d.ask_end=60;d.share=10;d.rule_barge=40;open("asked at 60 ticks with a 10-tick share: an answer could come as the 70-tick rule ends the turn",d);
    d=c;d.share=8;open("an 8-tick share: an answer at 16 could come after the counter's 24",d);
    d=c;d.costs.cut=0;open("a cut-off that costs nothing",d);
    d=c;d.hold_to=101;open("a hold to 101 ticks",d);
    d=c;d.floor=1.5;open("a confidence floor of 1.5",d);
    d=c;d.pipe.asr_lag=21;open("partial words 21 ticks late",d);
    x:=vsx_line();x.start[1]=x.turn.heard[0];
    print("a word that starts where the one before it ends: %\n",vs_why_name(vs_line(c,x,0).why));
    x=vsx_line();x.bits[5]=9;
    print("an overlap tick with bit 8 set: %\n",vs_why_name(vs_line(c,x,0).why));
    print("answers at the gate, the turn's epoch 3, deadline 104:\n");
    p:=VsProposal.{kind=VsKind.turn_end,act=VsAct.end,p=0.9,epoch=3,asked=100,arrives=102,deadline=104};
    q:=p;print("  end at 0.9, arriving at 102: %\n",vs_drop_name(vs_gate(c,q,3)));
    q=p;q.act=VsAct.stop;print("  stop, to a question about the turn: %\n",vs_drop_name(vs_gate(c,q,3)));
    q=p;q.p=1.2;print("  end at 1.2: %\n",vs_drop_name(vs_gate(c,q,3)));
    q=p;q.p=0.3;print("  end at 0.3, under the floor of 0.4: %\n",vs_drop_name(vs_gate(c,q,3)));
    q=p;print("  end at 0.9 after the user spoke again (epoch 4): %\n",vs_drop_name(vs_gate(c,q,4)));
    q=p;q.arrives=105;print("  end at 0.9, arriving at 105: %\n",vs_drop_name(vs_gate(c,q,3)));
    s:=c;s.model.on=false;o:=vs_line(s,vsx_line(),0);
    print("a stopped model: % questions, % absent; the turn ends at % by the rule and first audio comes % ticks after the user stopped\n",
        o.log.asked,o.log.drops[cast(int)VsDrop.absent],o.eot,o.first);
}
```

```text
an answer 5 ticks after the question, with a 4-tick share: budget, 0 questions asked
asked at 60 ticks with a 10-tick share: an answer could come as the 70-tick rule ends the turn: budget, 0 questions asked
an 8-tick share: an answer at 16 could come after the counter's 24: budget, 0 questions asked
a cut-off that costs nothing: costs, 0 questions asked
a hold to 101 ticks: timing, 0 questions asked
a confidence floor of 1.5: floor, 0 questions asked
partial words 21 ticks late: pipe, 0 questions asked
a word that starts where the one before it ends: line
an overlap tick with bit 8 set: line
answers at the gate, the turn's epoch 3, deadline 104:
  end at 0.9, arriving at 102: kept
  stop, to a question about the turn: shape
  end at 1.2: clamp
  end at 0.3, under the floor of 0.4: floor
  end at 0.9 after the user spoke again (epoch 4): epoch
  end at 0.9, arriving at 105: late
a stopped model: 3 questions, 3 absent; the turn ends at 150 by the rule and first audio comes 116 ticks after the user stopped
```

<!-- source: psionic/lib/voice-steer.coax check-refuse -->
```jai
#test "configurations, lines and sets that are refused" {
    c:=vs_config();#assert vs_check(c)==VsWhy.none;
    d:=c;d.costs.cut=0;#assert vs_check(d)==VsWhy.costs;
    d=c;d.costs.tick= -1;#assert vs_check(d)==VsWhy.costs;
    d=c;d.costs.restart=TH_COST_MAX*2;#assert vs_check(d)==VsWhy.costs;
    z:=0.0;d=c;d.costs.false_stop=z/z;#assert vs_check(d)==VsWhy.costs;
    d=c;d.costs.missed=0;#assert vs_check(d)==VsWhy.costs;
    d=c;d.costs.restart=TH_COST_MAX;#assert vs_check(d)==VsWhy.none;
    d=c;d.ask_end=0;#assert vs_check(d)==VsWhy.timing;
    d=c;d.ask_end=1;#assert vs_check(d)==VsWhy.none;
    d=c;d.rule_end=d.ask_end;#assert vs_check(d)==VsWhy.timing;
    d=c;d.hold_to=d.rule_end;#assert vs_check(d)==VsWhy.timing;
    d=c;d.hold_to=VP_DELAY+1;#assert vs_check(d)==VsWhy.timing;
    d=c;d.hold_to=VP_DELAY;#assert vs_check(d)==VsWhy.none;
    d=c;d.window[0]=0;#assert vs_check(d)==VsWhy.timing;
    d=c;d.window[0]=1;#assert vs_check(d)==VsWhy.none;
    d=c;d.window[1]=d.window[0];#assert vs_check(d)==VsWhy.timing;
    d=c;d.rule_barge=d.window[1];#assert vs_check(d)==VsWhy.timing;
    d=c;d.rule_barge=41;#assert vs_check(d)==VsWhy.timing;
    d=c;d.rule_barge=40;#assert vs_check(d)==VsWhy.none;
    d=c;d.model.latency= -1;#assert vs_check(d)==VsWhy.timing;
    d=c;d.model.jitter= -1;#assert vs_check(d)==VsWhy.timing;
    d=c;d.model.jitter=101;#assert vs_check(d)==VsWhy.timing;
    d=c;d.model.jitter=100;#assert vs_check(d)==VsWhy.none;
    // The decision must fit inside its share, and the share before the rules act.
    d=c;d.model.latency=5;#assert vs_check(d)==VsWhy.budget;
    d=c;d.model.latency=4;#assert vs_check(d)==VsWhy.none;
    d=c;d.ask_end=60;d.share=10;d.rule_barge=40;#assert vs_check(d)==VsWhy.budget;
    d.share=9;#assert vs_check(d)==VsWhy.none;
    d=c;d.share=7;d.window[1]=17;#assert vs_check(d)==VsWhy.budget;
    d=c;d.share=7;#assert vs_check(d)==VsWhy.none;
    d=c;d.share= -1;d.model.latency= -2;#assert vs_check(d)==VsWhy.timing;
    d=c;d.floor=1.01;#assert vs_check(d)==VsWhy.floor;
    d=c;d.floor= -0.01;#assert vs_check(d)==VsWhy.floor;
    d=c;d.floor=1;#assert vs_check(d)==VsWhy.none;
    d=c;d.pipe.step=0;#assert vs_check(d)==VsWhy.pipe;
    d=c;d.pipe.asr_lag=d.ask_end+d.pipe.asr_final+1;#assert vs_check(d)==VsWhy.pipe;
    d=c;d.pipe.asr_lag=d.ask_end+d.pipe.asr_final;#assert vs_check(d)==VsWhy.none;
    // A refused configuration runs nothing.
    x:=vsx_line();d=c;d.floor=2;o:=vs_line(d,x,0);#assert o.why==VsWhy.floor && o.log.asked==0 && o.eot==0;
    o=vs_line(c,x,0);#assert o.why==VsWhy.none;
    #assert vs_line_check(c,x)==VsWhy.none;
    y:=x;y.turn.n=0;#assert vs_line(c,y,0).why==VsWhy.line;
    y=x;y.turn.end=y.turn.heard[2]+1;#assert vs_line_check(c,y)==VsWhy.line;
    y=x;y.start[0]=y.turn.heard[0];#assert vs_line_check(c,y)==VsWhy.line;
    y=x;y.start[1]=y.turn.heard[0];#assert vs_line_check(c,y)==VsWhy.line;
    y=x;y.start[1]=y.turn.heard[0]+1;y.start[0]= -1;#assert vs_line_check(c,y)==VsWhy.line;
    y=x;y.start[0]=0;#assert vs_line_check(c,y)==VsWhy.none;
    y=x;y.dur=0;#assert vs_line_check(c,y)==VsWhy.line;
    y=x;y.dur=1;#assert vs_line_check(c,y)==VsWhy.none;
    y=x;y.dur=121;#assert vs_line_check(c,y)==VsWhy.line;
    y=x;y.dur=120;#assert vs_line_check(c,y)==VsWhy.none;
    y=x;y.bits[3]=8;#assert vs_line_check(c,y)==VsWhy.line;
    y=x;y.bits[119]=8;#assert vs_line_check(c,y)==VsWhy.line;
    y=x;y.bits[119]=7;#assert vs_line_check(c,y)==VsWhy.none;
    xs:[2]VsLine;xs[0]=x;xs[1]=y;costs:[2]f64;
    #assert vs_score(c,xs[..],costs[0..<1]).why==VsWhy.size && vs_score(c,xs[0..<0],costs[0..<0]).why==VsWhy.size;
    #assert vs_score(d,xs[..],costs[..]).why==VsWhy.floor;
    xs[1].dur=0;#assert vs_score(c,xs[..],costs[..]).why==VsWhy.line;
    #assert vs_lines(1,xs[0..<0])==VsWhy.size && vs_lines(1,xs[..])==VsWhy.none;
    #assert !vs_count_ok(0) && vs_count_ok(1) && vs_count_ok(1024) && !vs_count_ok(1025);
    names:=[8]string.["none","costs","timing","budget","floor","pipe","line","size"];
    for k:0..<8 {#assert vs_why_name(cast(VsWhy)cast(u8)k)==names[k];}
    drops:=[7]string.["kept","absent","shape","clamp","floor","epoch","late"];
    for k:0..<7 {#assert vs_drop_name(cast(VsDrop)cast(u8)k)==drops[k];}
}
```

## Independent checks

The checks in [psionic/tests/voice-steer-checks.coax](../tests/voice-steer-checks.coax)
walk every tick of a conversation: each answer is delivered on the tick it
arrives, the epoch is read at that tick, and the counter runs frame by frame.
That reference is written apart from `vs_turn` and `vs_barge` and must agree
with them on 400 drawn conversations under random latencies, shares, floors
and windows, some with short pauses so answers arrive as speech resumes.
With the model stopped, a third computation gives the rule alone, and
chapter 184 run with a 70-tick hangover gives the first audio. The actions
are compared with the written expected costs computed directly. The file is
`#authority`, and `coax mutate` leaves no survivors.

<!-- source: psionic/lib/voice-steer.coax check-random -->
```jai
#forall "each line equals the tick-by-tick reference" seed:0..<400 #steps 200000000 {
    vsx_reference_check(seed);
}
#forall "a stopped model gives the fixed rule's outcome on every line" seed:0..<200 #steps 100000000 {
    vsx_stopped_check(seed);
}
#forall "the actions agree with chapter 216's cheapest answer" seed:0..<2000 {
    vsx_cost_check(seed);
}
```

## What is not built yet

- **Elephant as the decider.** The draft of this chapter planned Elephant
  behind `/v1/systemone` as the voice engine's `decisions:<model>` decider.
  Here the decider is a stand-in posterior with simulated answer delays.
- **A trained model and real audio.** The features are drawn by this
  chapter's generator, not measured from audio, and the model is that
  generator's posterior. Its calibration on real conversations, as in
  [chapter 16](ch016-are-the-probabilities-any-good.md), is unmeasured.
- **The voice engine itself.** The gate copies the order of `decide` in
  `verse/voice.coax` but is not that function; the engine's frames, its
  20 ms clock and its consent and floor rules are not driven here.
- **Measured latency.** Answer delays are simulated ticks. Wall-clock
  decision latency, and the share of answers on time under load, need
  [chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s benchmark and
  [chapter 163](ch163-execute-nemotron-nano.md)'s measured model latency.
