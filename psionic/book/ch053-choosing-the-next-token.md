# 53. Choosing the next token

> **Status: partly implemented.** The sampler, its counter RNG, stop
> conditions, cancellation and a byte-level grammar mask run on the CPU.
> Compiling a JSON schema into a grammar, masking a real tokenizer's
> vocabulary from chapter 9's tables, and a recorded run on a second host
> are not built yet.

A model ends each step with one logit per token. Something has to turn those
numbers into one token ID, and that rule decides whether a run can be
repeated, whether two programs agree, and whether the output can be malformed.
This chapter writes the rule down and builds it as a library unit,
[`next-token.coax`](../lib/next-token.coax), that any model in this book can
call:

- a named counter-based random number generator, so a seed and a counter
  name exactly one draw on any host;
- temperature, top-k and top-p, with one stated tie rule;
- stop tokens, a token limit and cancellation, each with an exact effect on
  the counter;
- a grammar, written as a finite-state automaton over bytes, that masks the
  vocabulary before every draw, so a token that would break the parse is
  never offered.

[Chapter 64](ch064-execute-gpt-oss-20b.md) needed sampling before this
chapter existed and built its own copy inside the gpt-oss unit
(`psionic/lib/gptoss/sampling.coax`), and [chapter 77](ch077-call-a-tool.md)
added a mask for a finite set of JSON objects. Those files are pinned by
hash in chapter 64's qualification records, so they stay as they are. This
unit uses the same random number generator, the same tie rule and the same
truncation order, without any gpt-oss vocabulary size or token IDs, so the
chapters that come later (58, 78, 81, 126, 146, 200, 203 and 245) can share
it.

```shell
coax/bin/coax run psionic/examples/choose-next-token.coax
coax/bin/coax test psionic/lib/next-token.coax
psionic/check.sh --only next-token
python3 -I psionic/tests/next_token_oracle.py
```

```text
logits: 2 1 2 0 -1
  T 1: 0.3917 0.1441 0.3917 0.053 0.0195  (5 kept)
  T 0.5: 0.4638 0.0628 0.4638 0.0085 0.0011  (5 kept)
  T 1, top-k 1: 1.0 0.0 0.0 0.0 0.0  (1 kept)
  T 1, top-k 2: 0.5 0.0 0.5 0.0 0.0  (2 kept)
  T 1, top-p 0.5: 0.5 0.0 0.5 0.0 0.0  (2 kept)
  T 0 (greedy): 1.0 0.0 0.0 0.0 0.0  (1 kept)
  sort order: 0 2 1 3 4
seed 45001 counter 0: uniform 0.3924, token 1
seed 45001 counter 1: uniform 0.2935, token 0
seed 45001 counter 2: uniform 0.0229, token 0
stopped: limit, counter now 3
20000 seeds, counts: 7843 2890 7827 1042 398; expected: 7834.0 2882.0 7834.0 1060.0 390.0
state 0: offers '[' '[1'
state 1: offers '0' '1' '7' '42'
state 2: offers ']' ',' '0' '1' '7' '42' ',0]'
state 3: offers ']' ',' ',0]'
state 4: offers '0' '1' '7' '42'
state 5: offers '<end>'
seed 1: [17,0] (4 tokens, stop_token, parses: true)
seed 2: [1707] (6 tokens, stop_token, parses: true)
seed 3: [71] (5 tokens, stop_token, parses: true)
seed 4: [42420,742,0] (11 tokens, stop_token, parses: true)
seed 5: [42] (4 tokens, stop_token, parses: true)
cancelled: token -1, state 2, counter 2, cancelled
overlapping edges: grammar, edge 3
vocabulary cannot continue '[': empty
temperature -1: shape
```

The [example](../examples/choose-next-token.coax) compiles and runs in about three seconds.

## The rule, worked by hand

Take five tokens with logits 2, 1, 2, 0 and −1.

**Order.** Sort the tokens by logit, highest first. Tokens 0 and 2 tie at 2.
The tie rule: equal logits keep ascending ID order. So the order is 0, 2, 1,
3, 4. Every later rule reads this order, so the tie rule decides every
boundary.

**Temperature 1.** Subtract the top logit and exponentiate:

| ID | logit | logit − 2 | weight |
| --- | --- | --- | --- |
| 0 | 2 | 0 | 1 |
| 2 | 2 | 0 | 1 |
| 1 | 1 | −1 | 0.36788 |
| 3 | 0 | −2 | 0.13534 |
| 4 | −1 | −3 | 0.04979 |

The weights sum to 2.55301 in that order. Dividing gives 0.3917, 0.1441,
0.3917, 0.0530 and 0.0195 by ID, the first line of the transcript.

**Temperature 0.5** divides the differences by 0.5 before exponentiating:
weights 1, e⁻² = 0.13534, 1, e⁻⁴ = 0.01832 and e⁻⁶ = 0.00248, summing to
2.15613. The two top tokens get 0.4638 each. A lower temperature sharpens the
distribution; temperature 0 is greedy and gives the first token in the order
probability 1.

**Top-k 1** keeps only the first token in the order. Tokens 0 and 2 tie, and
the tie rule keeps ID 0. **Top-k 2** keeps both, at 0.5 each.

**Top-p 0.5** keeps the shortest prefix of the order whose weight reaches
half the total, 0.5 × 2.55301 = 1.27651. After ID 0 the running weight is
1, which is less, so ID 2 joins and the weight reaches 2. The kept tokens are
renormalized to 0.5 and 0.5. At top-p 0.3 the target is 0.76590 and ID 0
alone reaches it.

**A draw.** The uniform for seed 45001 at counter 0 is 0.39237. The pick
walks the probabilities in ID order, not sort order, and takes the first ID
whose running sum exceeds the uniform: 0.3917 after ID 0 is not above it,
0.5358 after ID 1 is. The token is 1. Counter 1 gives 0.29347 and token 0;
counter 2 gives 0.02290 and token 0. Three draws with a limit of three stop
at `limit` with the counter at 3, as the transcript shows.

## A random number generator with a name

The generator is chapter 64's `splitmix64-top53-v1`, kept in
[`counter-rng.coax`](../lib/counter-rng.coax), which this unit loads. A draw
is a pure function of a 64-bit seed and a 64-bit counter:

```text
z = seed + 0x9e3779b97f4a7c15 · (counter + 1)        (wrapping, mod 2^64)
z = (z ^ (z >> 30)) · 0xbf58476d1ce4e5b9
z = (z ^ (z >> 27)) · 0x94d049bb133111eb
z =  z ^ (z >> 31)
uniform = (z >> 11) / 2^53                            in [0, 1)
```

Because there is no hidden state, a run is reproduced by its seed and its
counter alone, a step can be replayed from the middle, and two runs can
share a seed without sharing an object. Every emitted token consumes exactly
one counter, greedy steps included, so the counter always equals the number
of tokens emitted since the seed was set. The last counter, 2⁶⁴ − 1, is
refused rather than wrapped.

llama.cpp seeds `std::mt19937` and draws through the C++ standard library's
distributions, which do not specify their algorithm. A seeded llama.cpp run
can't be matched draw for draw. Compare greedy outputs, or compare
distributions, instead.

## Order and tie rule

<!-- source: psionic/lib/next-token.coax order -->
```jai
// Allowed IDs, sorted by logit descending; equal logits keep ascending ID.
// A bottom-up merge sort: stable, O(V log V), no allocation.
nx_order :: (logits:[]f32,order:[]int,temporary:[]int,count:int)->int {
    width:=1;passes:=0;
    while width<count {
        first:=0;
        while first<count {
            mid:=nx_min(first+width,count);end:=nx_min(first+2*width,count);a:=first;b:=mid;
            for dest:first..<end {
                left:=b==end;if a<mid && b<end {left=logits[order[a]]>=logits[order[b]];}
                if left {temporary[dest]=order[a];a+=1;}else {temporary[dest]=order[b];b+=1;}
            }
            first=end;
        }
        for i:0..<count {order[i]=temporary[i];}width*=2;passes+=1;
    }
    return passes;
}
```

The merge takes from the left run when its logit is greater than or equal to
the right run's. The left run always holds lower IDs, so equal logits keep
ascending ID order. This is the whole tie rule. Top-k, top-p and greedy
decoding all read this order, so a tie at any boundary goes to the lower ID.

## Temperature, top-k and top-p

<!-- source: psionic/lib/next-token.coax distribution -->
```jai
nx_distribution :: (logits:[]f32,mask:[]u8,options:NxOptions,out:[]f64,work:NxWork,overlap:bool)->NxResult #export_c {
    n:=logits.count;
    if !nx_vocab(n) || !nx_options(options,n) {return NxResult.{status=NxStatus.shape};}
    if (mask.count!=0 && mask.count!=n) || out.count<n || work.stage.count<n || work.order.count<n || work.temporary.count<n {return NxResult.{status=NxStatus.capacity};}
    if overlap {return NxResult.{status=NxStatus.alias};}
    count:=0;
    for i:0..<n {
        if !nx_logit(logits[i]) || (mask.count!=0 && mask[i]>1) {return NxResult.{status=NxStatus.number};}
        if mask.count==0 || mask[i]==1 {work.order[count]=i;count+=1;}work.stage[i]=0;
    }
    if count==0 {return NxResult.{status=NxStatus.empty};}
    passes:=nx_order(logits,work.order,work.temporary,count);
    take:=1;
    if options.temperature==0 {work.stage[work.order[0]]=1;}
    else {
        kept:=count;if options.top_k!=0 {kept=nx_min(kept,options.top_k);}
        top:=f64(logits[work.order[0]]);total:=0.0;
        for i:0..<kept {v:=exp((f64(logits[work.order[i]])-top)/options.temperature);work.stage[work.order[i]]=v;total+=v;}
        // The same weights in the same order: at top_p 1 the loop ends with mass == total.
        mass:=0.0;take=0;
        while mass<options.top_p*total {mass+=work.stage[work.order[take]];take+=1;}
        for i:0..<take {work.stage[work.order[i]]/=mass;}
        for i:take..<kept {work.stage[work.order[i]]=0;}
    }
    for i:0..<n {out[i]=work.stage[i];}
    return NxResult.{written=n,kept=take,passes=passes};
}
```

The order of operations is fixed: mask, sort, top-k, temperature weights,
top-p, renormalize. Every parameter is explicit. The zero value of
`NxOptions` is temperature 1, top-k off (0) and top-p 1, which matches
gpt-oss's published defaults; a caller that wants other values writes them.

The weights are summed in sort order, and top-p adds them again in the same
order. At top-p 1 the running sum therefore ends exactly equal to the total,
which is why the loop needs no separate bound. A weight that underflows to
zero still counts as kept: `kept` reports how many tokens the truncation
kept, not how many have positive weight.

Logits must lie within ±10³⁰. The comparison also refuses NaN and the
infinities, because NaN fails both comparisons and an infinity fails one.

## The draw

<!-- source: psionic/lib/next-token.coax pick -->
```jai
// The first ID whose running mass exceeds the uniform. If rounding leaves the
// uniform at or above the final mass, the last ID with positive mass wins.
nx_pick :: (probabilities:[]f64,uniform:f64)->NxPick #export_c {
    if !nx_vocab(probabilities.count) || !(uniform>=0 && uniform<1) {return NxPick.{status=NxStatus.shape};}
    total:=0.0;
    for v:probabilities {if !(v>=0 && v<=1) {return NxPick.{status=NxStatus.number};}total+=v;}
    if !(abs(total-1)<NX_MASS) {return NxPick.{status=NxStatus.number};}
    running:=0.0;
    for v,i:probabilities {running+=v;if uniform<running {return NxPick.{token=i};}}
    last:=probabilities.count-1;while probabilities[last]==0 {last-=1;}
    return NxPick.{token=last};
}
```

`nx_pick` checks the distribution it is given: every entry in [0, 1] and a
total within 2⁻³⁰ of 1. It can then be used with distributions that come
from somewhere else, such as a diffusion schedule or a policy.

## One step: stops, limit and cancellation

<!-- source: psionic/lib/next-token.coax step -->
```jai
nx_step :: (logits:[]f32,mask:[]u8,options:NxOptions,run:NxRun,stops:[]int,cancel:bool,out:[]f64,work:NxWork,overlap:bool)->NxEmission #export_c {
    if run.stop!=NxStop.running || run.limit>NX_LIMIT_MAX || run.emitted<0 || run.emitted>=run.limit {return NxEmission.{status=NxStatus.state};}
    for s:stops {if s<0 || s>=logits.count {return NxEmission.{status=NxStatus.shape};}}
    if cancel {next:=run;next.stop=NxStop.cancelled;return NxEmission.{run=next};}
    draw:=cr_draw(run.random);if !draw.ok {return NxEmission.{status=NxStatus.state};}
    d:=nx_distribution(logits,mask,options,out,work,overlap);if d.status!=NxStatus.ok {return NxEmission.{status=d.status};}
    pick:=nx_pick(out[0..<logits.count],draw.value);#assert pick.status==NxStatus.ok;
    next:=run;next.random=draw.next;next.emitted+=1;
    for s:stops {if s==pick.token {next.stop=NxStop.stop_token;}}
    if next.stop==NxStop.running && next.emitted==next.limit {next.stop=NxStop.limit;}
    return NxEmission.{token=pick.token,run=next};
}
```

`NxRun` holds everything a stream needs between steps: the seed and counter,
the tokens emitted, the limit and the stop reason. The rules:

- A stop token is emitted and then ends the run with `stop_token`. The caller
  decides whether to show it.
- The limit ends the run with `limit` after its last token, unless that
  token was a stop token.
- Cancellation is checked before the draw. It publishes no token (the token
  is −1), consumes no counter and ends the run with `cancelled`.
- A step on a run that has stopped is refused with `state`. So is a step
  whose counter is the last one.
- A refused distribution publishes nothing and leaves the caller's run
  unchanged.

## A grammar as a byte automaton

A grammar here is a deterministic finite-state automaton over bytes. It is
written as a list of edges, each a range of bytes from one state to another,
and a list of accepting states. The chapter's grammar is a list of integers
without leading zeros, such as `[10,0,972]`:

| From | Bytes | To | Meaning |
| --- | --- | --- | --- |
| 0 | `[` | 1 | the list opens |
| 1 | `0` | 3 | a zero, which can't be followed by digits |
| 1 | `1`–`9` | 2 | a number starts |
| 2 | `0`–`9` | 2 | the number continues |
| 2, 3 | `,` | 4 | another item follows |
| 2, 3 | `]` | 5 | the list closes; state 5 accepts |
| 4 | `0` | 3 | |
| 4 | `1`–`9` | 2 | |

<!-- source: psionic/lib/next-token.coax compile -->
```jai
nx_compile :: (states:int,start:int,edges:[]NxEdge,accepting:[]int,next:[]int,accept:[]u8,live:[]u8,overlap:bool)->NxCompiled #export_c {
    if !nx_states(states) || start<0 || start>=states {return NxCompiled.{status=NxStatus.shape};}
    if next.count<states*256 || accept.count<states || live.count<states {return NxCompiled.{status=NxStatus.capacity};}
    if overlap {return NxCompiled.{status=NxStatus.alias};}
    for i:0..<states*256 {next[i]=-1;}
    for s:0..<states {accept[s]=0;live[s]=0;}
    for e,k:edges {
        if e.from<0 || e.from>=states || e.to<0 || e.to>=states || e.low>e.high {return NxCompiled.{status=NxStatus.grammar,cause=k};}
        for b:cast(int)e.low..<cast(int)e.high+1 {
            if next[e.from*256+b]!=-1 {return NxCompiled.{status=NxStatus.grammar,cause=k};}
            next[e.from*256+b]=e.to;
        }
    }
    for s,k:accepting {if s<0 || s>=states {return NxCompiled.{status=NxStatus.grammar,cause=edges.count+k};}accept[s]=1;live[s]=1;}
    // Live: some byte path leads to acceptance. Repeat until nothing changes.
    changed:=true;
    while changed {
        changed=false;
        for s:0..<states {
            reach:=false;for b:0..<256 {t:=next[s*256+b];if t>=0 && live[t]==1 {reach=true;}}
            if reach && live[s]==0 {live[s]=1;changed=true;}
        }
    }
    if live[start]==0 {return NxCompiled.{status=NxStatus.empty};}
    return NxCompiled.{grammar=NxGrammar.{states=states,start=start,next=next[0..<states*256],accepting=accept[0..<states],live=live[0..<states]}};
}
```

The compiler fills a table of 256 entries per state, −1 meaning "no
transition". It refuses two edges that give the same byte two targets in one
state, since the automaton must be deterministic, and names the edge. It then
marks every state that can still reach an accepting state as live, repeating
until nothing changes; a language whose start is not live is empty and is
refused.

## Masking the vocabulary

A token is a byte string, its piece. In state s, a token is offered when
walking its bytes from s never leaves the table and ends in a live state. The
end token is offered only in an accepting state.

<!-- source: psionic/lib/next-token.coax mask -->
```jai
// 1 where the token keeps a complete parse reachable; the end token only
// where the text so far is complete. Empty pieces other than the end token
// make no progress and are never offered.
nx_mask :: (g:NxGrammar,state:int,pieces:[]string,end_token:int,out:[]u8,overlap:bool)->NxResult #export_c {
    if !nx_vocab(pieces.count) || end_token<0 || end_token>=pieces.count {return NxResult.{status=NxStatus.shape};}
    if g.next.count<g.states*256 || g.accepting.count<g.states || g.live.count<g.states || out.count<pieces.count {return NxResult.{status=NxStatus.capacity};}
    if state<0 || state>=g.states || g.live[state]!=1 {return NxResult.{status=NxStatus.state};}
    if overlap {return NxResult.{status=NxStatus.alias};}
    allowed:=0;
    for piece,i:pieces {
        ok:=false;
        if i==end_token {ok=g.accepting[state]==1;}
        else if piece.count!=0 {t:=nx_walk(g,state,piece);ok=t>=0 && g.live[t]==1;}
        out[i]=ifx ok then 1 else 0;if ok {allowed+=1;}
    }
    return NxResult.{written=pieces.count,kept=allowed};
}
```

Because every offered token ends in a live state, the run can always still
finish: a mask never leads into a dead end. Pieces may cross the grammar's
own boundaries. In the transcript, `[1` opens the list and starts a number in
one token, and `,0]` adds an item and closes the list. `],` is never offered:
after `]` nothing can follow.

## Guided generation

<!-- source: psionic/lib/next-token.coax guided -->
```jai
nx_guided :: (logits:[]f32,g:NxGrammar,state:int,pieces:[]string,end_token:int,options:NxOptions,run:NxRun,cancel:bool,mask:[]u8,out:[]f64,work:NxWork,overlap:bool)->NxGuided #export_c {
    if logits.count!=pieces.count {return NxGuided.{status=NxStatus.shape};}
    m:=nx_mask(g,state,pieces,end_token,mask,overlap);if m.status!=NxStatus.ok {return NxGuided.{status=m.status};}
    stops:=[1]int.[end_token];
    e:=nx_step(logits,mask[0..<pieces.count],options,run,stops[..],cancel,out,work,overlap);
    if e.status!=NxStatus.ok {return NxGuided.{status=e.status,state=state};}
    after:=state;if e.token>=0 && e.token!=end_token {after=nx_walk(g,state,pieces[e.token]);}
    return NxGuided.{token=e.token,run=e.run,state=after};
}
```

`nx_guided` masks, steps with the end token as the stop token, and advances
the grammar state by the chosen piece. The example's toy model gives its
highest logit, 9, to two tokens that are never valid: `x` and the empty
piece. Neither is ever chosen. Every run that stops at the end token parses;
a run that reaches its limit first stops with `limit`, and its text is a
prefix that can still be completed, not a complete parse.

If the vocabulary can't spell any continuation from the current state, the
mask offers nothing. The step then refuses with `empty` before drawing, so
no counter is consumed. A grammar used with a vocabulary should be checked
for this, for example by making sure every byte the grammar uses is also a
token.

## Refusals

The end of the transcript shows three broken inputs and their exact results:

- An edge list where state 1 sends `5` to two places: `grammar`, naming
  edge 3. No grammar is returned.
- A vocabulary of `[`, `]` and the end token, in state 1 after `[`: no token
  is offered, so the step refuses with `empty`.
- Temperature −1: `shape`.

The checks also cover every other bound: temperature within [0, 100],
top-k within [0, V], top-p within (0, 1], a vocabulary of 1 to 262,144
tokens, 1 to 4,096 grammar states, mask bytes that are 0 or 1, buffers that
are too short, and overlapping buffers. Each refusal publishes nothing into
the caller's output.

## Checks

- **An independent oracle.**
  [`next_token_oracle.py`](../tests/next_token_oracle.py) recomputes the
  generator's words and uniforms and the distributions for nine settings from
  the written rules, in Python. The unit's `#test` blocks compare those
  values bit for bit: the 64-bit words for seeds 45001 and 53, and every
  probability in the worked example.
- **Frequencies.** For three settings (temperature 1; temperature 2 with
  top-k 3; temperature 0.5 with top-p 0.9), 20,000 seeds each draw one
  token. Every count is within five standard deviations of
  20,000 · p, and a token with probability 0 is never drawn.
- **Ties.** A fixture with four equal logits sorts to IDs 0, 1, 2, 3, greedy
  picks ID 0, and a seven-token fixture with two pairs of ties checks the
  merge sort across uneven runs.
- **Grammar.** Three hundred seeded guided runs through the integer-list
  grammar emit only offered tokens; after each token the grammar state is
  live and equals a fresh walk of the text so far; all 288 runs that stop at
  the end token are accepted by the grammar, and the other 12 stop at the
  limit.
- **Mutation.** `coax mutate psionic/lib/next-token.coax` leaves no
  surviving mutant.

The transcript is compared exactly in debug and release builds.

## What remains

- **JSON schemas.** The draft promised masks compiled from a JSON schema.
  Chapter 77 masks a finite set of complete JSON objects; a compiler from a
  general schema to this unit's automaton is not written.
- **A real tokenizer.** The masks here take token pieces as byte strings.
  Building those pieces from chapter 9's tokenizer tables, and masking a
  full vocabulary of 150,000 or more tokens per step without walking every
  piece, is not done.
- **Two hosts.** The output is a pure function of the seed, the counter and
  IEEE binary64 arithmetic, and the transcript matches in debug and release
  builds. No run on a second machine has been recorded yet.

## What later chapters use

| Name | What it does |
| --- | --- |
| `NxOptions` | temperature, top-k, top-p; zero value 1, off, 1 |
| `nx_distribution(logits, mask, options, out, work, overlap)` | the truncated, renormalized distribution |
| `nx_pick(probabilities, uniform)` | inverse CDF in ID order |
| `NxRun`, `nx_step(...)` | one counter, one token, stop tokens, limit, cancellation |
| `NxEdge`, `nx_compile(...)`, `NxGrammar` | a byte automaton with live states |
| `nx_mask(...)`, `nx_walk`, `nx_accepts` | per-state vocabulary masks and parsing |
| `nx_guided(...)` | mask, step and advance the grammar in one call |
| `CrSeed`, `cr_draw` | the counter RNG, from `counter-rng.coax` |
