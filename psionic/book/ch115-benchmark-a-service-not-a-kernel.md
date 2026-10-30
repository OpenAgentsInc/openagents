# 115. Benchmark a service, not a kernel

A kernel benchmark times one operation on a warm device with nothing else
running. A person using the service sees something else: the time until the
first word appears, the pauses between words, the whole answer, and whether
the server answered at all. Those depend on what else arrives at the same
moment, how long the prompts are, what the client asks for, and the network
and serialization around the model. This chapter builds a load generator and
a way of reading its timings that measure the service.

Everything that decides lives in Coax:
[`psionic/lib/bench.coax`](../lib/bench.coax) makes the schedule, computes
every metric and percentile, and judges a run against a baseline;
[`psionic/lib/bench-wire.coax`](../lib/bench-wire.coax) writes the request
bodies and says which received lines are tokens. Both are `#authority` files
with zero surviving mutants. The host program,
[`psionic/platform/bench-drive.c`](../platform/bench-drive.c), only connects,
sends at the scheduled times, reads, and keeps a monotonic clock.

```shell
coax/bin/coax run psionic/examples/bench-service.coax
psionic/check.sh --only bench bench-wire
```

```jai
schedule, seed 115:
  at 0.0 ms: 8 words, up to 16 tokens, topic 0, effort medium
  at 2005.708 ms: 8 words, up to 32 tokens, topic 2, effort low
  at 6935.126 ms: 128 words, up to 16 tokens, topic 0, effort high
  at 9275.753 ms: 8 words, up to 32 tokens, topic 3, effort medium
  at 12275.826 ms: 32 words, up to 32 tokens, topic 0, effort medium
  at 17572.702 ms: 32 words, up to 16 tokens, topic 3, effort high
  at 19940.358 ms: 8 words, up to 16 tokens, topic 3, effort medium
  at 23575.182 ms: 32 words, up to 32 tokens, topic 2, effort medium
seed 116 starts at 2528.88 ms, 7388.47, 10055.148, 15829.976
request 0's body: {"model":"gpt-oss-20b","input":"Summarize these words: clock garden clock stone eleven green cloud silver","max_output_tokens":16,"stream":true,"reasoning":{"effort":"medium"}}
shadow at 200 percent: 1002.854 ms, 3467.563 ms, ... 11787.591 ms
three requests by hand: 2 answered, 1 refused, 7 tokens in 4500 us, 1555.555 tokens/s in total
  time to first token, us: n 2, P50 200.0, P90 300.0, P95 300.0, P99 300.0
  inter-token latency, us: n 5, P50 100.0, P90 500.0, P95 500.0, P99 500.0
  end to end, us: n 2, P50 500.0, P90 1200.0, P95 1200.0, P99 1200.0
  inference, us: n 1, P50 400.0, P90 400.0, P95 400.0, P99 400.0
  per-user tokens/s: n 2, P50 3333.333, P90 6000.0, P95 6000.0, P99 6000.0
gpt-oss, gap 4000 ms -> 2000 ms, margin 2 percent:
  time to first token: better better better better
  inter-token latency: within within worse within
  end to end: within within within within
  inference: unmeasured unmeasured unmeasured unmeasured
  end to end less inference: unmeasured unmeasured unmeasured unmeasured
  per-user tokens/s: within within within within
and the seed changed too: 2 variables changed -> refused
```

The [example](../examples/bench-service.coax) needs no server. Its last part
reads runs recorded against the gpt-oss server on a heavily loaded host; see
[what those numbers are](#against-the-real-servers).

## A schedule from a named generator

A run is a list of requests, each with an arrival time, an input length, an
output limit, a content choice and a sampling choice. All five come from one
generator, SplitMix64 at a counter (`splitmix64-counter-v1`). It is the same
generator as chapter 70's `tb_bits`, and the first check pins its published
seed-zero words. Word `c` of the stream named by seed `s` depends only on `s`
and `c`, so request 5 of seed 115 is the same whether the run has 6 requests
or 4,000.

Request `i` reads counters `8i` to `8i+4`: its gap since the previous
request, its input length, its output limit, its content and its sampling
choice. The first request arrives at 0. Each later gap is the mean gap moved
uniformly by up to `jitter_pct` percent of it, so jitter 0 sends evenly and
jitter 100 lets gaps run from 0 to twice the mean. Lengths are drawn from a
histogram of up to 16 lengths with integer weights, which is how a recorded
distribution is reproduced: count the lengths you saw and use the counts as
weights.

<!-- source: psionic/lib/bench.coax schedule -->
```jai
bn_valid :: (l:BnLengths)->bool {
    if l.count<1 || l.count>16 {return false;}
    total:=0;
    for i:0..<l.count {if l.len[i]<1 || l.weight[i]<0 {return false;}total+=l.weight[i];}
    return total>0;
}
// The length whose share of the total weight holds word bits.
bn_draw :: (l:BnLengths,bits:u64)->int {
    total:=0;for i:0..<l.count {total+=l.weight[i];}
    r:=cast(int)(bits%cast(u64)total);pick:=0;acc:=0;
    for i:0..<l.count {acc+=l.weight[i];if acc<=r {pick=i+1;}}
    return l.len[pick];
}
// Request i reads counters 8i to 8i+4: its gap, input length, output length,
// content and sampling choice. The first request arrives at 0.
bn_schedule :: (w:BnWorkload,out:[]BnRequest)->BnStatus #export_c {
    if w.requests<1 || w.requests>BN_REQUESTS_MAX || w.gap_us<1 || w.jitter_pct<0 || w.jitter_pct>100 {return BnStatus.shape;}
    if w.contents<1 || w.samplings<1 || !bn_valid(w.input) || !bn_valid(w.output) {return BnStatus.shape;}
    if out.count<w.requests {return BnStatus.capacity;}
    half:=w.gap_us*w.jitter_pct/100;at:=0;
    for i:0..<w.requests {
        c:=cast(u64)(8*i);
        if i>0 {at+=w.gap_us+cast(int)(bn_bits(w.seed,c)%cast(u64)(2*half+1))-half;}
        out[i]=BnRequest.{at_us=at,input=bn_draw(w.input,bn_bits(w.seed,c+1)),output=bn_draw(w.output,bn_bits(w.seed,c+2)),
            content=cast(int)(bn_bits(w.seed,c+3)%cast(u64)w.contents),sampling=cast(int)(bn_bits(w.seed,c+4)%cast(u64)w.samplings)};
    }
    return BnStatus.ok;
}
// A shadow: recorded requests at their recorded offsets, scale_pct times as
// fast (100 is real time). Its answers are measured and then discarded.
bn_replay :: (recorded:[]BnRequest,scale_pct:int,out:[]BnRequest)->BnStatus #export_c {
    n:=recorded.count;
    if n<1 || n>BN_REQUESTS_MAX || scale_pct<1 {return BnStatus.shape;}
    if out.count<n {return BnStatus.capacity;}
    for i:1..<n {if recorded[i].at_us<recorded[i-1].at_us {return BnStatus.order;}}
    t0:=recorded[0].at_us;
    for i:0..<n {out[i]=recorded[i];out[i].at_us=(recorded[i].at_us-t0)*100/scale_pct;}
    return BnStatus.ok;
}
```

The two built-in workloads are deliberately small:

| Target | Input | Output limit | Contents | Sampling |
| --- | --- | --- | --- | --- |
| `responses` (gpt-oss, chapter 48b) | 8, 32 or 128 words in 4:2:1 | 16 or 32 tokens in 2:1 | 4 instructions | reasoning effort low, medium or high |
| `systemone` (chapter 16) | 1, 4 or 16 sentences of state in 2:1:1 | one answer | 2 questions | none |

Request bodies come from [`bw_body`](../lib/bench-wire.coax). A prompt's
words are drawn from counters `2^32+64i` onward of the same seed, so a
schedule always sends the same bytes. The gpt-oss server accepts only
temperature 0 and top_p 1 (chapter 48b refuses anything else), so its
sampling choice is the reasoning effort.

## A shadow

Recorded traffic is a list of the same requests with their arrival times.
`bn_replay` keeps their offsets from the first request, scaled: 100 is real
time and 200 replays twice as fast. The driver's `--record FILE` writes the
schedule it sent, and `--replay FILE --scale PCT` sends one again. The driver
never prints or stores what a server answers, only when it answered, so a
replay is a shadow: the server does the work and nobody receives the result.
A production shadow would read the arrivals from the edge's own request log;
the file format is one request per line, `at_us input output content
sampling`.

## Percentiles by nearest rank

A percentile here is the nearest rank: the P-th percentile of n values is
the value at rank `ceil(P*n/100)` in sorted order, counting from 1. It is
always one of the values measured, never an interpolation between two. The
`#forall` check proves the defining property for every n below 24 and every
P from 1 to 100: at least P percent of the values are at or below it, and
fewer than P percent are below it.

<!-- source: psionic/lib/bench.coax rank -->
```jai
// Nearest rank: the P-th percentile of n values is the value at rank
// ceil(P*n/100) in sorted order, counting from 1. It is always one of the
// values measured. Value v holds rank r when fewer than r values are below
// it and at least r are at or below it.
bn_rank :: (p:int,n:int)->int {return (p*n+99)/100;}
bn_stats :: (values:[]int)->BnStats #export_c {
    n:=values.count;s:=BnStats.{n=n};
    for i:0..<n {
        below:=0;atmost:=0;
        for j:0..<n {
            if values[j]<values[i] {below+=1;}
            if values[j]<=values[i] {atmost+=1;}
        }
        for k:0..<4 {r:=bn_rank(BN_PCT[k],n);if below<r && atmost>=r {s.p[k]=values[i];}}
    }
    return s;
}
```

The rank makes the small-run trap explicit. With 10 values, P95 and P99 are
both rank 10, the maximum. P99 is distinct from the maximum only from 101
values on, and it means something only when many more than that were
measured. A report of P99 from eight requests is a report of the slowest
request.

## What is measured

The driver time-stamps each received line when its last byte arrives and
gives Coax, for every request in send order, the HTTP status, when it was
sent, when its last byte arrived, the server's own report of its time if it
gives one, and the arrival time of every token event. From those:

- **Time to first token**: from sending the request to the first token
  event. A request with no token events (a decision) has none.
- **Inter-token latency**: every gap between consecutive token events of one
  answer, pooled over all answers.
- **End to end**: from sending to the last byte.
- **Inference**: what the server says it spent. `/v1/systemone` reports
  `latency_ms`, in whole milliseconds. The gpt-oss server reports no timing,
  so this metric is not measured for it.
- **End to end less inference**: transport, admission, parsing and
  serialization, where inference is known.
- **Per-user tokens per second**: an answer's token events over its end-to-end
  time, one value per answer.
- **Total tokens per second**: all token events of answered requests over the
  run's span, from the first send to the last byte of the run.

Only status 200 counts as answered. 429 and 503 are busy refusals, counted
separately; anything else, including a failed connection, is a failure.

<!-- source: psionic/lib/bench.coax summarize -->
```jai
bn_refused :: (code:int)->bool {return code==429 || code==503;}
bn_answered :: (code:int)->bool {return code==200;}
// obs in send order; end_us is when the run's last byte arrived; values holds
// at least as many entries as there are requests and as there are token times.
bn_summarize :: (obs:[]BnObs,times:[]int,end_us:int,values:[]int)->BnSummary #export_c {
    n:=obs.count;
    if n<1 || n>BN_REQUESTS_MAX || times.count>BN_TIMES_MAX {return BnSummary.{status=BnStatus.shape};}
    if values.count<n || values.count<times.count {return BnSummary.{status=BnStatus.capacity};}
    m:=BnSummary.{status=BnStatus.ok};
    for i:0..<n {
        o:=obs[i];
        if o.first<0 || o.count<0 || o.first+o.count>times.count || o.server_us<-1 {return BnSummary.{status=BnStatus.shape};}
        if i>0 && o.sent_us<obs[i-1].sent_us {return BnSummary.{status=BnStatus.order};}
        last:=o.sent_us;
        for k:o.first..<o.first+o.count {if times[k]<last {return BnSummary.{status=BnStatus.order};}last=times[k];}
        if o.done_us<last || o.done_us>end_us || o.server_us>o.done_us-o.sent_us {return BnSummary.{status=BnStatus.order};}
        if bn_refused(o.code) {m.refused+=1;} else if bn_answered(o.code) {m.ok+=1;m.tokens+=o.count;} else {m.failed+=1;}
    }
    m.span_us=end_us-obs[0].sent_us;
    if m.span_us>0 {m.total_milli=m.tokens*1000000000/m.span_us;}
    // Each metric gathers its values from the answered requests, then ranks them.
    k:=0;for o:obs {if bn_answered(o.code) && o.count>0 {values[k]=times[o.first]-o.sent_us;k+=1;}}
    m.ttft=bn_stats(values[0..<k]);
    k=0;for o:obs {if bn_answered(o.code) {for j:o.first+1..<o.first+o.count {values[k]=times[j]-times[j-1];k+=1;}}}
    m.itl=bn_stats(values[0..<k]);
    k=0;for o:obs {if bn_answered(o.code) {values[k]=o.done_us-o.sent_us;k+=1;}}
    m.e2e=bn_stats(values[0..<k]);
    k=0;for o:obs {if bn_answered(o.code) && o.server_us>=0 {values[k]=o.server_us;k+=1;}}
    m.infer=bn_stats(values[0..<k]);
    k=0;for o:obs {if bn_answered(o.code) && o.server_us>=0 {values[k]=o.done_us-o.sent_us-o.server_us;k+=1;}}
    m.overhead=bn_stats(values[0..<k]);
    // Per user: the answer's tokens over its end-to-end time.
    k=0;
    for o:obs {
        if bn_answered(o.code) && o.count>0 && o.done_us>o.sent_us {values[k]=o.count*1000000000/(o.done_us-o.sent_us);k+=1;}
    }
    m.user_milli=bn_stats(values[0..<k]);
    return m;
}
```

A token event is an SSE data line whose type ends in `.delta`. The gpt-oss
server streams one event per generated ID, reasoning and final text alike,
so the count includes reasoning IDs and the Harmony framing that chapter 44
describes, not only the visible text:

<!-- source: psionic/lib/bench-wire.coax event -->
```jai
// A token arrival is an SSE data line whose type ends in ".delta"; a response
// ends at completed, incomplete or failed.
bw_event :: (line:string)->BwEvent #export_c {
    head:="data: {\"type\":\"";
    if !bw_starts(line,head) {return BwEvent.other;}
    end:=head.count;
    while end<line.count && line[end]!='"' {end+=1;}
    if end==line.count {return BwEvent.other;}
    t:=cast(string)line[head.count..<end];
    if t.count>=6 && cast(string)t[t.count-6..<t.count]==".delta" {return BwEvent.token;}
    if t=="response.completed" || t=="response.incomplete" || t=="response.failed" {return BwEvent.end;}
    return BwEvent.other;
}
```

Times are integer microseconds and rates integer milli-tokens per second, so
the arithmetic has no rounding to describe. Timings that cannot happen (a
token before its request was sent, an answer after the run ended, a server
claiming more time than the client waited, sends out of order) are refused
rather than summarized.

## One variable against a fixed baseline

A comparison needs three runs: the baseline, a repeat of the baseline with
nothing changed, and the candidate. Each run carries eight identity words:
a label for what the client cannot see (the server's build, backend and
options), the model, the target, the seed (or the replay file), the request
count, the mean gap, the jitter, and the replay scale. `bn_compare` refuses
when the repeat differs from the baseline in any of them, or the candidate in
more than one. A candidate that differs in none is a repeat, not a result.

A change counts only when the candidate clears both the baseline and its
repeat by more than a margin. The repeat is how far the metric moved when
nothing changed, so a candidate inside that band is `within`, whichever way
it moved.

<!-- source: psionic/lib/bench.coax compare -->
```jai
bn_changed :: (a:BnRun,b:BnRun)->int {
    c:=0;for i:0..<BN_VARS {if a.v[i]!=b.v[i] {c+=1;}}
    return c;
}
// Percentile k of a candidate against the baseline b and its repeat r, a
// second run of the baseline's own configuration. A change counts only when
// it clears both runs by more than margin_pct percent.
bn_compare :: (base:BnRun,repeat:BnRun,cand:BnRun,b:BnStats,r:BnStats,c:BnStats,k:int,lower:bool,margin_pct:int)->BnCall #export_c {
    call:=BnCall.{changed=bn_changed(base,cand)};
    if bn_changed(base,repeat)!=0 || call.changed>1 {call.verdict=BnVerdict.refused;return call;}
    if call.changed==0 {call.verdict=BnVerdict.repeat;return call;}
    if k<0 || k>3 || margin_pct<0 || margin_pct>99 || b.n==0 || r.n==0 || c.n==0 || b.p[k]<=0 || r.p[k]<=0 {call.verdict=BnVerdict.unmeasured;return call;}
    y:=c.p[k]*100;
    up:=y>b.p[k]*(100+margin_pct) && y>r.p[k]*(100+margin_pct);
    down:=y<b.p[k]*(100-margin_pct) && y<r.p[k]*(100-margin_pct);
    call.verdict=BnVerdict.within;
    if (down && lower) || (up && !lower) {call.verdict=BnVerdict.better;}
    if (up && lower) || (down && !lower) {call.verdict=BnVerdict.worse;}
    return call;
}
```

## Against the real servers

Build the driver and point it at a server. `bench-run.sh` reads the gpt-oss
key from the server's own environment when the server belongs to the same
user on this machine; it is never printed.

```shell
psionic/platform/bench-build.sh build/bench release plain
psionic/platform/bench-run.sh build/bench responses 8080 gpt-oss-20b 115 8 4000 50 --label gptoss-8080 --out base.run --record base.sched
psionic/platform/bench-run.sh build/bench responses 8080 gpt-oss-20b 115 8 4000 50 --label gptoss-8080 --out repeat.run
psionic/platform/bench-run.sh build/bench responses 8080 gpt-oss-20b 115 8 2000 50 --label gptoss-8080 --out cand.run
psionic/platform/bench-run.sh build/bench compare base.run repeat.run cand.run 2
psionic/platform/bench-run.sh build/bench responses 8080 gpt-oss-20b 115 8 4000 50 --label gptoss-8080 --replay base.sched --scale 100
```

The arguments after the model are the seed, the request count, the mean gap
in milliseconds and the jitter in percent.

**These numbers measure a loaded host, not the service.** They were taken on
2026-10-29 while the machine's load average was 133 to 167 from other work,
with the gpt-oss server shared with other agents. Qualified figures wait for
a quiet host. The raw run files and logs are in
[`psionic/fixtures/bench/2026-10-29/`](../fixtures/bench/2026-10-29/).

gpt-oss-20b on the RTX 4080 (chapter 48b), 8 requests, mean gap 4 s, jitter
50 percent:

| Metric | Baseline | Repeat | Gap 2 s |
| --- | --- | --- | --- |
| answered, refused busy | 6, 2 | 8, 0 | 8, 0 |
| time to first token P50 / P90 | 363 / 658 ms | 369 / 665 ms | 322 / 644 ms |
| inter-token latency P50 / P90 / P99 | 15.8 / 18.0 / 150.8 ms | 15.7 / 22.0 / 105.0 ms | 15.6 / 21.0 / 123.0 ms |
| end to end P50 / P90 | 727 / 851 ms | 725 / 859 ms | 735 / 849 ms |
| per-user tokens/s P50 | 26.9 | 26.0 | 26.5 |

The server runs one request at a time and answers a second one with 503
(chapter 48b), so the two refusals in the baseline are requests that arrived
while another was running, ours or another agent's. Every answer here was
short, so they rarely overlapped at either rate. The comparison reports time
to first token "better" at twice the rate. That is not a property of the
server: two runs a minute apart on a host this busy moved by more than a
2 percent margin. It shows why the margin has to come from several repeats
on a quiet host before a verdict means anything.

The decision target was the chapter 16 toy, `psionic-serve --toy
--fixture-delay-ms 50`, which holds each decision for a fixed time on top of
its own work. Elephant's server at port 8797 stopped while these runs were
being taken, and all six requests sent to it failed to connect. 12 requests,
mean gap 1.5 s, jitter 50 percent:

| Metric | Delay 50 ms | Repeat | Delay 250 ms |
| --- | --- | --- | --- |
| answered, refused busy | 11, 1 | 12, 0 | 10, 2 |
| end to end P50 / P90 | 437 / 1022 ms | 638 / 931 ms | 536 / 1206 ms |
| inference P50 / P90 | 433 / 990 ms | 625 / 930 ms | 536 / 2372 ms |
| end to end less inference P50 / P90 | 4.2 / 25.1 ms | 4.5 / 13.7 ms | 5.5 / 20.5 ms |

Adding 200 ms to every decision did not register at P50: the repeat alone
moved P50 by 46 percent, so the candidate was `within`. On this host the
toy's own work, which should take a few milliseconds, took hundreds. The
part outside inference stayed between 4 and 6 ms at P50, which is what the
network, admission and JSON cost here.

## Limits

- One driver thread per request and an open-loop schedule: requests are sent
  on time whether or not earlier ones have finished. A closed-loop client
  (each user waits for its answer) is not built.
- Token events are SSE events, not tokenizer IDs; a server that sends several
  IDs per event would be undercounted.
- Inference time is only what the server reports, and `/v1/systemone`
  reports whole milliseconds. Prefill and decode are not separated here;
  chapter 14's phase timings would supply them.
- A margin of a few percent is a choice, not a measurement. Chapter 179's
  noise floor from blocks of repeats is the measured version, and it needs
  more repeats than a shared host allowed.
- Arrivals are uniform jitter around a mean, not a Poisson process or
  bursts; a recorded schedule replayed as a shadow covers those.

The [verification record](../../docs/verification/psionic-ch115-2026-10-29.md) lists what ran, against what and under what load.
