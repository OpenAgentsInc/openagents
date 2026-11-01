# 245. Reflective prompt evolution

> **Status: partly implemented.** Prompts as versioned components named by
> one identity digest, with every field but the prompt's rules fixed by the
> seed; a stand-in reflector that proposes one edit from the student's
> failures; refusal of any other reply, of a changed field, of a grader that
> shares weights with the student or the reflector or did not pass
> chapter 246's gate, of a missing canary and of a drifted answer space; a
> Pareto front over per-item scores with parents drawn by how many items
> each is tied-best on; every call priced under a budget; and the final
> prompt against the seed on a held-out partition read once, with
> chapter 241's verdict. The student and the reflector are stand-ins whose
> behavior is written down. Still to come: a served student and a
> reflector served by Psionic (gpt-oss with constrained output), and the
> comparison with chapter 243's settings fitting.

GEPA improves a prompt by showing a model the prompt's failures and asking
it to propose better text. Each proposal is tried on a few examples, kept if
it does better, and the kept prompts form a Pareto front: a prompt that is
best on some examples survives even if its average is lower. The next
proposal starts from a prompt drawn from that front.

The idea is cheap to state and easy to fool. The pre-reset attempts
produced no measured win: the one run that executed gained +0.025 against a
noise floor of 0.056, which is unverifiable. Several ways to fool it are
known:
- a reply that isn't a prompt gets installed verbatim;
- a "better prompt" quietly changes something other than the text, such as
  the tools it may call or the pass mark;
- the grader is the student's own model, grading its own work;
- the runner sends the old text, and the new prompt's score is really the
  old one's;
- the new prompt widens the answer space, for example by allowing "unsure",
  and the metric no longer means what it meant.

This chapter builds the loop with each of those refused, on a world small
enough to check by hand:
- **a prompt is a component:** a student's settings plus a set of rule lines
  from a fixed library of seven, named by one identity digest that covers
  the rendered text, the tool grants and the threshold
  ([chapter 239](ch239-a-suite-is-three-digests.md)'s encoding, over
  [chapter 246](ch246-judges-are-components.md)'s component identity);
- **a reflector that may only edit:** it reads failure traces from a small
  batch and replies `add k` or `drop k`; any other reply is refused;
- **admission:** a child keeps every field of the seed but its rules, its
  grader is separate and passed chapter 246's gate, the student echoed the
  child's canary, its answers stayed among the options, and it beat its
  parent on the batch;
- **a Pareto front:** kept prompts are scored per item, dominated ones drop
  out, and the next parent is drawn in proportion to the items it is
  tied-best on;
- **a budget:** every student call, grade and reflection has a price, and a
  step starts only when its worst case fits;
- **one held-out read:** the final prompt against the seed, through
  [chapter 240](ch240-how-far-a-metric-moves-by-itself.md)'s floor and
  [chapter 241](ch241-gates-that-can-say-unverifiable.md)'s verdict, with a
  receipt in chapter 239's chain that refuses a second read.

```shell
coax/bin/coax run psionic/examples/prompt-evolution.coax
coax/bin/coax test psionic/lib/evolve.coax
```

## The world, written down

The library has seven rule lines, numbered 0 to 6:

| Rule | Line |
| ---: | --- |
| 0 | Read the whole task before you answer. |
| 1 | Check the units before you give a number. |
| 2 | Count the items one at a time. |
| 3 | Quote the log line that shows the error. |
| 4 | Give the shortest reply that is correct. |
| 5 | Explain each step before the answer. |
| 6 | Say unsure when the task is unclear. |

A prompt's text is a fixed header, the rules it has in library order, and a
fixed answer line ("Reply with the letter of one option."). Every item has
four options, one of them right, and a kind that says which rules it needs:

| Kind | Needs | Harmed by |
| --- | --- | --- |
| count | 0 and 2 | |
| units | 0 and 1 | |
| log | 3 | |
| short | 4 | |
| explain | 5 | 4 |
| unclear | 6 | |

The stand-in student reads the text it was sent, line by line. With every
rule its item needs and no rule that harms it, it is right when the item's
draw is below 0.9; otherwise below 0.25. A wrong answer is the next option.
With rule 6, an unclear item gets "unsure", which is not one of the four
options. The draw for item i on run r is shared by every prompt, so two
prompts on the same run face the same luck.

Its trace for each item records whether it was right, the lowest needed rule
missing and the lowest harmful rule present. The stand-in reflector reads
the failures: each votes to add its missing rule and to drop its harmful
one. A draw picks one vote, so an edit is proposed in proportion to the
failures that ask for it. With no vote, there is no reply.

The world has 64 items in 32 tasks of two: items 0 to 15 are the Pareto
items every kept prompt is scored on, 16 to 39 feed the batches of four, and
40 to 63 are held out in a locked partition.

## A front by hand

Four prompts on four items, each scored 1 or 0:

| Prompt | item 0 | item 1 | item 2 | item 3 |
| --- | ---: | ---: | ---: | ---: |
| A | 1 | 1 | 0 | 0 |
| B | 1 | 0 | 1 | 0 |
| C | 1 | 0 | 0 | 0 |
| D | 1 | 1 | 0 | 0 |

- A dominates C: at least as good everywhere and better on item 1. C is off
  the front.
- D has A's scores exactly. The earlier one stays; D is off.
- A and B don't dominate each other: A wins item 1, B wins item 2. Both stay.

The front is {A, B}. Each front member's weight is the number of items on
which no prompt beats it:
- item 0: A and B tie at 1;
- item 1: A;
- item 2: B;
- item 3: everyone is at 0, so A and B tie.

A's weight is 3, B's is 3, total 6. A draw u picks the target ⌊6u⌋ and walks
the weights in order: u = 0.49 gives target 2, inside A's 3; u = 0.5 gives
target 3, past A, so B. The unit's checks assert each of these, and a
`#forall` compares the front on every 4-prompt, 2-item case with scores 0,
1 or 2 against an independent oracle that sorts by total and sweeps.

## One step by hand

The seed, v0, has rule 0 only. Step 1 draws its parent from a front of one,
so v0. The batch is items 16 to 19 on run 1:

| Item | Kind | v0 has what it needs? | Draw | v0 | v1 = v0 + rule 5 |
| ---: | --- | --- | ---: | --- | --- |
| 16 | unclear | no (rule 6), chance 0.25 | 0.0382 | right | right |
| 17 | log | no (rule 3), chance 0.25 | 0.5276 | wrong | wrong |
| 18 | explain | no (rule 5), chance 0.25 | 0.4602 | wrong | right (chance 0.9) |
| 19 | count | no (rule 2), chance 0.25 | 0.7436 | wrong | wrong |

v0 gets 1 of 4. Item 16 is right by luck, so it doesn't vote. The three
failures vote add 3, add 5 and add 2. In vote order (add 2, add 3, add 5),
the reflector's draw 0.7458 picks target ⌊3 × 0.7458⌋ = 2, the third: it
replies `add 5`. The reply parses, the child v1 has rules {0, 5}, its other
fields are the seed's, and its grader is judge 4 from chapter 246, which
runs on Mammoth's weights while the student and the reflector run on
Elephant's. On the same draws v1 gets 2 of 4, beating 1, so it is scored on
the 16 Pareto items: 8, one more than v0's 7 (item 2 is an explain item).

The prices: a student call costs 1 and a grade by judge 4 costs 18 (price 6,
three samples), so an answer costs 19. A reflection costs 4. Scoring the
seed on the Pareto items costs 16 × 19 = 304. Step 1 costs a batch for the
parent (4 × 19 = 76), a reflection (4), a batch for the child (76) and the
Pareto items (304): 460, so 764 spent. A step whose child is refused after
its batch costs 76 + 4 + 76 = 156.

## The budget by hand

A step can't know its outcome before it runs, so it starts only when its
worst case fits: 2 × 76 + 4 + 304 = 460. With a budget of 4,000, the run
stops after step 12 with 3,696 spent, since 3,696 + 460 = 4,156 is over. It
never overspends, and every unit is accounted for: each step's spend is the
seed's 304 plus the cost of the steps by their outcomes, which a `#forall`
checks for twelve budgets.

## The run

```text
items: 64 in 32 tasks; Pareto items 0-15 (3 count, 4 units, 2 log, 4 short, 1 explain, 2 unclear), batch items 16-39, held out 40-63
seed prompt v0, identity 99c03de0:
    You solve tasks from the queue.
    Read the whole task before you answer.
    Reply with the letter of one option.
grader: judge 4 (Mammoth, passed chapter 246's gate); an answer costs 1.0 + 18.0 = 19.0, a reflection 4.0; budget 4000.0
v0 on the Pareto items: 7/16; spent 304.0
step 1: front 1, parent v0 1/4 on items 16-19; reply "add 5"; child 2/4 -> accepted as v1, 8 on the Pareto items; spent 764.0
step 2: front 1, parent v1 1/4 on items 20-23; reply "add 3"; child 2/4 -> accepted as v2, 9 on the Pareto items; spent 1224.0
step 3: front 1, parent v2 1/4 on items 24-27; reply "add 6" -> drift; spent 1380.0
step 4: front 1, parent v2 2/4 on items 28-31; reply "add 6" -> drift; spent 1536.0
step 5: front 1, parent v2 1/4 on items 32-35; reply "add 6" -> drift; spent 1692.0
step 6: front 1, parent v2 0/4 on items 36-39; reply "add 2"; child 2/4 -> accepted as v3, 10 on the Pareto items; spent 2152.0
step 7: front 1, parent v3 3/4 on items 16-19; reply "add 6" -> drift; spent 2308.0
step 8: front 1, parent v3 3/4 on items 20-23; reply "add 6" -> drift; spent 2464.0
step 9: front 1, parent v3 0/4 on items 24-27; reply "add 6" -> drift; spent 2620.0
step 10: front 1, parent v3 3/4 on items 28-31; reply "add 4"; child 4/4 -> accepted as v4, 13 on the Pareto items; spent 3080.0
step 11: front 2, parent v3 2/4 on items 32-35; reply "add 6" -> drift; spent 3236.0
step 12: front 2, parent v4 1/4 on items 36-39; reply "add 1"; child 2/4 -> accepted as v5, 15 on the Pareto items; spent 3696.0
stopped: budget (spent 3696.0 of 4000.0)
  v0: rules {0}, identity 99c03de0, Pareto 7/16
  v1: rules {0,5}, from v0 by add 5, identity e16eb8f9, Pareto 8/16
  v2: rules {0,3,5}, from v1 by add 3, identity 64dbe9ed, Pareto 9/16
  v3: rules {0,2,3,5}, from v2 by add 2, identity a2c60d02, Pareto 10/16, on the front
  v4: rules {0,2,3,4,5}, from v3 by add 4, identity eb4c2ffb, Pareto 13/16
  v5: rules {0,1,2,3,4,5}, from v4 by add 1, identity 5778acd2, Pareto 15/16, on the front
front: 2 prompts; final: v5 (highest Pareto total)
    You solve tasks from the queue.
    Read the whole task before you answer.
    Check the units before you give a number.
    Count the items one at a time.
    Quote the log line that shows the error.
    Give the shortest reply that is correct.
    Explain each step before the answer.
    Reply with the letter of one option.
held out, 24 items in 12 tasks, 16 runs each: seed 0.25, final 0.7578
  floor 0.1395 from the seed's 4 blocks of 4 runs; gain 0.5729 on block means, 0 of 1000 replicates at or below the floor -> passed (beyond_floor)
  receipts: 1, chain ok
a second read of the held-out partition, for v1: refused (locked)
reply "Sure! Add a rule about units.": parsed false
v5 with its tool grants widened: field
v5 with its threshold lowered: field
grader judge 0 (Elephant, the student's weights): shared
grader judge 4 if its gate had said unverifiable: unproven
the whole run with grader judge 0: refused (shared), spent 0.0
runner sends v4's text when v5 is under test: canary
```

The [example](../examples/prompt-evolution.coax) is compiled only after every check of
`evolve.coax` passes, since the unit is `#authority`.

What the run shows:
- **Seven of twelve steps were refused for drift.** Unclear items fail
  without rule 6, so their failures keep voting for it. With rule 6 the
  student answers "unsure", outside the four options, and the child is
  refused after its batch. A loop that scored "unsure" as merely wrong
  would have measured a different task.
- **The front held two prompts from step 11.** Adding rule 4 helped the
  short items and broke the one explain item among the Pareto items, so
  v4 did not dominate v3, and v3 stayed on the front.
- **The final prompt has a rule that hurts.** v5 keeps rule 4, so it fails
  explain items. The stand-in reflector would propose dropping it only from
  an explain failure, and the budget ran out first.
- **The held-out gain passed the floor.** On 24 held-out items read once,
  the final prompt's gain over the seed is 0.5729, far beyond the floor of
  0.1395. The stand-in world was built so that rules matter; a real
  student's gain could well land inside the floor and be reported as
  unverifiable.

## A prompt is a component

<!-- source: psionic/lib/evolve.coax identity -->
```jai
ev_line :: (buf:[]u8,at:int,s:string)->int {for i:0..<s.count {buf[at+i]=s[i];}buf[at+s.count]=10;return at+s.count+1;}
// The prompt's text: the header, the rules present in library order, and
// the answer line. buf holds EV_TEXT_MAX bytes.
ev_render :: (rules:u8,buf:[]u8)->int {
    at:=ev_line(buf,0,EV_HEADER);
    for k:0..<EV_RULES {if (rules>>cast(u8)k)&1==1 {at=ev_line(buf,at,EV_RULE[k]);}}
    return ev_line(buf,at,EV_ANSWER);
}
ev_ok :: (c:EvCandidate)->bool {return jg_component_ok(c.student) && c.rules<128 && tb_in(c.threshold,0,1);}
// Chapter 239's encoding under its own tag: the student's identity with the
// rendered prompt as its rubric (chapter 246), the tool grants and the
// threshold. Any changed byte of the prompt is a new prompt.
ev_identity :: (c:EvCandidate)->[32]u8 #export_c {
    if !ev_ok(c) {return su_zero();}
    text:[EV_TEXT_MAX]u8;s:=c.student;s.rubric=cast(string)text[0..<ev_render(c.rules,text[..])];
    buf:[80]u8;at:=su_text(buf[..],0,"psionic.prompt.v1");d:=jg_identity(s);
    for i:0..<32 {buf[at+i]=d[i];}
    buf[at+32]=c.tools;at=su_u64(buf[..],at+33,f64_bits(c.threshold));
    return su_hash(buf[0..<at]);
}
// The canary: the identity's first four bytes in hex, sent as the last line.
ev_canary :: (c:EvCandidate)->[8]u8 {
    d:=ev_identity(c);digits:="0123456789abcdef";out:[8]u8;
    for i:0..<4 {out[2*i]=digits[cast(int)(d[i]>>4)];out[2*i+1]=digits[cast(int)(d[i]&15)];}
    return out;
}
ev_send :: (c:EvCandidate,buf:[]u8)->int {
    at:=ev_render(c.rules,buf);w:=ev_canary(c);
    for i:0..<7 {buf[at+i]="canary "[i];}
    for i:0..<8 {buf[at+7+i]=w[i];}
    buf[at+15]=10;return at+16;
}
```

The identity is chapter 246's component identity with the rendered prompt as
the rubric, then the tool grants and the threshold, under the tag
`psionic.prompt.v1`. The rubric field a caller passes is ignored: the rules
are the prompt. The price is not part of the identity, as in chapter 246.
The seed's digest, `99c03de0…`, is checked against Python's `hashlib` over
the same bytes, as are v5's and the digests of the seed with its tools or
threshold changed.

The canary is the identity's first four bytes in hex, sent as the last line
of the text. It is part of what is sent, not part of what is named.

## The stand-in student

<!-- source: psionic/lib/evolve.coax student -->
```jai
// stand-in-student-v1. It reads the text it was sent line by line. Text
// that doesn't start with the header line is not a prompt and is ignored;
// a line equal to a library rule turns that rule on, and a canary line is
// echoed.
ev_read :: (text:string)->EvRead {
    r:EvRead;start:=0;first:=true;
    for c,i:text {
        if c!=10 {continue;}
        line:=text[start..<i];start=i+1;
        if first && line!=EV_HEADER {return EvRead.{};}
        first=false;
        for k:0..<EV_RULES {if line==EV_RULE[k] {r.rules|=cast(u8)(1<<k);}}
        if line.count==15 && line[0..<7]=="canary " {r.seen=true;for j:0..<8 {r.canary[j]=line[7+j];}}
    }
    return r;
}
ev_low :: (mask:u8)->int {
    k:=0;m:=mask;
    while m!=0 {if m&1==1 {return k;}m>>=1;k+=1;}
    return -1;
}
// With every needed rule and no harmful one the student is right when the
// draw u is below 0.9, otherwise below 0.25; a wrong answer is the next
// option. With rule 6 an unclear item gets "unsure", outside the options.
ev_answer :: (r:EvRead,item:EvItem,u:f64)->EvAnswer {
    need:=EV_NEED[item.kind]&(255^r.rules);harm:=EV_CLASH[item.kind]&r.rules;
    a:=EvAnswer.{missing=ev_low(need),clash=ev_low(harm)};
    if item.kind==EV_UNCLEAR && (r.rules>>EV_UNSURE)&1==1 {a.answer=EV_OPTIONS;return a;}
    p:=ifx need==0 && harm==0 then EV_GOOD else EV_POOR;
    a.answer=ifx u<p then item.gold else (item.gold+1)%EV_OPTIONS;
    a.right=a.answer==item.gold;
    return a;
}
// The draw for item i on run r, shared by every prompt so that two prompts
// on one run face the same luck.
ev_draw :: (world:u64,item:int,run:int)->f64 {return tb_uniform(world,cast(u64)(run*EV_ITEMS_MAX+item));}
// The grader compares each answer with the gold one. echoed: the student
// saw a canary and it is the one expected.
ev_measure :: (text:string,expect:[8]u8,items:[]EvItem,which:[]int,world:u64,run:int,out:[]EvAnswer)->EvMeasure {
    r:=ev_read(text);m:=EvMeasure.{echoed=r.seen};
    for j:0..<8 {if r.canary[j]!=expect[j] {m.echoed=false;}}
    for k:0..<which.count {
        a:=ev_answer(r,items[which[k]],ev_draw(world,which[k],run));out[k]=a;
        if a.right {m.right+=1;}
        if a.answer>=EV_OPTIONS {m.unsure+=1;}
    }
    return m;
}
```

The student parses its prompt instead of being handed the rule set, so the
canary check means something: if the runner sends the parent's text, the
student follows the parent's rules and echoes the parent's canary, and the
mismatch is caught. A line counts only when it equals a rule exactly; a
trailing space or a missing newline drops it, and text that doesn't start with
the header line is not read at all.

## The stand-in reflector, and the replies accepted

<!-- source: psionic/lib/evolve.coax reflect -->
```jai
// stand-in-reflector-v1. Each failure votes to add its missing rule and to
// drop its harmful one; a draw u picks a vote, so an edit is chosen in
// proportion to the failures that ask for it. No vote, no reply.
ev_reflect :: (traces:[]EvAnswer,u:f64,reply:[]u8)->int {
    votes:[14]int;total:=0;
    for a:traces {
        if a.right {continue;}
        if a.missing>=0 {votes[a.missing]+=1;total+=1;}
        if a.clash>=0 {votes[EV_RULES+a.clash]+=1;total+=1;}
    }
    if total==0 {return 0;}
    target:=cast(int)(u*f64(total));v:=0;
    while target>=votes[v] {target-=votes[v];v+=1;}
    verb:=EV_VERB[v/EV_RULES];
    for i:0..<verb.count {reply[i]=verb[i];}
    reply[verb.count]=cast(u8)(48+v%EV_RULES);
    return verb.count+1;
}
// The only replies accepted: "add k" or "drop k" with k a rule's number.
ev_parse :: (reply:string)->EvParse {
    n:=reply.count;
    if n<5 {return EvParse.{};}
    op:=ifx n==5 then EvOp.add else EvOp.drop;
    k:=cast(int)reply[n-1]-48;
    if reply[0..<n-1]!=EV_VERB[cast(int)op] || k<0 || k>=EV_RULES {return EvParse.{};}
    return EvParse.{ok=true,edit=EvEdit.{op=op,rule=k}};
}
ev_apply :: (rules:u8,e:EvEdit)->u8 {
    bit:=cast(u8)(1<<e.rule);
    return ifx e.op==EvOp.add then rules|bit else rules&(255^bit);
}
```

A real reflector is a model asked for a better prompt, and a model can reply
with anything. The only replies accepted are `add k` and `drop k` with k a
rule's number. GEPA as written installs an unmatched reply verbatim. Here
"Sure! Add a rule about units." is refused, as are an unknown rule, a
missing number, extra spaces and a change of case. An edit that changes
nothing, such as adding a rule already present, is refused as `no_change`.

## Admission

<!-- source: psionic/lib/evolve.coax admit -->
```jai
// A child keeps the seed's fields: student, price, tools and threshold;
// only its rules differ. Its grader must not share weights with the student
// or the reflector, and must have passed chapter 246's gate.
ev_admit :: (seed:EvCandidate,child:EvCandidate,s:EvSetup)->EvReason #export_c {
    a:=seed.student;b:=child.student;a.rubric=EV_HEADER;b.rubric=EV_HEADER;
    if !ev_ok(child) || !su_same(jg_identity(a),jg_identity(b)) || a.price!=b.price || child.tools!=seed.tools || child.threshold!=seed.threshold {
        return EvReason.field;
    }
    g:=jg_grade(jg_separate(s.grader,b,s.reflector),s.judge,vd(VdVerdict.passed,VdReason.none));
    if g.reason==JgReason.shared {return EvReason.shared;}
    if g.reason==JgReason.unproven {return EvReason.unproven;}
    return EvReason.none;
}
// After the batch: the student echoed the canary, and stayed in the answer space.
ev_checked :: (m:EvMeasure)->EvReason {
    if !m.echoed {return EvReason.canary;}
    if m.unsure>0 {return EvReason.drift;}
    return EvReason.none;
}
```

The checks come in two places. Before the child runs, its fields are
compared with the seed's: the student's model, weights and settings (by
identity with a fixed rubric), its price, its tool grants and its
threshold. A child that widened its tools, lowered its threshold or switched
model is refused as `field`. The grader is checked with chapter 246's
`jg_separate` and `jg_grade`: a grader that shares weights with the student
or the reflector is `shared`, and one whose gate did not pass is
`unproven`. The whole run is refused the same way before anything is spent.

After the child's batch, the student must have echoed the child's canary,
and every answer must be one of the four options.

## The front and the pick

<!-- source: psionic/lib/evolve.coax front -->
```jai
// a dominates b: at least as good on every item and better on one.
ev_dominates :: (a:[]f64,b:[]f64)->bool {
    better:=false;
    for i:0..<a.count {
        if a[i]<b[i] {return false;}
        if a[i]>b[i] {better=true;}
    }
    return better;
}
// Candidate i is on the front when no candidate dominates it and no earlier
// one has the same scores. scores is candidate-major, m items each.
ev_front :: (scores:[]f64,count:int,m:int,keep:[]bool)->int {
    kept:=0;
    for i:0..<count {
        a:=scores[i*m..<(i+1)*m];keep[i]=true;
        for j:0..<count {
            b:=scores[j*m..<(j+1)*m];
            if ev_dominates(b,a) || (j<i && ev_tie(a,b)) {keep[i]=false;}
        }
        if keep[i] {kept+=1;}
    }
    return kept;
}
ev_tie :: (a:[]f64,b:[]f64)->bool {for i:0..<a.count {if a[i]!=b[i] {return false;}}return true;}
// GEPA's per-item Pareto sampling: a front member's weight is the number of
// items on which it is tied-best; u in [0, 1) picks in proportion.
ev_wins :: (scores:[]f64,count:int,m:int,keep:[]bool,wins:[]int)->int {
    total:=0;
    for i:0..<count {
        wins[i]=0;
        if !keep[i] {continue;}
        for t:0..<m {
            top:=true;
            for j:0..<count {if scores[j*m+t]>scores[i*m+t] {top=false;}}
            if top {wins[i]+=1;}
        }
        total+=wins[i];
    }
    return total;
}
ev_pick :: (wins:[]int,total:int,u:f64)->int {
    target:=cast(int)(u*f64(total));i:=0;
    while target>=wins[i] {target-=wins[i];i+=1;}
    return i;
}
// The final prompt: the highest total on the Pareto items; the earlier wins a tie.
ev_sum :: (v:[]f64)->f64 {t:=0.0;for x:v {t+=x;}return t;}
ev_best :: (scores:[]f64,count:int,m:int)->int {
    best:=0;top:=ev_sum(scores[0..<m]);
    for i:1..<count {
        t:=ev_sum(scores[i*m..<i*m+m]);
        if t>top {top=t;best=i;}
    }
    return best;
}
```

A front member's weight counts the items on which no prompt scores higher,
including items where every prompt is at 0. The final prompt is the one
with the highest total on the Pareto items, the earlier on a tie; it is
written down before the held-out partition is read.

## The loop

<!-- source: psionic/lib/evolve.coax loop -->
```jai
ev_index_ok :: (v:[]int,n:int)->bool {for x:v {if x<0 || x>=n {return false;}}return true;}
ev_setup_ok :: (seed:EvCandidate,s:EvSetup,items:[]EvItem,dev:[]int,feedback:[]int,pool:[]EvCandidate,scores:[]f64,steps:[]EvStep)->EvStatus {
    n:=items.count;
    if n<1 || n>EV_ITEMS_MAX || dev.count<1 || feedback.count<1 || !ev_index_ok(dev,n) || !ev_index_ok(feedback,n) {return EvStatus.shape;}
    for x:items {if x.kind<0 || x.kind>=EV_KINDS || x.gold<0 || x.gold>=EV_OPTIONS {return EvStatus.shape;}}
    if s.batch<1 || s.batch>EV_BATCH_MAX || s.batch>feedback.count || pool.count<1 || pool.count>EV_POOL_MAX {return EvStatus.shape;}
    if scores.count<pool.count*dev.count || steps.count>EV_STEPS_MAX {return EvStatus.shape;}
    if !ev_ok(seed) || !jg_component_ok(s.grader) || !jg_component_ok(s.reflector) {return EvStatus.shape;}
    if !tb_in(s.budget,0,EV_BIG) {return EvStatus.number;}
    return EvStatus.ok;
}
// One answer costs a student call and a grade; a step's worst case is two
// batches, one reflection and a full pass over the Pareto items.
ev_unit :: (seed:EvCandidate,s:EvSetup)->f64 {return jg_cost(seed.student,false)+jg_cost(s.grader,false);}
// A prompt sent with its canary and scored 1 or 0 per item on run r.
ev_score :: (c:EvCandidate,items:[]EvItem,which:[]int,world:u64,run:int,out:[]f64)->int {
    text:[EV_TEXT_MAX]u8;answers:[EV_ITEMS_MAX]EvAnswer;
    m:=ev_measure(cast(string)text[0..<ev_send(c,text[..])],ev_canary(c),items,which,world,run,answers[..]);
    for k:0..<which.count {out[k]=ifx answers[k].right then 1.0 else 0.0;}
    return m.right;
}
ev_evolve :: (seed:EvCandidate,s:EvSetup,items:[]EvItem,dev:[]int,feedback:[]int,pool:[]EvCandidate,scores:[]f64,steps:[]EvStep)->EvRun #export_c {
    st:=ev_setup_ok(seed,s,items,dev,feedback,pool,scores,steps);
    if st!=EvStatus.ok {return EvRun.{status=st};}
    first:=ev_admit(seed,seed,s);
    if first!=EvReason.none {return EvRun.{status=EvStatus.refused,reason=first};}
    m:=dev.count;unit:=ev_unit(seed,s);full:=f64(m)*unit;b:=s.batch;
    if full>s.budget {return EvRun.{status=EvStatus.refused,reason=EvReason.budget};}
    pool[0]=seed;pool[0].parent=-1;
    r:=EvRun.{seed=ev_score(seed,items,dev,s.world,0,scores[0..<m]),candidates=1,spend=full};
    worst:=2*f64(b)*unit+jg_cost(s.reflector,false)+full;
    keep:[EV_POOL_MAX]bool;wins:[EV_POOL_MAX]int;which:[EV_BATCH_MAX]int;
    pa:[EV_BATCH_MAX]EvAnswer;ca:[EV_BATCH_MAX]EvAnswer;text:[EV_TEXT_MAX]u8;reply:[8]u8;
    while r.steps<steps.count {
        if r.spend+worst>s.budget {r.reason=EvReason.budget;break;}
        if r.candidates==pool.count {r.reason=EvReason.full;break;}
        t:=r.steps;n:=r.candidates;
        x:=EvStep.{front=ev_front(scores,n,m,keep[..]),child=-1,reason=EvReason.perfect};total:=ev_wins(scores,n,m,keep[..],wins[..]);
        p:=ev_pick(wins[0..<n],total,tb_uniform(s.draw,cast(u64)(2*t)));
        for j:0..<b {which[j]=feedback[(t*b+j)%feedback.count];}
        parent:=pool[p];
        pm:=ev_measure(cast(string)text[0..<ev_send(parent,text[..])],ev_canary(parent),items,which[0..<b],s.world,1+t,pa[..]);
        x.parent=p;x.parent_right=pm.right;r.spend+=f64(b)*unit;
        if pm.right<b {
            r.spend+=jg_cost(s.reflector,false);
            e:=ev_parse(cast(string)reply[0..<ev_reflect(pa[0..<b],tb_uniform(s.draw,cast(u64)(2*t+1)),reply[..])]);
            child:=parent;child.parent=p;child.edit=e.edit;child.rules=ev_apply(parent.rules,e.edit);x.edit=e.edit;
            x.reason=ifx !e.ok then EvReason.silent else ifx child.rules==parent.rules then EvReason.no_change else ev_admit(seed,child,s);
            if x.reason==EvReason.none {
                cm:=ev_measure(cast(string)text[0..<ev_send(child,text[..])],ev_canary(child),items,which[0..<b],s.world,1+t,ca[..]);
                r.spend+=f64(b)*unit;x.child_right=cm.right;x.reason=ev_checked(cm);
                if x.reason==EvReason.none && cm.right<=pm.right {x.reason=EvReason.no_gain;}
                if x.reason==EvReason.none {
                    pool[n]=child;x.dev=ev_score(child,items,dev,s.world,0,scores[n*m..<n*m+m]);
                    r.spend+=full;x.child=n;x.reason=EvReason.accepted;r.candidates+=1;
                }
            }
        }
        x.spend=r.spend;steps[t]=x;r.steps+=1;
    }
    r.best=ev_best(scores,r.candidates,m);
    return r;
}
```

Step t draws its parent with draw 2t and the reflector's pick with draw
2t + 1, both from the setup's seed, so a run is reproducible. The batches
cycle through the feedback items in order. A prompt on the Pareto items is
scored on run 0; parent and child on a batch share run t + 1. The run stops
when the next step's worst case would pass the budget, when the pool is
full, or when the step log is.

## One read of the held-out partition

<!-- source: psionic/lib/evolve.coax held -->
```jai
// The held-out partition, read once: seed and final prompt on the same
// draws for runs 1000 on, the floor from the seed's runs in blocks, chapter
// 241's decision on the block means, and one receipt in chapter 239's chain.
// A second read of a locked partition, or a full store, is refused before
// any result is shown.
ev_held :: (suite:SuSuite,part:int,store:[]SuReceipt,count:int,seed:EvCandidate,final:EvCandidate,items:[]EvItem,held:[]int,world:u64,runs:int,block:int)->EvHeld #export_c {
    n:=held.count;
    if n<1 || n>EV_ITEMS_MAX || !ev_index_ok(held,items.count) || runs<2 || runs>EV_RUNS_MAX || block<1 || runs%block!=0 || !ev_ok(seed) || !ev_ok(final) ||
        suite.status!=SuStatus.ok || part<0 || part>=SU_PARTITIONS || count<0 || count>store.count {
        return EvHeld.{status=EvStatus.shape};
    }
    a:[EV_RUNS_MAX*EV_ITEMS_MAX]f64;b:[EV_RUNS_MAX*EV_ITEMS_MAX]f64;task:[EV_ITEMS_MAX]int;zero:[EV_ITEMS_MAX]int;
    h:EvHeld;
    for r:0..<runs {
        h.seed+=f64(ev_score(seed,items,held,world,EV_HELD_RUN+r,a[r*n..<r*n+n]));
        h.final+=f64(ev_score(final,items,held,world,EV_HELD_RUN+r,b[r*n..<r*n+n]));
    }
    h.seed/=f64(runs*n);h.final/=f64(runs*n);
    // Each item's mean over the first block of runs, for seed and final.
    ma:[EV_ITEMS_MAX]f64;mb:[EV_ITEMS_MAX]f64;
    for k:0..<n {
        task[k]=items[held[k]].task;
        for r:0..<block {ma[k]+=a[r*n+k];mb[k]+=b[r*n+k];}
        ma[k]/=f64(block);mb[k]/=f64(block);
    }
    h.floor=ns_floor(a[0..<runs*n],zero[0..<n],0,runs,block);
    h.decision=vd_decision(ma[0..<n],mb[0..<n],task[0..<n],zero[0..<n],0,h.floor,1,world,1000);
    app:=su_append(store,count,su_result(suite,part,jg_seed(ev_identity(final)),h.decision.gain.gain));
    if app.status!=SuStatus.ok {
        return EvHeld.{status=EvStatus.refused,reason=ifx app.status==SuStatus.locked then EvReason.locked else EvReason.full,receipts=count};
    }
    h.receipts=app.count;
    return h;
}
```

The seed and the final prompt run 16 times on the 24 held-out items, on the
same draws. The floor comes from the seed's runs in 4 blocks of 4. Each
item's mean over the first block is the score chapter 241's decision gate
compares, with whole tasks drawn again by the bootstrap. The result is
appended as one receipt naming the final prompt; the partition is locked,
so a second read, for any prompt, is refused before its result is shown.
A full store is refused the same way.

## A broken input

The example's last lines are refusals. A reply that isn't an edit doesn't
parse; a child with widened tools or a lowered threshold is a changed field;
an Elephant grader shares the student's weights; a grader whose gate said
unverifiable is unproven; the whole run with an Elephant grader is refused
before it spends anything; and a runner that sends v4's text while v5 is
under test fails the canary:

```text
reply "Sure! Add a rule about units.": parsed false
v5 with its tool grants widened: field
v5 with its threshold lowered: field
grader judge 0 (Elephant, the student's weights): shared
grader judge 4 if its gate had said unverifiable: unproven
the whole run with grader judge 0: refused (shared), spent 0.0
runner sends v4's text when v5 is under test: canary
```

The example exits with status 0 after printing them. The checks also refuse
a run with no items or more than 64, an index outside the items, an item
whose kind or gold answer is out of range, a batch of 0, more than 8 or more
than the feedback items, a pool of 0 or more than 16 prompts, too little
score storage, more than 64 steps, a malformed seed, grader or reflector, a
budget that is negative or not a number, and a held-out read with no items,
fewer than 2 or more than 16 runs, a block that doesn't divide the runs, a
broken suite or a partition or store position out of range.

## Supported profile

`evolve-v1`:
- binary64;
- a seven-rule library, four options an item, up to 64 items, 16 prompts in
  the pool, 8 items a batch, 64 steps and 16 held-out runs;
- identities under the tag `psionic.prompt.v1` over chapter 246's
  `psionic.component.v1`, in chapter 239's encoding;
- chapter 103's counter-based draws;
- chapter 240's floor, chapter 241's decision gate with 1,000 replicates,
  and chapter 239's suite, receipt chain and locked partition;
- stand-in-student-v1 and stand-in-reflector-v1 as written above.

## How it is checked

- **Coax checks:** the chapter's 14 checks, among the 249 that run in debug
  and release with the units `evolve.coax` loads. They cover:
  - the seed's, v5's and two changed-field identities against digests
    computed with Python's `hashlib`, the price and rubric not changing it,
    and the rendered and sent text byte for byte;
  - the student's reading of lines and canaries, and its answers at each
    chance boundary by hand;
  - the step-1 batch above, a stale text failing the canary, a canary wrong
    in its last byte, and rule 6 drifting the answer space;
  - the reflector's votes and picks by hand, and eleven refused replies;
  - every admission refusal, field by field;
  - the front, weights and pick by hand, and a `#forall` over all 6,561
    four-prompt, two-item cases with scores 0 to 2 against an independent
    sort-and-sweep oracle ([evolve-checks.coax](../tests/evolve-checks.coax));
  - the example's run step by step; a step needing no reflection, one with
    no vote and a child with no gain; a step whose worst case exactly fits;
    and a `#forall` over twelve budgets that
    spending never passes the budget and equals a recount by outcome;
  - the held-out read: the means, the floor's blocks, the verdict, the
    receipt and its chain, the gain recomputed from the items, the second
    read refused, and every shape refusal.
- **Mutation testing** of `evolve.coax` (its own sites, in four shards): 609
  of 609 mutants killed. The first run left 23 survivors: text slices whose
  first byte the student never needed, a missing rule 0 and a vote weight
  that no check reached, a pool limit hidden by a short score buffer, call
  prices whose swap setting no check set, a worst case that no budget
  matched exactly, an edit nobody read back, and a best-total search that
  never kept its running maximum. The student now requires the header line,
  and the checks cover the rest.

These checks ran with the cached `coax` compiler.

## What this does not do yet

- **A served student and reflector.** Both are stand-ins. A real student is
  Elephant or Mammoth with the prompt as its instructions, and a real
  reflector is a separately pinned model served by Psionic, such as gpt-oss
  from [chapter 64](ch064-execute-gpt-oss-20b.md), whose reply is
  constrained by [chapter 53](ch053-choosing-the-next-token.md)'s sampler to
  the edit grammar. The admission, front, budget and held-out code takes
  them unchanged.
- **Free-text edits.** The reflector edits a fixed library of rules, so a
  prompt can't grow text nobody wrote down. GEPA proposes free text; that
  needs the served reflector and a grammar for the fields it may change.
- **The comparison with settings fitting.** The draft planned to report
  GEPA's gain beside [chapter 243](ch243-fit-settings-on-recorded-answers.md)'s
  on the same held-out partition. Chapter 243 fits a stall detector, not a
  prompt, so there is no shared task yet to compare them on.
- **Merging.** GEPA can merge two front members; this loop only edits one.

## Sources

- [Chapter plan](PLAN.md), Part XXIV.
- [Measure, then improve](../docs/self-improvement.md).
- [NIP-OPT](../../nostr/nips/openagents/NIP-OPT.md).
- Agrawal et al., "GEPA: Reflective Prompt Evolution Can Outperform
  Reinforcement Learning", 2025.
