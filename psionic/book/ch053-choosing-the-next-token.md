# 53. Choosing the next token

> **Status: partly implemented.** The sampler, its counter RNG, stop
> conditions, cancellation, a byte-level grammar mask, a compiler from a
> bounded JSON Schema subset to that grammar, and trie-walk masks over a
> real Tekken vocabulary subset run on the CPU. Masks over the whole
> 131,072-token vocabulary read from `tekken.json`, and a recorded run on a
> second machine, are not done yet.

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
- a compiler from a bounded subset of JSON Schema to that automaton;
- masks for a real tokenizer's vocabulary, found in one walk of a trie of
  its tokens instead of one walk per token.

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
coax/bin/coax run psionic/examples/schema-next-token.coax
coax/bin/coax test psionic/lib/next-token.coax
coax/bin/coax test psionic/lib/next-token-schema.coax
coax/bin/coax test psionic/lib/next-token-trie.coax
psionic/check.sh --only next-token next-token-schema next-token-trie
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

## A JSON schema as an automaton

A tool call or a structured answer is usually described by a JSON Schema,
not by a list of edges. [`next-token-schema.coax`](../lib/next-token-schema.coax)
compiles a bounded subset of JSON Schema into the edges above, so the same
mask holds a model to the schema.

The automaton accepts one spelling of each value, the canonical one: no
whitespace, an object's keys in the order its `properties` are written,
every property present, strings of printable ASCII without `"` or `\`, and
integers without leading zeros or `-0`. A model held to the schema can only
write that spelling, and a reader can compare outputs byte for byte.

The subset is:

| Type | Keywords |
| --- | --- |
| `object` | `properties`; `required`, listing every property once; `additionalProperties: false` |
| `array` | `items`; `maxItems` (0 to 8, required); `minItems` |
| `string` | `enum` (1 to 16 words of up to 32 bytes), or `minLength`, `maxLength` (up to 64) and `pattern` of the form `^[...]*$` |
| `integer` | `minimum` and `maximum`, both required, within ±999,999,999 |
| `boolean`, `null` | none |

`title`, `description` and `$schema` are allowed as strings on any schema,
and schemas nest at most four deep. Any other keyword is refused, and the
refusal names it: `format`, `anyOf`, `$ref`, `multipleOf`, an optional
property, `pattern` with `+`. The compiler never guesses at a keyword it
doesn't implement.

### An integer range, by hand

Integers are the only part that needs thought. Take `minimum` −5 and
`maximum` 120. Negative numbers are a `-` followed by a magnitude from 1
to 5; the rest are 0 to 120. Each part is built from its digits, left to
right.

A state is a set of possible endings. After reading a digit prefix with
value x, the endings with j more digits are the suffixes s, 0 ≤ s < 10^j,
with lo ≤ x·10^j + s ≤ hi. For a range from 0 to 120 that set is one
interval for each j:

| Prefix | j = 0 | j = 1 | j = 2 | State |
| --- | --- | --- | --- | --- |
| `1` | 0 to 0 | 0 to 9 (10–19) | 0 to 20 (100–120) | 4 |
| `2` … `9` | 0 to 0 | 0 to 9 | empty | 5 |
| `10`, `11` | 0 to 0 | 0 to 9 (100–109, 110–119) | empty | 5 |
| `12` | 0 to 0 | 0 to 0 (120) | empty | 6 |
| `13` … `99`, `100` … `120` | 0 to 0 | empty | empty | 7 |

Two prefixes with the same intervals have the same future, so they are the
same state. `2` and `10` both mean "this is a complete number, and exactly
one more digit of any value may follow", so they share state 5. Every
prefix whose j = 0 interval holds 0 is a complete number. A prefix whose
intervals are all empty, such as `13` followed by another digit, is never
made. `0` is a complete number that takes no more digits, so it gets its
own state, 3. The magnitude 1 to 5 after `-` has one state, 2, since each
of its prefixes is complete and takes no more digits.

That gives eight states: the start, `-`, the negative end, `0`, and states
4 to 7. Counting edges: the start has `-` and ten digits (11), `-` has
`1`–`5` (5), states 4 and 5 have all ten digits (20), and state 6 has `0`
(1), so 37 in all. The example prints the same table:

```text
integer -5..120: 8 states, 37 edges
  0 -> state 3 (complete)
  1 -> state 4 (complete)
  2 -> state 5 (complete)
  10 -> state 5 (complete)
  11 -> state 5 (complete)
  12 -> state 6 (complete)
  13 -> state 7 (complete)
  120 -> state 7 (complete)
  - -> state 1 (not complete)
  -3 -> state 2 (complete)
  121 -> -1, 05 -> -1, -0 -> -1
```

The number of states depends on the number of digits, not on the size of
the range. The range −999,999,999 to 999,999,999 takes 21 states.

<!-- source: psionic/lib/next-token-schema.coax integer -->
```jai
// The completions of a digit prefix with value x: for each suffix length j,
// the suffixes s (0 <= s < 10^j) with lo <= x*10^j+s <= hi, an interval
// from a to z, empty when a > z. Returns whether any is not empty. A live
// prefix is complete when a is 0 for j = 0: it is not below lo, and being
// live it is not above hi.
ns_sign :: (x:int,lo:int,hi:int,d:int,out:[]int)->bool {
    p:=1;any:=false;
    for j:0..<d {
        a:=-nx_min(x*p-lo,0);z:=nx_min(hi-x*p,p-1);
        out[2*j]=a;out[2*j+1]=z;any=any || a<=z;p*=10;
    }
    return any;
}
// Two prefixes with the same completions are the same state: every interval
// equal, or empty in both.
ns_same :: (a:[]int,b:[]int,d:int)->bool {
    for j:0..<d {
        empty:=a[2*j]>a[2*j+1];
        if empty!=(b[2*j]>b[2*j+1]) || (!empty && (a[2*j]!=b[2*j] || a[2*j+1]!=b[2*j+1])) {return false;}
    }
    return true;
}
// Decimal digits for lo..hi (0 <= lo <= hi) after s, states found breadth
// first; end states are added to ends from at. "0" is its own end state and
// takes no more digits; every other prefix starts with 1 to 9. Returns the
// new end count.
ns_digits :: (b:[]NsBuild,w:NsWork,s:int,lo:int,hi:int,ends:[]int,at:int)->int {
    d:=1;p:=10;while p<=hi {d+=1;p*=10;}
    signs:[NS_SIGNS][18]int;values:[NS_SIGNS]int;ids:[NS_SIGNS]int;sign:[18]int;n:=at;
    if lo==0 {z:=ns_state(b);ns_edge(b,w,s,'0','0',z);ends[n]=z;n+=1;}
    ids[0]=s;count:=1;k:=0;
    while k<count {
        from:=ifx k==0 then 1 else 0;
        for c:from..<10 {
            x:=values[k]*10+c;
            if ns_sign(x,lo,hi,d,sign[..]) {
                found:=count;
                for m:1..<count {if ns_same(signs[m][..],sign[..],d) {found=m;}}
                if found==count {
                    count+=1;values[found]=x;ids[found]=ns_state(b);signs[found]=sign;
                    if sign[0]==0 {ends[n]=ids[found];n+=1;}
                }
                ns_edge(b,w,ids[k],cast(u8)('0'+c),cast(u8)('0'+c),ids[found]);
            }
        }
        k+=1;
    }
    return n;
}
// minimum..maximum: the non-negative part after s, the negative part after '-'.
ns_integer :: (b:[]NsBuild,w:NsWork,i:int,s:int,ends:[]int)->int {
    x:=rs_get(w.nodes,i,"minimum");y:=rs_get(w.nodes,i,"maximum");
    lo:=ns_number(w,x);hi:=ns_number(w,y);
    if lo.why!=NsWhy.none {return ns_fail(b,lo.why,"minimum",i);}
    if hi.why!=NsWhy.none {return ns_fail(b,hi.why,"maximum",i);}
    if lo.value>hi.value {return ns_fail(b,NsWhy.range,"minimum",i);}
    n:=0;
    if lo.value<0 {m:=ns_state(b);ns_edge(b,w,s,'-','-',m);n=ns_digits(b,w,m,-nx_min(-1,hi.value),-lo.value,ends,0);}
    if hi.value>=0 {n=ns_digits(b,w,s,-nx_min(0,-lo.value),hi.value,ends,n);}
    return n;
}
```

### Values, objects and arrays

Every value is compiled from a start state and returns its end states. The
caller then adds the next byte, `,`, `]` or `}`, from each end state. An
integer can have several end states, because `12` is complete and can also
continue. Every other value ends on one closing byte.

<!-- source: psionic/lib/next-token-schema.coax value -->
```jai
// Schema node i as a value starting in state s. Its end states go to ends:
// after them the next byte is the parent's , ] or }. Returns how many, or -1.
ns_value :: (b:[]NsBuild,w:NsWork,i:int,s:int,depth:int,ends:[]int)->int {
    if depth>NS_DEPTH {return ns_fail(b,NsWhy.depth,w.nodes[i].key,i);}
    if !rs_kind(w.nodes,i,JSON_OBJECT) {return ns_fail(b,NsWhy.value,w.nodes[i].key,i);}
    kind:=rs_string(w.nodes,rs_get(w.nodes,i,"type"));none:=[1]string.["type"];
    if kind=="integer" {keys:=[2]string.["minimum","maximum"];if ns_keys(b,w,i,keys[..]) {return -1;}return ns_integer(b,w,i,s,ends);}
    if kind=="string" {keys:=[4]string.["enum","minLength","maxLength","pattern"];if ns_keys(b,w,i,keys[..]) {return -1;}return ns_string(b,w,i,s,ends);}
    if kind=="boolean" {if ns_keys(b,w,i,none[..]) {return -1;}ends[0]=ns_bytes(b,w,s,"true");ends[1]=ns_bytes(b,w,s,"false");return 2;}
    if kind=="null" {if ns_keys(b,w,i,none[..]) {return -1;}ends[0]=ns_bytes(b,w,s,"null");return 1;}
    if kind=="array" {keys:=[3]string.["items","minItems","maxItems"];if ns_keys(b,w,i,keys[..]) {return -1;}return ns_array(b,w,i,s,depth,ends);}
    if kind=="object" {keys:=[3]string.["properties","required","additionalProperties"];if ns_keys(b,w,i,keys[..]) {return -1;}return ns_object(b,w,i,s,depth,ends);}
    // Unknown keys are named before a missing or unsupported type.
    if ns_keys(b,w,i,none[..]) {return -1;}
    return ns_fail(b,NsWhy.type,"type",i);
}
// [ item , item ... ]: maxItems copies of the item, ] after each from minItems on.
ns_array :: (b:[]NsBuild,w:NsWork,i:int,s:int,depth:int,ends:[]int)->int {
    items:=rs_get(w.nodes,i,"items");if items== -1 {return ns_fail(b,NsWhy.value,"items",i);}
    most:=ns_count(b,w,i,"maxItems",false,NS_ITEMS_MAX);if most== -2 {return -1;}
    least:=ns_count(b,w,i,"minItems",true,most);if least== -2 {return -1;}
    open:=ns_bytes(b,w,s,"[");close:=ns_state(b);
    if least<=0 {ns_edge(b,w,open,']',']',close);}
    if most==0 {ends[0]=close;return 1;}
    inner:=depth+1;n:=ns_value(b,w,items,open,inner,ends);if n== -1 {return -1;}
    // Every later item is the same schema, which has just compiled.
    for k:1..<most+1 {
        if k>=least {for e:ends[0..<n] {ns_edge(b,w,e,']',']',close);}}
        if k<most {n=ns_value(b,w,items,ns_join(b,w,ends[0..<n],','),inner,ends);}
    }
    ends[0]=close;return 1;
}
// { "key":value , ... }: the properties in the order they are written, each required once.
ns_object :: (b:[]NsBuild,w:NsWork,i:int,s:int,depth:int,ends:[]int)->int {
    p:=rs_get(w.nodes,i,"properties");q:=rs_get(w.nodes,i,"required");more:=rs_get(w.nodes,i,"additionalProperties");
    if !rs_kind(w.nodes,p,JSON_OBJECT) {return ns_fail(b,NsWhy.value,"properties",i);}
    if more!= -1 && !rs_kind(w.nodes,more,JSON_FALSE) {return ns_fail(b,NsWhy.value,"additionalProperties",i);}
    if !rs_kind(w.nodes,q,JSON_ARRAY) {return ns_fail(b,NsWhy.value,"required",i);}
    count:=0;k:=p+1;
    while k<w.nodes[p].next {
        name:=w.nodes[k].key;listed:=0;j:=q+1;while j<w.nodes[q].next {if rs_string(w.nodes,j)==name {listed+=1;}j=w.nodes[j].next;}
        if name.count==0 || !ns_word(name) {return ns_fail(b,NsWhy.value,"properties",i);}
        if listed!=1 {return ns_fail(b,NsWhy.value,"required",i);}
        count+=1;k=w.nodes[k].next;
    }
    if w.nodes[q].next-q-1!=count {return ns_fail(b,NsWhy.value,"required",i);}
    ends[0]=ns_bytes(b,w,s,"{");n:=1;k=p+1;first:=true;
    while k<w.nodes[p].next {
        x:=ns_join(b,w,ends[0..<n],ifx first then '"' else ',');if !first {x=ns_bytes(b,w,x,"\"");}
        x=ns_bytes(b,w,ns_bytes(b,w,x,w.nodes[k].key),"\":");
        n=ns_value(b,w,k,x,depth+1,ends);if n== -1 {return -1;}
        first=false;k=w.nodes[k].next;
    }
    ends[0]=ns_join(b,w,ends[0..<n],'}');return 1;
}
```

An array with `maxItems` n is n copies of its item, so a long array of
large items can pass 4,096 states. That schema is refused with `capacity`.
An `enum` shares states along common prefixes, so `"al"` and `"ann"` both
start with the same `"a` states.

<!-- source: psionic/lib/next-token-schema.coax schema -->
```jai
// A schema's JSON text to a compiled grammar in the caller's buffers.
nx_schema :: (text:string,w:NsWork)->NsSchema #export_c {
    if !rs_json(text,w.nodes,w.arena).ok {return NsSchema.{why=NsWhy.json};}
    if w.ends.count<NS_ENDS {return NsSchema.{why=NsWhy.capacity};}
    b:[1]NsBuild;start:=ns_state(b[..]);
    n:=ns_value(b[..],w,0,start,1,w.ends);
    if n== -1 {return NsSchema.{why=b[0].why,keyword=b[0].keyword,node=b[0].node};}
    if b[0].edges>w.edges.count || n>w.accepting.count {return NsSchema.{why=NsWhy.capacity};}
    for e,k:w.ends[0..<n] {w.accepting[k]=e;}
    c:=nx_compile(b[0].states,start,w.edges[0..<b[0].edges],w.accepting[0..<n],w.next,w.accept,w.live,false);
    if c.status!=NxStatus.ok {return NsSchema.{why=NsWhy.capacity};}
    return NsSchema.{states=b[0].states,edges=b[0].edges,grammar=c.grammar};
}
```

## Masks for a real vocabulary

`nx_mask` walks every token's bytes through the automaton. A real
vocabulary has 130,000 tokens or more, and most of them share prefixes:
`c`, `ca`, `cap`, `capital`. [`next-token-trie.coax`](../lib/next-token-trie.coax)
stores the tokens' bytes once, as a trie, and walks it beside the automaton.
When a prefix leaves the grammar, or reaches a state from which no complete
text can be reached, the walk skips that whole subtree.

A small trace. The pieces are `a`, `ab`, `abc` and `b`, and the grammar is
the string enum `"a"`, `"ab"`, in the state just after the opening quote.
The trie's root has two children, `a` and `b`. The walk reads `a`: the
grammar allows it, so token `a` is offered and the walk goes down. `ab` is
allowed and offered. `abc` leaves the grammar, so it is skipped with any
tokens below it. Back at the root, `b` leaves the grammar too. Two tokens
are offered after visiting four trie nodes. A token-by-token walk would have
read seven bytes.

<!-- source: psionic/lib/next-token-trie.coax trie -->
```jai
nt_build :: (pieces:[]string,byte:[]u8,child:[]int,sibling:[]int,first:[]int,after:[]int)->NtBuilt #export_c {
    if !nx_vocab(pieces.count) {return NtBuilt.{status=NxStatus.shape};}
    room:=nx_min(nx_min(byte.count,child.count),nx_min(sibling.count,first.count));
    if room<1 || after.count<pieces.count {return NtBuilt.{status=NxStatus.capacity};}
    // The root has no siblings; its sibling entry is never read.
    child[0]=-1;first[0]=-1;n:=1;
    for piece,t:pieces {
        at:=0;
        for c:piece {
            k:=child[at];while k!= -1 && byte[k]!=c {k=sibling[k];}
            if k== -1 {
                if n==room {return NtBuilt.{status=NxStatus.capacity,cause=t};}
                k=n;n+=1;byte[k]=c;child[k]=-1;first[k]=-1;sibling[k]=child[at];child[at]=k;
            }
            at=k;
        }
        after[t]=first[at];first[at]=t;
    }
    return NtBuilt.{trie=NtTrie.{nodes=n,byte=byte[0..<n],child=child[0..<n],sibling=sibling[0..<n],first=first[0..<n],after=after[0..<pieces.count]}};
}
```
<!-- source: psionic/lib/next-token-trie.coax mask -->
```jai
// The trie walked beside the automaton from state. The stack holds pairs
// (node, the state before its byte); a node whose byte leaves the grammar or
// reaches a dead state is skipped with all its descendants. passes counts the
// nodes visited.
nt_mask :: (t:NtTrie,g:NxGrammar,state:int,end_token:int,out:[]u8,stack:[]int,overlap:bool)->NxResult #export_c {
    v:=t.after.count;
    if !nx_vocab(v) || end_token<0 || end_token>=v {return NxResult.{status=NxStatus.shape};}
    if g.next.count<g.states*256 || g.accepting.count<g.states || g.live.count<g.states || out.count<v || stack.count<2 {return NxResult.{status=NxStatus.capacity};}
    if state<0 || state>=g.states || g.live[state]!=1 {return NxResult.{status=NxStatus.state};}
    if overlap {return NxResult.{status=NxStatus.alias};}
    for i:0..<v {out[i]=0;}
    stack[0]=t.child[0];stack[1]=state;top:=ifx t.child[0]!= -1 then 2 else 0;visited:=0;allowed:=0;
    while top!=0 {
        top-=2;k:=stack[top];s:=stack[top+1];visited+=1;
        // The next sibling takes this slot; its state is the same s, already there.
        if t.sibling[k]!= -1 {stack[top]=t.sibling[k];top+=2;}
        q:=g.next[s*256+cast(int)t.byte[k]];
        if q!= -1 && g.live[q]==1 {
            x:=t.first[k];while x!= -1 {out[x]=1;allowed+=1;x=t.after[x];}
            if t.child[k]!= -1 {
                if top+2>stack.count {return NxResult.{status=NxStatus.capacity};}
                stack[top]=t.child[k];stack[top+1]=q;top+=2;
            }
        }
    }
    allowed+=cast(int)g.accepting[state]-cast(int)out[end_token];out[end_token]=g.accepting[state];
    return NxResult.{written=v,kept=allowed,passes=visited};
}
// One row per state: masks[s*V..<(s+1)*V]. Dead states get an empty row.
nt_table :: (t:NtTrie,g:NxGrammar,end_token:int,masks:[]u8,stack:[]int)->NxResult #export_c {
    v:=t.after.count;total:=NxResult.{};
    if masks.count<g.states*v {return NxResult.{status=NxStatus.capacity};}
    for s:0..<g.states {
        row:=masks[s*v..<s*v+v];
        if g.live[s]==1 {
            m:=nt_mask(t,g,s,end_token,row,stack,false);if m.status!=NxStatus.ok {return NxResult.{status=m.status};}
            total.kept+=m.kept;total.passes+=m.passes;
        } else {for i:0..<v {row[i]=0;}}
    }
    total.written=g.states*v;return total;
}
```

`nt_table` builds one mask row per state, once per grammar. `nt_guided` is
`nx_guided` with its mask read from that table. Every check compares the
trie's rows with `nx_mask`'s.

<!-- source: psionic/lib/next-token-trie.coax guided -->
```jai
// nx_guided with the mask read from the table instead of walked per token.
nt_guided :: (logits:[]f32,g:NxGrammar,state:int,pieces:[]string,end_token:int,masks:[]u8,options:NxOptions,run:NxRun,cancel:bool,out:[]f64,work:NxWork,overlap:bool)->NxGuided #export_c {
    v:=pieces.count;
    if logits.count!=v || state<0 || state>=g.states || masks.count<g.states*v {return NxGuided.{status=NxStatus.shape,state=state};}
    stops:=[1]int.[end_token];
    e:=nx_step(logits,masks[state*v..<state*v+v],options,run,stops[..],cancel,out,work,overlap);
    if e.status!=NxStatus.ok {return NxGuided.{status=e.status,state=state};}
    after:=state;if e.token!= -1 && e.token!=end_token {after=nx_walk(g,state,pieces[e.token]);}
    return NxGuided.{token=e.token,run=e.run,state=after};
}
```

### Tekken

The vocabulary in the checks and the example is real: Tekken v13, the
tokenizer of Ministral 3 ([chapter 81](ch081-execute-ministral-4.md)), restricted
to chapter 81's fixture strings. That is 417 tokens, plus the 166
single-byte tokens the fixture lacks, plus `</s>`, 584 in all, with their
real ids. In Tekken, ranks 0 to 255 are the 256 single bytes, ids 1000 to
1255. The checks confirm this for the 90 single-byte tokens in the fixture,
and it was confirmed against the whole of the pinned `tekken.json`. The
single bytes are what let the vocabulary spell every schema.

The example compiles a schema with a city name of up to 12 lowercase
letters and spaces, a year from 1900 to 2100, and a boolean, and masks the
vocabulary in all of its 56 states:

```text
schema: 56 states, 100 edges
vocabulary: 584 Tekken tokens, trie of 645 nodes
table: 56 rows, 2466 offers, 17087 trie nodes visited (token walks would read up to 74032 bytes)
rows equal to token-by-token walks: 56 of 56
```

Then it samples. Each seed draws a random logit for every token and samples
at temperature 1, through the masks, until `</s>`:

```text
seed 1: {"city":"airanceealir","year":2100,"open":false} (38 tokens, stop_token, parses: true, validates: true)
```

The tokens are real Tekken pieces such as ` ca` (id 7887). Because only
offered tokens can be drawn, every run ends in a complete parse, and the
independent validator accepts it.

## The second example

The [second example](../examples/schema-next-token.coax) runs everything in
this part of the chapter and prints the transcript below, identically in
debug and release builds.

```shell
coax/bin/coax run psionic/examples/schema-next-token.coax
```

```text
integer -5..120: 8 states, 37 edges
  0 -> state 3 (complete)
  1 -> state 4 (complete)
  2 -> state 5 (complete)
  10 -> state 5 (complete)
  11 -> state 5 (complete)
  12 -> state 6 (complete)
  13 -> state 7 (complete)
  120 -> state 7 (complete)
  - -> state 1 (not complete)
  -3 -> state 2 (complete)
  121 -> -1, 05 -> -1, -0 -> -1
schema: 56 states, 100 edges
  {"city":"oslo","year":1905,"open":true} -> true
  {"city":"","year":2100,"open":false} -> true
  {"city":"new york city","year":1950,"open":true} -> false
vocabulary: 584 Tekken tokens, trie of 645 nodes
table: 56 rows, 2466 offers, 17087 trie nodes visited (token walks would read up to 74032 bytes)
rows equal to token-by-token walks: 56 of 56
after '', offered 1: '{'=1123
after '{"city":"new', offered 240: ' '=1032 '  '=1256 '   '=1293 ' c'=1272 ' ca'=7887 ' cap'=2735
after '{"city":"oslo","year":19', offered 10: '0'=1048 '2'=1050 '5'=1053 '1'=1049 '3'=1051 '4'=1052
seed 1: {"city":"airanceealir","year":2100,"open":false} (38 tokens, stop_token, parses: true, validates: true)
seed 2: {"city":"ailingommeem","year":2100,"open":true} (38 tokens, stop_token, parses: true, validates: true)
seed 3: {"city":"tabommopilin","year":1969,"open":true} (38 tokens, stop_token, parses: true, validates: true)
seed 4: {"city":" thefordmbis","year":1999,"open":true} (39 tokens, stop_token, parses: true, validates: true)
seed 5: {"city":"lding stothe","year":2005,"open":false} (39 tokens, stop_token, parses: true, validates: true)
refused {"type":"string","format":"date"}: keyword, keyword 'format'
refused {"type":"object","properties":{"a":{"type":"null"}},"required":[]}: value, keyword 'required'
refused {"type":"number"}: type, keyword 'type'
refused {"type":"string","pattern":"^[a-z]+$"}: value, keyword 'pattern'
```

The last four lines are the broken schemas: a `format` keyword, an
optional property, a `number`, and a pattern with `+`. Each one is refused
before any state is built, and the refusal names the keyword.

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
- **Schemas against a separate validator.**
  [`next-token-schema-oracle.coax`](../tests/next-token-schema-oracle.coax)
  generates random small schemas. It has its own validator, which reads a
  schema and a text, and its own counter of how many valid texts a schema
  has at each length. It shares no code with the compiler. For 120 random
  schemas, the compiled automaton's count of accepted strings equals the
  counter's at every length up to the largest length with at most 1,000
  texts. Every string the automaton accepts up to that length is accepted by
  the validator, so the two sets are equal. Forty one-byte edits of accepted
  texts per schema are accepted by the automaton exactly when the validator
  accepts them.
- **Integer ranges by brute force.** Eighteen ranges, from a single value
  to ±999,999,999, are checked at both bounds, one past them, and every
  value within 20 of a narrow range.
- **Refusals.** More than 80 broken schemas, each refused with the
  expected reason, keyword and node.
- **Trie masks.** In every live state of the list grammar, the hand schema
  and 24 random schemas, the trie's mask over the 584 Tekken tokens equals
  `nx_mask`'s, token for token. Twenty seeded runs through the table all
  stop at `</s>`, parse, and pass the validator.
- **Mutation.** `coax mutate` leaves no surviving mutant in
  `next-token.coax`, `next-token-schema.coax` or `next-token-trie.coax`.

The transcript is compared exactly in debug and release builds.

## What remains

- **The whole vocabulary.** The trie and its masks are checked on a
  584-token subset of Tekken. Reading all 131,072 tokens from
  `tekken.json` (chapter 81's reader does this) and recording the table's
  size and time over the whole vocabulary is not done yet.
- **More of JSON Schema.** Optional properties, `number`, `anyOf`, `$ref`,
  `format`, escapes and non-ASCII text in strings, and whitespace between
  tokens are refused, not compiled.
- **Two hosts.** The output is a pure function of the seed, the counter and
  IEEE binary64 arithmetic, and the transcripts match in debug and release
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
| `nx_schema(text, work)`, `NsWork`, `ns_work(...)` | a bounded JSON Schema to a compiled grammar, or a refusal naming the keyword |
| `nt_build(...)`, `NtTrie` | a trie of a vocabulary's pieces |
| `nt_mask(...)`, `nt_table(...)` | one state's mask, or one row per state, by walking the trie |
| `nt_guided(...)` | `nx_guided` reading its mask from the table |
| `nt_tekken(...)` | the 584-token Tekken subset with real ids |
