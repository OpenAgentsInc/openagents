# 81. Decisions, evaluations, optimization and programs

Agents make judgments, and people need to know how good those judgments
are. Four OpenAgents NIPs carry that loop. NIP-DEC asks a model typed
questions about one state and gets probabilities back. NIP-EVAL records
how a model, program or whole agent did on a pinned suite of cases.
NIP-OPT searches for a better implementation of the same task without
letting the search see the cases that will judge it. NIP-PRG composes
decisions, checks and operations into a typed workflow. This chapter
builds the checks that stop a wrong record from counting: an answer that
answers some other question set, a report about a suite the reader cannot
see, an admission that leans on evidence the reader cannot count, and a
study whose held-out cases leak into its search.

Part X, Every NIP. Result: NIP-DEC body bounds and answer admission (an
answer must match its request's question set exactly), NIP-EVAL reports
as one reader sees them (verified, unverifiable or untrusted) and the
admission that cites them, and NIP-OPT's data-plan isolation. NIP-PRG,
the 25910 wire job, the 3189 publication event and most of NIP-OPT's
records are described and not built; see [What is not built](#what-is-not-built).

## What this chapter teaches

- DEC: the three question types (noul, choice, score), EntryType, the
  bounds a worker checks before dispatch, and the exact shape of an
  answer: every question answered with its own type, probabilities over
  exactly the options asked, the choice and confidence fixed by the
  probabilities.
- Why a decision and an HTTP call with the same body are the same
  decision, and what a decision-call record's two digests bind.
- EVAL: suite identity, reports with coverage that adds up, comparisons
  that need a baseline, and why a report about a suite the reader does not
  hold is shown as an unverified claim, not as evidence.
- How one reader weighs several evaluators: trusted keys, held suites,
  and an admission that cites only what the reader can count.
- OPT: semantic signatures, immutable implementations, study plans, and
  why confirmation cases stay out of search and selection.
- PRG: definitions, steps, the binding language and Wasm modules, in
  outline.

## Where the code is

| File | What |
| --- | --- |
| [coax/lib/nostr/dec/answer.coax](../../coax/lib/nostr/dec/answer.coax) | `dec_body` (a decision body's bounds) and `dec_answer` (an answer against its question set), `#authority` |
| [coax/lib/nostr/eval/report.coax](../../coax/lib/nostr/eval/report.coax) | `eval_report` (what a reader may show of a report) and `eval_admit` (an eval-admission record against the reports read), `#authority` |
| [coax/lib/nostr/opt/plan.coax](../../coax/lib/nostr/opt/plan.coax) | `opt_partition`, `opt_plan` and `opt_trial`: an EVAL partition, a study's data plan, a trial's phase, `#authority` |
| [coax/lib/nostr/ctrl/fingerprint.coax](../../coax/lib/nostr/ctrl/fingerprint.coax) | `ctrl_fingerprint`, reused for the decision-call record's state and question digests (chapter 76) |
| [coax/lib/agent/claude/fields.coax](../../coax/lib/agent/claude/fields.coax) | The JSON field reader all three files use, over [json.coax](../../coax/lib/nostr/json.coax) |
| [coax/tests/nostr/dec_eval_vectors.py](../../coax/tests/nostr/dec_eval_vectors.py) | The Python generator of the bodies, reports, admissions and digests |
| [coax/tests/nostr/data/decisions/](../../coax/tests/nostr/data/decisions/) | The retired Rust crate's own decision fixtures, kept as an independent check |
| [nostr/examples/decisions-evals.coax](../examples/decisions-evals.coax) | The worked example |
| [nostr/examples/checks/decisions-evals.coax](../examples/checks/decisions-evals.coax) | Its checks |

The repository already had decision code before this chapter:
`coax/lib/systemone/` builds System One request bodies and reads served
answers for Coder, and `psionic/lib/decision-*.coax` computes answers
inside Psionic's own model. Both serve a particular door. The files here
are the protocol's side: what any reader of a NIP-DEC answer or NIP-EVAL
report must refuse, whoever served it.

## Four NIPs, one loop

A decision is one judgment. An evaluation measures many judgments
against labeled cases. An optimization study proposes other
implementations and measures them the same way. A program is where a
judgment is used, in a `decide` step beside checks and operations. Each
NIP keeps one rule from the others: a probability is never permission,
a score is never admission, a selected candidate is never promoted by
being selected, and a program grants no authority by naming a step.

None of the four allocates authority. A NIP-DEC answer records a
judgment; the consumer pins its own threshold. A NIP-EVAL admission
admits one exact subject for one scope and grants no execution. A
NIP-OPT result names a candidate; adoption still needs EVAL admission
and a release. That is why the code in this chapter decides only what a
reader may show and count, never what an agent may do.

## DEC: a typed question set

A decision body is `{model, state, questions}`. The state is a string or
a JSON object; every question reads the same state. `questions` maps a
caller's question IDs to questions of three types:

| Type | `criteria` | Answer |
| --- | --- | --- |
| `noul` | optional `{true, false}` descriptions, or a legacy string | `noul`, the probability of yes |
| `choice` | required: option ID to description, 1 to 255 options | `choice`, `confidence`, `probabilities` |
| `score` | required: 2 to 10 ordered level descriptions | `score`, `confidence`, `legend`, `probabilities` |

Descriptions are EntryTypes: a string, an object, an array or `null`,
never a bare number or boolean, nested at most 8 levels. A worker
refuses a body past its bounds before any model sees it, with the NIP's
codes: `invalid_request`, `too_many_questions` above 64,
`too_many_options` above 255 options or 10 levels.

<!-- source: coax/lib/nostr/dec/answer.coax codes -->
```jai
DEC_OK :: cast(u32)0;
DEC_MALFORMED :: cast(u32)1;          // not a JSON object, or no questions or answers object
DEC_INVALID :: cast(u32)2;            // invalid_request: the body breaks a bound or a shape
DEC_TOO_MANY_QUESTIONS :: cast(u32)3; // too_many_questions: more than 64
DEC_TOO_MANY_OPTIONS :: cast(u32)4;   // too_many_options: more than 255 options or 10 levels
DEC_UNANSWERED :: cast(u32)5;         // a question has no answer
DEC_EXTRA :: cast(u32)6;              // an answer to no question
DEC_TYPE :: cast(u32)7;               // an answer of another type than its question
DEC_PROBABILITY :: cast(u32)8;        // a probability or confidence not a number in [0, 1]
DEC_OPTIONS :: cast(u32)9;            // probabilities not naming exactly the options or levels
DEC_SUM :: cast(u32)10;               // categorical probabilities not summing to one
DEC_CHOICE :: cast(u32)11;            // the choice is not the most probable option
DEC_CONFIDENCE :: cast(u32)12;        // confidence is not the chosen or largest probability
DEC_SCORE :: cast(u32)13;             // the score is not the probability-weighted index
DEC_LEGEND :: cast(u32)14;            // the legend does not name exactly the level indexes

DEC_MAX_QUESTIONS :: 64; DEC_QUESTION_ID :: 64; DEC_MODEL :: 128;
DEC_MAX_OPTIONS :: 255; DEC_OPTION_ID :: 128; DEC_MIN_LEVELS :: 2; DEC_MAX_LEVELS :: 10;
DEC_ENTRY_DEPTH :: 8; DEC_STATE_BYTES :: 131072; DEC_QUESTION_BYTES :: 65536;
DEC_SLACK :: 0.000001;
DEC_NOUL :: 1; DEC_CHOICE_KIND :: 2; DEC_SCORE_KIND :: 3;
```

The helpers read keys decoded, so an ID written with a `\u` escape is the
same ID; `dec_keys` checks every key of an object in one pass, which
keeps a 255-option choice linear to read.

<!-- source: coax/lib/nostr/dec/answer.coax helpers -->
```jai
// Within DEC_SLACK of b.
dec_near :: (a:f64,b:f64)->bool {return a>=b-DEC_SLACK && a<=b+DEC_SLACK;}
dec_unit :: (p:JsonF64)->bool {return p.ok && p.value>=0.0 && p.value<=1.0;}
// a before b in byte order.
dec_less :: (a:string,b:string)->bool {
  for i:0..<a.count {
    if i==b.count {return false;}
    if a[i]<b[i] {return true;}
    if a[i]>b[i] {return false;}
  }
  return a.count<b.count;
}
// Whether v nests at most most levels: a string or null 0, each object or
// array one more.
dec_within :: (text:string,v:ClSlice,most:int)->bool {
  j:[1]Json;cl_parse(j[..],cl_json(text,v));
  while json_next(j[..])>JSON_END {
    if j[0].depth>most {return false;}
  }
  return true;
}
// An EntryType within its depth: a string, an object, an array or null.
dec_entry :: (text:string,v:ClSlice)->bool {
  if v.tok!=JSON_STRING && v.tok!=JSON_OBJECT && v.tok!=JSON_ARRAY && v.tok!=JSON_NULL {return false;}
  return dec_within(text,v,DEC_ENTRY_DEPTH);
}
// Member i's key of the object v, decoded into buf (borrowed when plain).
dec_key :: (text:string,v:ClSlice,i:int,buf:[]u8)->string {
  inner:=text[v.from..<v.to];ar:[1]ClArena;ar[0].buf=buf;
  return cl_text(ar[..],inner,cl_key(inner,i));
}
dec_count :: (text:string,v:ClSlice)->int {return cl_count(text[v.from..<v.to]);}
dec_kind :: (text:string,q:ClSlice)->int {
  t:=cl_member(text,q,"type");
  if cl_eq(text,t,"noul") {return DEC_NOUL;}
  if cl_eq(text,t,"choice") {return DEC_CHOICE_KIND;}
  if cl_eq(text,t,"score") {return DEC_SCORE_KIND;}
  return 0;
}
// Whether the object v's keys, at most DEC_MAX_OPTIONS of them, are
// distinct and each 1 to most bytes: one pass, the keys decoded side by side.
dec_keys :: (text:string,v:ClSlice,most:int)->bool {
  inner:=text[v.from..<v.to];j:[1]Json;cl_parse(j[..],inner);
  arena:[40000]u8;ar:[1]ClArena;ar[0].buf=arena[..];
  keys:[DEC_MAX_OPTIONS]string;n:=0;
  opened:=json_next(j[..]);
  while json_next(j[..])==JSON_STRING {
    k:=cl_text(ar[..],inner,ClSlice.{ok=true,tok=JSON_STRING,from=j[0].from,to=j[0].to});
    if k.count<1 || k.count>most {return false;}
    for i:0..<n {if keys[i]==k {return false;}}
    keys[n]=k;n+=1;
    value:=json_next(j[..]);skipped:=json_skip(j[..]);
  }
  return true;
}
```

`dec_question` checks one question by its type and `dec_body` the whole
body. A refusal names the question it found at fault.

<!-- source: coax/lib/nostr/dec/answer.coax question -->
```jai
// What is wrong with question q, or DEC_OK.
dec_question :: (text:string,q:ClSlice)->u32 {
  if q.to-q.from>DEC_QUESTION_BYTES {return DEC_INVALID;}
  kind:=dec_kind(text,q);
  if kind==0 {return DEC_INVALID;}
  ins:=cl_member(text,q,"instructions");
  if ins.ok && !dec_entry(text,ins) {return DEC_INVALID;}
  c:=cl_member(text,q,"criteria");
  if kind==DEC_NOUL {
    if !c.ok || c.tok==JSON_NULL || c.tok==JSON_STRING {return DEC_OK;}
    if c.tok!=JSON_OBJECT {return DEC_INVALID;}
    yes:=0;no:=0;
    for i:0..<dec_count(text,c) {
      b:[8]u8;k:=dec_key(text,c,i,b[..]);
      if k=="true" {yes+=1;} else if k=="false" {no+=1;} else {return DEC_INVALID;}
      if !dec_entry(text,cl_element(text,c,i)) {return DEC_INVALID;}
    }
    if yes>1 || no>1 {return DEC_INVALID;}
    return DEC_OK;
  }
  if kind==DEC_CHOICE_KIND {
    if c.tok!=JSON_OBJECT {return DEC_INVALID;}
    n:=dec_count(text,c);
    if n>DEC_MAX_OPTIONS {return DEC_TOO_MANY_OPTIONS;}
    if n<1 || !dec_keys(text,c,DEC_OPTION_ID) {return DEC_INVALID;}
    for i:0..<n {if !dec_entry(text,cl_element(text,c,i)) {return DEC_INVALID;}}
    return DEC_OK;
  }
  if c.tok!=JSON_ARRAY {return DEC_INVALID;}
  n:=dec_count(text,c);
  if n>DEC_MAX_LEVELS {return DEC_TOO_MANY_OPTIONS;}
  if n<DEC_MIN_LEVELS {return DEC_INVALID;}
  for i:0..<n {if !dec_entry(text,cl_element(text,c,i)) {return DEC_INVALID;}}
  return DEC_OK;
}

// What is wrong with the decision body, or DEC_OK; question is the index
// of the question at fault.
dec_body :: (text:string)->DecCheck {
  root:=cl_root(text);
  if !root.ok || root.tok!=JSON_OBJECT {return dec_at(DEC_MALFORMED,-1);}
  model:=cl_member(text,root,"model");
  if model.ok && (model.tok!=JSON_STRING || model.to-model.from<1 || model.to-model.from>DEC_MODEL) {return dec_at(DEC_INVALID,-1);}
  state:=cl_member(text,root,"state");
  if !state.ok || (state.tok!=JSON_STRING && state.tok!=JSON_OBJECT) || state.to-state.from>DEC_STATE_BYTES {return dec_at(DEC_INVALID,-1);}
  qs:=cl_member(text,root,"questions");
  if !qs.ok || qs.tok!=JSON_OBJECT {return dec_at(DEC_MALFORMED,-1);}
  n:=dec_count(text,qs);
  if n>DEC_MAX_QUESTIONS {return dec_at(DEC_TOO_MANY_QUESTIONS,-1);}
  if n<1 || !dec_keys(text,qs,DEC_QUESTION_ID) {return dec_at(DEC_INVALID,-1);}
  for i:0..<n {
    code:=dec_question(text,cl_element(text,qs,i));
    if code!=DEC_OK {return dec_at(code,i);}
  }
  return dec_at(DEC_OK,-1);
}
```

The body this code reads has the same shape at every door. The HTTP
gateway's `POST /v1/systemone` takes it as its JSON body; a 25910 job
carries the same `model`, `state` and `questions` with the job's own
`request`, `attempt` and `deadline`, encrypted with NIP-44 to the worker.
"A decision job and an HTTP call with the same body ask the same thing."
The job's tags are NIP-CJ's (chapter 80's `cj_answer` admits the
result's signer, `p` and `e`); this file reads the body inside.

## DEC: an answer to exactly that set

A completed result carries `response.answers`, keyed exactly as the
questions were. The NIP fixes every number in an answer from the
probabilities: categorical probabilities sum to one within 0.000001; a
choice is a maximum-probability option, ties to the lexicographically
least ID, and its confidence is that option's probability; a score is
the probability-weighted level index, its confidence the largest level
probability, and its legend maps the same indexes.

<!-- source: coax/lib/nostr/dec/answer.coax answer -->
```jai
// What is wrong with answer a (in at) to question q (in qt), or DEC_OK.
dec_one :: (qt:string,q:ClSlice,at:string,a:ClSlice)->u32 {
  kind:=dec_kind(qt,q);
  type:=cl_member(at,a,"type");
  if kind==DEC_NOUL {
    if !cl_eq(at,type,"noul") {return DEC_TYPE;}
    if !dec_unit(cl_num_of(at,cl_member(at,a,"noul"))) {return DEC_PROBABILITY;}
    return DEC_OK;
  }
  if (kind==DEC_CHOICE_KIND && !cl_eq(at,type,"choice")) || (kind==DEC_SCORE_KIND && !cl_eq(at,type,"score")) {return DEC_TYPE;}
  c:=cl_member(qt,q,"criteria");n:=dec_count(qt,c);
  probs:=cl_member(at,a,"probabilities");
  if dec_count(at,probs)!=n {return DEC_OPTIONS;}
  sum:=0.0;weighted:=0.0;best:int;most:=-1.0;
  for i:0..<n {
    b:[256]u8;name:=dec_key(qt,c,i,b[..]);
    if kind==DEC_SCORE_KIND {b[0]=cast(u8)('0'+i);name=cast(string)b[0..<1];}
    p:=cl_member(at,probs,name);
    if !p.ok {return DEC_OPTIONS;}
    v:=cl_num_of(at,p);
    if !dec_unit(v) {return DEC_PROBABILITY;}
    sum+=v.value;weighted+=cast(f64)i*v.value;
    w:[256]u8;
    if v.value>most || (v.value==most && kind==DEC_CHOICE_KIND && dec_less(name,dec_key(qt,c,best,w[..]))) {most=v.value;best=i;}
  }
  if !dec_near(sum,1.0) {return DEC_SUM;}
  confidence:=cl_num_of(at,cl_member(at,a,"confidence"));
  if kind==DEC_CHOICE_KIND {
    w:[256]u8;ar:[1]ClArena;ar[0].buf=w[..];x:[256]u8;
    chosen:=cl_member(at,a,"choice");
    if chosen.tok!=JSON_STRING || cl_text(ar[..],at,chosen)!=dec_key(qt,c,best,x[..]) {return DEC_CHOICE;}
  } else {
    if !dec_near(cl_num_of(at,cl_member(at,a,"score")).value,weighted) || !cl_num_of(at,cl_member(at,a,"score")).ok {return DEC_SCORE;}
    legend:=cl_member(at,a,"legend");
    if dec_count(at,legend)!=n {return DEC_LEGEND;}
    for i:0..<n {b:[1]u8;b[0]=cast(u8)('0'+i);if !cl_member(at,legend,cast(string)b[..]).ok {return DEC_LEGEND;}}
  }
  if !dec_unit(confidence) {return DEC_PROBABILITY;}
  if !dec_near(confidence.value,most) {return DEC_CONFIDENCE;}
  return DEC_OK;
}

// What is wrong with response as the answer to body, or DEC_OK. A body
// that dec_body refuses refuses here with the same code.
dec_answer :: (body:string,response:string)->DecCheck {
  check:=dec_body(body);
  if check.code!=DEC_OK {return check;}
  root:=cl_root(response);
  answers:=cl_member(response,root,"answers");
  if answers.tok!=JSON_OBJECT {return dec_at(DEC_MALFORMED,-1);}
  qs:=cl_member(body,cl_root(body),"questions");n:=dec_count(body,qs);
  for i:0..<n {
    b:[256]u8;a:=cl_member(response,answers,dec_key(body,qs,i,b[..]));
    if !a.ok || a.tok!=JSON_OBJECT {return dec_at(DEC_UNANSWERED,i);}
    code:=dec_one(body,cl_element(body,qs,i),response,a);
    if code!=DEC_OK {return dec_at(code,i);}
  }
  if dec_count(response,answers)!=n {return dec_at(DEC_EXTRA,-1);}
  return dec_at(DEC_OK,-1);
}
```

The order matters for what the caller learns. A missing question is
found first, then each answer's type, its probabilities' range, their
keys, their sum, and only then the choice, score and confidence. An
answer to a question nobody asked is found last, once every asked
question has been answered.

One finding from writing this. `coax/lib/systemone/answers.coax` reads
the answers a System One door serves today, and documents a choice's
confidence as `(max p - 1/k) / (1 - 1/k)`, a concentration. NIP-DEC says
the confidence is the chosen option's probability. `dec_answer` follows
the pinned NIP, so it refuses such an answer as
`confidence is not that probability`; the example shows one. Either the
door or the NIP has to change; this chapter does not pick.

### What a decision record binds

When a program or agent makes a decision it records an
`openagents.decision-call.v1` call in its trajectory with `state_digest`
and `questions_digest`, SHA-256 of the canonical (JCS) state and
questions. The example computes both with chapter 76's
`ctrl_fingerprint`, and Python computes them independently. A record
whose digests match a body binds the answer it carries to exactly that
state and those questions; changing a word of any instruction changes
the questions digest.

## EVAL: reports one reader can check

A suite pins its purpose, workload, cases, partition, labels, metrics,
acceptance policy and environment, each by digest. Its partition splits
case IDs into `development`, `held_out` and `excluded`, with no overlap.
A report names the suite and partition by ArtifactRef, the subject (and
a baseline or null), the evaluator's key, ordered times, the runs,
coverage per arm, measurements and a verdict. For each arm the five
terminal outcomes (completed, refused, failed, cancelled, unknown) sum to
the attempts; a measurement's arm is the subject, the baseline or the
comparison between them, and the last two need a baseline.

The NIP says what a reader does when it cannot see the evidence: "label
the result an unverified claim at that reader's assurance level, never
independently reproduced evidence." Users also choose their evaluators;
"conflicting reports remain separate; no latest-event rule picks a
universally best model." So `eval_report` answers one of three ways to
show a well-formed report, and refuses an ill-formed one.

<!-- source: coax/lib/nostr/eval/report.coax codes -->
```jai
EV_VERIFIED :: cast(u32)0;     // shown: suite and partition held, evaluator trusted
EV_UNVERIFIABLE :: cast(u32)1; // shown as unverifiable: a suite or partition digest not held
EV_UNTRUSTED :: cast(u32)2;    // shown as untrusted: an evaluator this reader has not chosen
EV_MALFORMED :: cast(u32)3;    // not an object, or a field missing or of the wrong type
EV_VERSION :: cast(u32)4;      // not openagents.eval-report.v1
EV_REF :: cast(u32)5;          // an ArtifactRef without a sha256 Digest and a size
EV_TIME :: cast(u32)6;         // ended before it started
EV_COVERAGE :: cast(u32)7;     // counts that do not sum, or an arm without its baseline
EV_MEASUREMENT :: cast(u32)8;  // an arm, value, denominator or unknown count out of shape
EV_VERDICT :: cast(u32)9;      // not pass, fail or inconclusive

EV_PASS :: 1; EV_FAIL :: 2; EV_INCONCLUSIVE :: 3;   // verdicts

EA_OK :: cast(u32)0;           // the admission stands
EA_MALFORMED :: cast(u32)1;    // not an eval-admission.v1 body of the right shape
EA_ISSUER :: cast(u32)2;       // issued by a key this reader does not trust to admit
EA_EXPIRED :: cast(u32)3;      // at or past expires_at
EA_VALIDATION :: cast(u32)4;   // an admit citing no validation
EA_UNSEEN :: cast(u32)5;       // an admit citing a report this reader has not read
EA_UNVERIFIED :: cast(u32)6;   // an admit citing a report not shown as verified
EA_SUBJECT :: cast(u32)7;      // an admit citing a report about another subject
EA_VERDICT :: cast(u32)8;      // an admit citing a report whose verdict is not pass
```

<!-- source: coax/lib/nostr/eval/report.coax helpers -->
```jai
// "sha256:" and 64 lowercase hex digits.
ev_digest :: (s:string)->bool {
  if s.count!=71 || s[0..<7]!="sha256:" {return false;}
  for c:s[7..<71] {if !((c>='0' && c<='9') || (c>='a' && c<='f')) {return false;}}
  return true;
}
ev_in :: (s:string,set:[]string)->bool {for x:set {if x==s {return true;}}return false;}
// An ArtifactRef: an object with a Digest and a nonnegative integer size.
ev_ref :: (text:string,v:ClSlice)->bool {
  if v.tok!=JSON_OBJECT {return false;}
  d:=cl_member(text,v,"digest");size:=cl_int_of(text,cl_member(text,v,"size"));
  return d.tok==JSON_STRING && ev_digest(text[d.from..<d.to]) && size.ok && size.value>=0;
}
ev_str :: (text:string,v:ClSlice)->string {if !v.ok || v.tok!=JSON_STRING {return "";}return text[v.from..<v.to];}
ev_count :: (text:string,arm:ClSlice,name:string)->int {
  n:=cl_int_of(text,cl_member(text,arm,name));
  if !n.ok || n.value<0 {return -1;}
  return n.value;
}
// An arm's counts: each a nonnegative integer, the five outcomes summing to attempted.
ev_arm :: (text:string,arm:ClSlice)->bool {
  if arm.tok!=JSON_OBJECT {return false;}
  sum:=0;
  for name:[5]string.["completed","refused","failed","cancelled","unknown"] {
    n:=ev_count(text,arm,name);
    if n<0 {return false;}
    sum+=n;
  }
  return ev_count(text,arm,"planned")>=0 && ev_count(text,arm,"excluded")>=0 && ev_count(text,arm,"attempted")==sum;
}
```

<!-- source: coax/lib/nostr/eval/report.coax report -->
```jai
// What this reader may show of the report: held are the digests of the
// artifacts it holds, trusted the evaluators it has chosen.
eval_report :: (text:string,held:[]string,trusted:[]string)->EvalView {
  r:EvalView;r.code=EV_MALFORMED;
  root:=cl_root(text);
  if !root.ok || root.tok!=JSON_OBJECT {return r;}
  if !cl_eq(text,cl_member(text,root,"v"),"openagents.eval-report.v1") {r.code=EV_VERSION;return r;}
  for name:[4]string.["suite","partition","runs","limitations"] {
    if !ev_ref(text,cl_member(text,root,name)) {r.code=EV_REF;return r;}
  }
  subject:=cl_member(text,root,"subject");
  id:=ev_str(text,cl_member(text,cl_member(text,subject,"definition"),"id"));
  baseline:=cl_member(text,root,"baseline");
  evaluator:=ev_str(text,cl_member(text,root,"evaluator"));
  if id.count==0 || !baseline.ok || (baseline.tok!=JSON_NULL && baseline.tok!=JSON_OBJECT) || evaluator.count==0 {return r;}
  based:=baseline.tok==JSON_OBJECT;
  start:=cl_int_of(text,cl_member(text,root,"started_at"));end:=cl_int_of(text,cl_member(text,root,"ended_at"));
  if !start.ok || !end.ok {return r;}
  if end.value<start.value {r.code=EV_TIME;return r;}
  coverage:=cl_member(text,root,"coverage");
  base:=cl_member(text,coverage,"baseline");
  if !ev_arm(text,cl_member(text,coverage,"subject")) || !base.ok {r.code=EV_COVERAGE;return r;}
  if (based && !ev_arm(text,base)) || (!based && base.tok!=JSON_NULL) {r.code=EV_COVERAGE;return r;}
  ms:=cl_member(text,root,"measurements");
  if !ms.ok || ms.tok!=JSON_ARRAY {return r;}
  for i:0..<cl_count(text[ms.from..<ms.to]) {
    m:=cl_element(text,ms,i);arm:=cl_member(text,m,"arm");value:=cl_member(text,m,"value");
    own:=cl_eq(text,arm,"subject") || (based && (cl_eq(text,arm,"baseline") || cl_eq(text,arm,"comparison")));
    shaped:=value.ok && (value.tok==JSON_NULL || cl_num_of(text,value).ok);
    if !own || !shaped || ev_str(text,cl_member(text,m,"metric")).count==0 || ev_count(text,m,"denominator")<0 || ev_count(text,m,"unknown_count")<0 {r.code=EV_MEASUREMENT;return r;}
  }
  verdict:=cl_member(text,root,"verdict");
  if cl_eq(text,verdict,"pass") {r.verdict=EV_PASS;}
  if cl_eq(text,verdict,"fail") {r.verdict=EV_FAIL;}
  if cl_eq(text,verdict,"inconclusive") {r.verdict=EV_INCONCLUSIVE;}
  if r.verdict==0 {r.code=EV_VERDICT;return r;}
  r.subject=id;
  suite:=cl_member(text,cl_member(text,root,"suite"),"digest");part:=cl_member(text,cl_member(text,root,"partition"),"digest");
  if !ev_in(text[suite.from..<suite.to],held) || !ev_in(text[part.from..<part.to],held) {r.code=EV_UNVERIFIABLE;return r;}
  if !ev_in(evaluator,trusted) {r.code=EV_UNTRUSTED;return r;}
  r.code=EV_VERIFIED;
  return r;
}
```

"Held" means the reader has the artifact's exact bytes: the digest is of
the bytes, so a report naming a suite by digest names one suite, and a
reader holding other bytes under the same display name holds a
different suite. The shape is checked before trust, so a malformed
report from a trusted key still refuses.

### Weighing several evaluators

NIP-EVAL's promotion record, `openagents.eval-admission.v1`, is where
evidence becomes a decision: `subject`, `reports`, `policy`, `scope`,
`decision` (`admit`, `reject` or `inconclusive`), `issuer` and
`expires_at`, with `validation` references. The NIP requires an
independently trusted issuer and at least one validation for an `admit`.
`eval_admit` checks those, then adds this reader's own policy, marked
as such in the source: every report an `admit` cites must be one the
reader read, verified, about the admitted subject, with verdict `pass`.

<!-- source: coax/lib/nostr/eval/report.coax admit -->
```jai
// A report the reader has read: the digest of its exact bytes and its view.
EvalSeen :: struct {digest:string;view:EvalView;}

// Whether the admission stands at now, given the reports this reader has
// read and the operators it trusts to admit.
eval_admit :: (text:string,seen:[]EvalSeen,operators:[]string,now:int)->u32 {
  root:=cl_root(text);
  if root.tok!=JSON_OBJECT || !cl_eq(text,cl_member(text,root,"v"),"openagents.eval-admission.v1") {return EA_MALFORMED;}
  subject:=ev_str(text,cl_member(text,cl_member(text,root,"subject"),"id"));
  reports:=cl_member(text,root,"reports");decision:=cl_member(text,root,"decision");
  expires:=cl_int_of(text,cl_member(text,root,"expires_at"));
  admit:=cl_eq(text,decision,"admit");
  if subject.count==0 || reports.tok!=JSON_ARRAY || !expires.ok || !(admit || cl_eq(text,decision,"reject") || cl_eq(text,decision,"inconclusive")) {return EA_MALFORMED;}
  n:=cl_count(text[reports.from..<reports.to]);
  for i:0..<n {if !ev_ref(text,cl_element(text,reports,i)) {return EA_MALFORMED;}}
  if !ev_in(ev_str(text,cl_member(text,root,"issuer")),operators) {return EA_ISSUER;}
  if now>=expires.value {return EA_EXPIRED;}
  if !admit {return EA_OK;}
  validation:=cl_member(text,root,"validation");
  if !validation.ok || validation.tok!=JSON_ARRAY || cl_count(text[validation.from..<validation.to])<1 {return EA_VALIDATION;}
  for i:0..<n {
    d:=cl_member(text,cl_element(text,reports,i),"digest");digest:=text[d.from..<d.to];
    found:=-1;
    for k:0..<seen.count {if seen[k].digest==digest {found=k;}}
    if found<0 {return EA_UNSEEN;}
    v:=seen[found].view;
    if v.code!=EV_VERIFIED {return EA_UNVERIFIED;}
    if v.subject!=subject {return EA_SUBJECT;}
    if v.verdict!=EV_PASS {return EA_VERDICT;}
  }
  return EA_OK;
}
```

## OPT: the cases that judge stay unseen

NIP-OPT separates an AI task's meaning from its implementation. A
semantic signature (`openagents.ai-signature.v1`) pins input and output
schemas, the task's human meaning, invariants, effects and evaluation
requirements. An implementation (`openagents.ai-implementation.v1`)
realizes one signature with a pinned decision function, program or
operation, and a target naming each model by role and assurance
(`artifact`, `provider_revision` or `alias_only`, the last never
claiming immutable weights). A study freezes the signature, baseline,
search space, data plan, suite, objective, acceptance policy and
optimizer before search starts; proposals, candidates, trials,
materializations, usage and results are its records.

The data plan is what keeps confirmation honest. It splits the suite's
cases into `search`, `selection`, `confirmation` and `excluded`: IDs
distinct within and across arrays, their union exactly the suite's
cases, search and selection together exactly the partition's
development, confirmation exactly its held-out cases, excluded exactly
excluded. A trial runs one case in one phase, and the case must belong
to that phase.

<!-- source: coax/lib/nostr/opt/plan.coax codes -->
```jai
OP_OK :: cast(u32)0;
OP_DUPLICATE :: cast(u32)1;  // an ID twice, within one array or across two
OP_UNKNOWN :: cast(u32)2;    // an ID that is not a case of the suite
OP_MISSING :: cast(u32)3;    // a case of the suite in no array
OP_PARTITION :: cast(u32)4;  // the plan's split does not match the partition's
OP_PHASE :: cast(u32)5;      // a trial's case outside its phase, or no such phase

OptPartition :: struct {development:[]string;held_out:[]string;excluded:[]string;}
OptPlan :: struct {search:[]string;selection:[]string;confirmation:[]string;excluded:[]string;}
```

<!-- source: coax/lib/nostr/opt/plan.coax plan -->
```jai
op_in :: (s:string,set:[]string)->bool {for x:set {if x==s {return true;}}return false;}
// Whether every ID of a is in b.
op_subset :: (a:[]string,b:[]string)->bool {for x:a {if !op_in(x,b) {return false;}}return true;}
// OP_DUPLICATE when an ID of a repeats in a or in any earlier array, else
// OP_UNKNOWN when one is not a case, else OP_OK.
op_fresh :: (a:[]string,earlier:[][]string,cases:[]string)->u32 {
  for i:0..<a.count {
    if op_in(a[i],a[0..<i]) {return OP_DUPLICATE;}
    for e:earlier {if op_in(a[i],e) {return OP_DUPLICATE;}}
    if !op_in(a[i],cases) {return OP_UNKNOWN;}
  }
  return OP_OK;
}

opt_partition :: (p:OptPartition,cases:[]string)->u32 {
  arrays:=[3][]string.[p.development,p.held_out,p.excluded];
  for k:0..<3 {
    code:=op_fresh(arrays[k],arrays[0..<k],cases);
    if code!=OP_OK {return code;}
  }
  return OP_OK;
}

opt_plan :: (plan:OptPlan,p:OptPartition,cases:[]string)->u32 {
  code:=opt_partition(p,cases);
  if code!=OP_OK {return code;}
  arrays:=[4][]string.[plan.search,plan.selection,plan.confirmation,plan.excluded];
  total:=0;
  for k:0..<4 {
    code=op_fresh(arrays[k],arrays[0..<k],cases);
    if code!=OP_OK {return code;}
    total+=arrays[k].count;
  }
  if total!=cases.count {return OP_MISSING;}
  // Every plan ID is a distinct case and every case is in the plan, so each
  // plan array inside its partition array makes the two splits equal.
  if !op_subset(plan.search,p.development) || !op_subset(plan.selection,p.development) {return OP_PARTITION;}
  if !op_subset(plan.confirmation,p.held_out) || !op_subset(plan.excluded,p.excluded) {return OP_PARTITION;}
  return OP_OK;
}

opt_trial :: (plan:OptPlan,phase:string,case_id:string)->u32 {
  if phase=="search" && op_in(case_id,plan.search) {return OP_OK;}
  if phase=="selection" && op_in(case_id,plan.selection) {return OP_OK;}
  if phase=="confirmation" && op_in(case_id,plan.confirmation) {return OP_OK;}
  return OP_PHASE;
}
```

The equal-set test needs no counting: every plan ID is a distinct case,
every case is in the plan, and the partition's arrays are disjoint, so
each plan array lying inside its partition array forces the splits to be
the same.

## PRG: typed programs, in outline

A program definition has `id`, typed `input` and `output` SchemaRefs, at
most 256 named `steps`, a `result` binding and whole-run `bounds`. Each
step has a slug `name`, one of seven kinds (`query`, `check`, `decide`,
`delegate`, `program`, `module`, `invoke`), an exact `target`
DefinitionRef, an `after` list naming only earlier steps, an `input`
binding, an `output` schema, `bounds`, a required `on_error` (`stop` or
`continue`) and optional `when` and `retry`. Bindings are exactly
`{literal}`, `{from, pointer}` (from `input` or `step:<name>` within the
`after` closure), `{object}` or `{array}`: no interpolation, wildcard or
executable expression. A `decide` step puts a pinned NIP-DEC question
set to a decision door and records the decision call beside the answer,
the record whose digests the DEC section computes. Kind `30182` is a
program's addressable discovery head, tagged `t: oa:program:v1` and one
`t: oa:step:<kind>` per distinct step kind; kind `30183` announces a Wasm
module that a `module` step runs through the packet ABI. None of PRG is
built here.

## Worked example: three evaluations into one decision, by hand

**The decision.** The state is `cargo test ran 3 tests: 3 passed, 0
failed.` with three questions. The answer gives `passed` 0.97; `next`
0.8 for `ship` and 0.2 for `fix`; `risk` 0.6, 0.3 and 0.1 for levels 0,
1 and 2.

- `next`: 0.8 + 0.2 = 1. The largest is 0.8, so the choice is `ship` and
  its confidence 0.8.
- `risk`: 0.6 + 0.3 + 0.1 = 1. The score is 0 x 0.6 + 1 x 0.3 + 2 x 0.1
  = 0.5, and the confidence the largest probability, 0.6.
- A door that reports concentration would send
  (0.8 - 1/2) / (1 - 1/2) = 0.6 as `next`'s confidence. 0.6 is not 0.8,
  so that answer refuses.
- The two digests, from Python: `state_digest`
  `sha256:a9b23ead...d1bd2cd8f0`, `questions_digest`
  `sha256:22bba331...dc80bcc4ff`.

**The evaluations.** Three evaluators report on the judge
`79be667e...:merge/judge` against the baseline `merge/always-ship`. The
reader holds one suite (Digest `sha256:997df277...`) and its partition
(`sha256:d16766cc...`), and trusts keys 1 and 2.

| Report | Evaluator | Suite | Subject arm | Verdict | Shown |
| --- | --- | --- | --- | --- | --- |
| R1 | key 1 | held | 10 attempted = 10 completed | pass | verified |
| R2 | key 2 | `sha256:3966794d...`, not held | 10 = 10 | pass | unverifiable |
| R3 | key 7 | held | 10 = 10 | fail | untrusted |
| R4 | key 1 | held | 10 attempted, 9 + 0 + 0 + 0 + 0 = 9 | pass | refused: coverage |

Each report's measurements give the comparison as subject minus
baseline: R1 0.9 - 0.6 = 0.3 on 10 cases. Only R1 is evidence this
reader can count. R2 may well be right, but the reader cannot see its
suite; R3's evaluator is not one the reader chose; R4 does not add up.

**The decision.** Key 4, an operator the reader trusts, issues
admissions at T0 = 1790000000, expiring at T0 + 30 x 86400 = 1792592000:

- citing R1, R2 and R3: refused, R2 is not verified (the first such
  citation stops it);
- citing R1 without a validation: refused, an `admit` needs one;
- citing R1 with a validation: stands;
- the same, read by a reader that trusts keys 1 and 2 but not key 4:
  refused, issuer not trusted;
- the same at T0 + 30 days: expired (at or past `expires_at`).

So three evaluations become one decision: admit, on R1's evidence alone,
and only for a reader who holds R1's suite, trusts key 1 and key 4.

**The study split.** Six cases: development c1, c2, c3; held out c4,
c5; excluded c6. Search c1, c2 and selection c3 with confirmation c4, c5
admits. Moving c4 into selection leaves confirmation c5, which no longer
equals the held-out set: refused. A confirmation trial on c4 admits; one
on c3, a development case, is outside its phase.

## What is not built

- **DEC**: the 25910 job's own fields (`request`, `attempt`, `deadline`,
  `requires`, `v`), its NIP-44 content and the 26910 result and 27010
  status envelopes; cancel; idempotency and its fingerprint; model
  admission and aliases; the HTTP status table and door order; the
  decrypted-payload bound; the legend's level texts (only its indexes are
  checked); and a state nested past 16 levels, which `json.coax` refuses
  though the NIP allows 32.
- **EVAL**: suites and their workloads, labels, metrics and acceptance
  policies; reading the runs and limitations artifacts; required
  measurement profiles; optimization evaluation fields; the 3188
  envelope; the 3189 publication event (`t: oa:eval:v1`, `x` equal to the
  report digest, signer equal to the evaluator); redaction; the
  extension evaluation profile, checks, validations, hosted runs,
  adoption and private Gym boards. Duplicate JSON keys are read as their
  first occurrence.
- **OPT**: signatures, implementations, targets, builds, studies, search
  spaces, proposals, candidates, trials, materializations, usage and
  results as records; exposure and leakage units; confirmation
  allowances. Only the data plan's split and a trial's phase are checked,
  over case IDs the caller has read.
- **PRG**: all of it: definitions, the step graph, bindings, conditions,
  retries, the 30182 head and its tags, the Wasm packet ABI and 30183.

## The example

[nostr/examples/decisions-evals.coax](../examples/decisions-evals.coax)
runs the worked example. It computes the decision digests and every
report's Digest from the bytes at run time.

<!-- source: nostr/examples/decisions-evals.coax example -->
```jai
// Chapter 81: one decision checked against the question set it answers,
// then three evaluations of the judge that made it, read by one reader and
// weighed by one admission, then an optimization study's case split. Keys
// are the x-only public keys of the scalars 1, 2, 4 and 7: TEST ONLY. The
// JSON texts and their digests come from coax/tests/nostr/dec_eval_vectors.py.
#load "../../coax/lib/nostr/dec/answer.coax";
#load "../../coax/lib/nostr/eval/report.coax";
#load "../../coax/lib/nostr/opt/plan.coax";
#load "../../coax/lib/nostr/ctrl/fingerprint.coax";
#load "../../coax/tests/nostr/dec_eval_vectors.coax";
#import "Basic";

T0 :: 1790000000;
K1 :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
K2 :: "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
K4 :: "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13";

DEC_WORDS :: [15]string.["answers its question set","malformed","invalid_request","too_many_questions",
  "too_many_options","a question has no answer","an answer to no question","another type",
  "not a probability","not exactly the options asked","does not sum to one",
  "not the most probable option","confidence is not that probability","not the weighted index",
  "legend is not the levels"];
EV_WORDS :: [10]string.["verified","unverifiable: suite or partition not held","untrusted evaluator",
  "malformed","version","artifact reference","time","coverage does not sum","measurement","verdict"];
EA_WORDS :: [9]string.["stands","malformed","issuer not trusted","expired","no validation",
  "cites a report not read","cites a report not verified","another subject","cites a verdict not pass"];
OP_WORDS :: [6]string.["admitted","duplicate","unknown case","missing case","not the partition's split",
  "outside its phase"];
VERDICTS :: [4]string.["-","pass","fail","inconclusive"];

// The answer with from replaced by to.
edit :: (from:string,to:string,out:[]u8)->string {return ts_swap(DEV_ANSWER,from,to,out);}
say_dec :: (what:string,c:DecCheck) {
  if c.question<0 {print("  %: %\n",what,DEC_WORDS[c.code]);}
  else {print("  %: %, question %\n",what,DEC_WORDS[c.code],c.question);}
}
digest_of :: (text:string,out:[]u8)->string {
  bytes:[8192]u8;for i:0..<text.count {bytes[i]=text[i];}
  #assert ae_digest(bytes[0..<text.count],out)==AE_OK;
  return cast(string)out;
}
```

<!-- source: nostr/examples/decisions-evals.coax main -->
```jai
main :: () {
  print("decision: state and three questions (passed noul, next choice, risk score)\n");
  canon:[1024]u8;sd:[CF_DIGEST]u8;qd:[CF_DIGEST]u8;
  root:=cl_root(DEV_BODY);
  #assert ctrl_fingerprint(cl_json(DEV_BODY,cl_member(DEV_BODY,root,"state")),canon[..],sd[..]).error==CF_OK;
  #assert ctrl_fingerprint(cl_json(DEV_BODY,cl_member(DEV_BODY,root,"questions")),canon[..],qd[..]).error==CF_OK;
  print("  state_digest     %\n  questions_digest %\n",cast(string)sd[..],cast(string)qd[..]);
  say_dec("the body",dec_body(DEV_BODY));
  say_dec("the answer: noul 0.97; ship 0.8 of {ship, fix}; risk 0*0.6 + 1*0.3 + 2*0.1 = 0.5",dec_answer(DEV_BODY,DEV_ANSWER));
  o:[2048]u8;
  print("answers that are not this question set's\n");
  say_dec("risk left out",dec_answer(DEV_BODY,edit(",\"risk\":{\"type\":\"score\",\"score\":0.5,\"confidence\":0.6,\"legend\":{\"0\":\"No risk.\",\"1\":\"Some risk.\",\"2\":\"High risk.\"},\"probabilities\":{\"0\":0.6,\"1\":0.3,\"2\":0.1}}","",o[..])));
  say_dec("an answer to urgent, never asked",dec_answer(DEV_BODY,edit("\"passed\":{","\"urgent\":{\"type\":\"noul\",\"noul\":0.1},\"passed\":{",o[..])));
  say_dec("passed answered as a choice",dec_answer(DEV_BODY,edit("{\"type\":\"noul\",\"noul\":0.97}","{\"type\":\"choice\",\"choice\":\"yes\",\"confidence\":1,\"probabilities\":{\"yes\":1}}",o[..])));
  say_dec("next over {ship, revert}",dec_answer(DEV_BODY,edit("{\"ship\":0.8,\"fix\":0.2}","{\"ship\":0.8,\"revert\":0.2}",o[..])));
  say_dec("risk over four levels",dec_answer(DEV_BODY,edit("\"2\":0.1}","\"2\":0.1,\"3\":0}",o[..])));
  say_dec("next 0.8 and 0.3",dec_answer(DEV_BODY,edit("\"fix\":0.2","\"fix\":0.3",o[..])));
  say_dec("next chooses fix",dec_answer(DEV_BODY,edit("\"choice\":\"ship\"","\"choice\":\"fix\"",o[..])));
  say_dec("next's confidence (0.8 - 1/2) / (1 - 1/2) = 0.6",dec_answer(DEV_BODY,edit("\"confidence\":0.8","\"confidence\":0.6",o[..])));
  say_dec("risk scored 1, the most probable level's neighbour",dec_answer(DEV_BODY,edit("\"score\":0.5","\"score\":1",o[..])));
  b:[2048]u8;
  say_dec("a body asking risk on eleven levels",dec_body(ts_swap(DEV_BODY,"[\"No risk.\",\"Some risk.\",\"High risk.\"]","[\"0\",\"1\",\"2\",\"3\",\"4\",\"5\",\"6\",\"7\",\"8\",\"9\",\"10\"]",b[..])));

  print("evaluations of %:merge/judge, read by a reader holding one suite and its partition\n",K1[0..<8]);
  sdig:[71]u8;pdig:[71]u8;
  held:=[2]string.[digest_of(DEV_SUITE,sdig[..]),digest_of(DEV_PARTITION,pdig[..])];
  trusted:=[2]string.[K1,K2];
  reports:=[4]string.[DEV_R1,DEV_R2,DEV_R3,DEV_R4];
  names:=[4]string.["R1 by key 1","R2 by key 2","R3 by key 7","R4 by key 1"];
  d:[4][71]u8;seen:[4]EvalSeen;
  for i:0..<4 {
    v:=eval_report(reports[i],held[..],trusted[..]);
    seen[i]=EvalSeen.{digest=digest_of(reports[i],d[i][..]),view=v};
    print("  % %...: verdict %, %\n",names[i],seen[i].digest[0..<19],VERDICTS[v.verdict],EV_WORDS[v.code]);
  }
  ops:=[1]string.[K4];
  print("admissions by key 4 at T0, expiring T0 + 30 days\n");
  print("  admit citing R1, R2, R3: %\n",EA_WORDS[eval_admit(DEV_ADMIT_ALL,seen[..],ops[..],T0)]);
  print("  admit citing R1, no validation: %\n",EA_WORDS[eval_admit(DEV_ADMIT_BARE,seen[..],ops[..],T0)]);
  print("  admit citing R1 with a validation: %\n",EA_WORDS[eval_admit(DEV_ADMIT_R1,seen[..],ops[..],T0)]);
  print("  the same, read by a reader who does not trust key 4: %\n",EA_WORDS[eval_admit(DEV_ADMIT_R1,seen[..],trusted[..],T0)]);
  print("  the same, 30 days later: %\n",EA_WORDS[eval_admit(DEV_ADMIT_R1,seen[..],ops[..],T0+30*86400)]);

  print("an optimization study's split of six cases\n");
  cases:=[6]string.["c1","c2","c3","c4","c5","c6"];
  dev:=[3]string.["c1","c2","c3"];held_out:=[2]string.["c4","c5"];ex:=[1]string.["c6"];
  p:=OptPartition.{development=dev[..],held_out=held_out[..],excluded=ex[..]};
  search:=[2]string.["c1","c2"];selection:=[1]string.["c3"];
  plan:=OptPlan.{search=search[..],selection=selection[..],confirmation=held_out[..],excluded=ex[..]};
  print("  partition dev {c1 c2 c3} held_out {c4 c5} excluded {c6}: %\n",OP_WORDS[opt_partition(p,cases[..])]);
  print("  plan search {c1 c2} selection {c3} confirmation {c4 c5}: %\n",OP_WORDS[opt_plan(plan,p,cases[..])]);
  leak:=[2]string.["c3","c4"];rest:=[1]string.["c5"];
  print("  plan selection {c3 c4} confirmation {c5}: %\n",OP_WORDS[opt_plan(OptPlan.{search=search[..],selection=leak[..],confirmation=rest[..],excluded=ex[..]},p,cases[..])]);
  print("  a confirmation trial on c4: %\n",OP_WORDS[opt_trial(plan,"confirmation","c4")]);
  print("  a confirmation trial on c3: %\n",OP_WORDS[opt_trial(plan,"confirmation","c3")]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/decisions-evals.coax
```

Its output, in debug and release builds alike
([nostr/fixtures/decisions-evals.out](../fixtures/decisions-evals.out)):

```text
decision: state and three questions (passed noul, next choice, risk score)
  state_digest     sha256:a9b23eade88a35be19312f107a54ade134c606ada18cb1e03d6704d1bd2cd8f0
  questions_digest sha256:22bba3312316efe0a7b344979a2479489966dfd289257d26073cbfdc80bcc4ff
  the body: answers its question set
  the answer: noul 0.97; ship 0.8 of {ship, fix}; risk 0*0.6 + 1*0.3 + 2*0.1 = 0.5: answers its question set
answers that are not this question set's
  risk left out: a question has no answer, question 2
  an answer to urgent, never asked: an answer to no question
  passed answered as a choice: another type, question 0
  next over {ship, revert}: not exactly the options asked, question 1
  risk over four levels: not exactly the options asked, question 2
  next 0.8 and 0.3: does not sum to one, question 1
  next chooses fix: not the most probable option, question 1
  next's confidence (0.8 - 1/2) / (1 - 1/2) = 0.6: confidence is not that probability, question 1
  risk scored 1, the most probable level's neighbour: not the weighted index, question 2
  a body asking risk on eleven levels: too_many_options, question 2
evaluations of 79be667e:merge/judge, read by a reader holding one suite and its partition
  R1 by key 1 sha256:3c0b9f7e9b89...: verdict pass, verified
  R2 by key 2 sha256:ba7fd097a756...: verdict pass, unverifiable: suite or partition not held
  R3 by key 7 sha256:f59021234ac0...: verdict fail, untrusted evaluator
  R4 by key 1 sha256:95e6c0fafdd7...: verdict -, coverage does not sum
admissions by key 4 at T0, expiring T0 + 30 days
  admit citing R1, R2, R3: cites a report not verified
  admit citing R1, no validation: no validation
  admit citing R1 with a validation: stands
  the same, read by a reader who does not trust key 4: issuer not trusted
  the same, 30 days later: expired
an optimization study's split of six cases
  partition dev {c1 c2 c3} held_out {c4 c5} excluded {c6}: admitted
  plan search {c1 c2} selection {c3} confirmation {c4 c5}: admitted
  plan selection {c3 c4} confirmation {c5}: not the partition's split
  a confirmation trial on c4: admitted
  a confirmation trial on c3: outside its phase
```

The broken inputs and their exact refusals are in that output: a missing
question, an answer to a question never asked, a noul answered as a
choice, an option not asked, a fourth score level, probabilities summing
to 1.1, the less probable choice, a concentration confidence, a wrong
score, a body with eleven levels, a report whose coverage does not sum,
an admission citing an unverifiable report, one without a validation,
one from an untrusted issuer and one past its expiry, a leaked held-out
case and a trial outside its phase.

## How the code is checked

### Checks in the example

<!-- source: nostr/examples/checks/decisions-evals.coax check -->
```jai
// The digests, from coax/tests/nostr/dec_eval_vectors.py: Python's json
// (sorted keys, compact separators, which is JCS for these number-free
// values) and hashlib.sha256. They share no code with jcs.coax or
// sha256.coax.
#test "Coax computes each Python digest" #steps 200000000 {
  canon:[1024]u8;out:[CF_DIGEST]u8;root:=cl_root(DEV_BODY);
  #assert ctrl_fingerprint(cl_json(DEV_BODY,cl_member(DEV_BODY,root,"state")),canon[..],out[..]).error==CF_OK;
  #assert cast(string)out[..]==DEV_STATE_DIGEST && DEV_STATE_DIGEST=="sha256:a9b23eade88a35be19312f107a54ade134c606ada18cb1e03d6704d1bd2cd8f0";
  #assert ctrl_fingerprint(cl_json(DEV_BODY,cl_member(DEV_BODY,root,"questions")),canon[..],out[..]).error==CF_OK;
  #assert cast(string)out[..]==DEV_QUESTIONS_DIGEST && DEV_QUESTIONS_DIGEST=="sha256:22bba3312316efe0a7b344979a2479489966dfd289257d26073cbfdc80bcc4ff";
  d:[71]u8;
  #assert digest_of(DEV_SUITE,d[..])==DEV_SUITE_DIGEST && digest_of(DEV_PARTITION,d[..])==DEV_PARTITION_DIGEST;
  #assert digest_of(DEV_R1,d[..])==DEV_R1_DIGEST && DEV_R1_DIGEST=="sha256:3c0b9f7e9b89cb6829c93350eeb3e23298558b5c14f76162a9744a8c338af6f9";
  #assert digest_of(DEV_R2,d[..])==DEV_R2_DIGEST && digest_of(DEV_R3,d[..])==DEV_R3_DIGEST && digest_of(DEV_R4,d[..])==DEV_R4_DIGEST;
}

// The retired Rust crate's own decision fixtures (crates/nostr/fixtures/
// decisions at 0b401561a0^, kept in coax/tests/nostr/data/decisions/):
// every body it admitted admits here, and every one it refused refuses as
// invalid_request, the state array, the bare number, the empty choice, the
// third noul outcome, the bare-number level, the nine-level entry and the
// unknown type alike.
#test "the retired crate's valid bodies admit and its invalid ones refuse" #steps 400000000 {
  for i:0..<DEV_VALID.count {#assert dec_body(DEV_VALID[i]).code==DEC_OK;}
  for i:0..<DEV_INVALID.count {#assert dec_body(DEV_INVALID[i]).code==DEC_INVALID;}
  #assert DEV_VALID.count==7 && DEV_INVALID.count==7;
}

// The decision worked by hand: risk's score is 0*0.6 + 1*0.3 + 2*0.1 = 0.5
// and its confidence the largest probability, 0.6; next's choice is ship,
// the larger of 0.8 and 0.2, with confidence 0.8. The concentration form
// some doors serve, (0.8 - 1/2) / (1 - 1/2) = 0.6, is not NIP-DEC's.
#test "the hand values" {
  #assert dec_near(0.0*0.6+1.0*0.3+2.0*0.1,0.5) && dec_answer(DEV_BODY,DEV_ANSWER).code==DEC_OK;
  #assert dec_near((0.8-0.5)/(1.0-0.5),0.6);
}

// The three evaluations weighed by hand: R1 is the only one this reader
// may count, so the admission stands only citing R1 alone, with a
// validation, issued by key 4 before T0 + 30 days.
#test "the admission's outcomes" #steps 200000000 {
  sd:[71]u8;pd:[71]u8;held:=[2]string.[digest_of(DEV_SUITE,sd[..]),digest_of(DEV_PARTITION,pd[..])];
  trusted:=[2]string.[K1,K2];ops:=[1]string.[K4];
  r:=[3]string.[DEV_R1,DEV_R2,DEV_R3];dg:=[3]string.[DEV_R1_DIGEST,DEV_R2_DIGEST,DEV_R3_DIGEST];seen:[3]EvalSeen;
  for i:0..<3 {seen[i]=EvalSeen.{digest=dg[i],view=eval_report(r[i],held[..],trusted[..])};}
  #assert seen[0].view.code==EV_VERIFIED && seen[1].view.code==EV_UNVERIFIABLE && seen[2].view.code==EV_UNTRUSTED;
  #assert eval_report(DEV_R4,held[..],trusted[..]).code==EV_COVERAGE;
  #assert eval_admit(DEV_ADMIT_ALL,seen[..],ops[..],T0)==EA_UNVERIFIED;
  #assert eval_admit(DEV_ADMIT_BARE,seen[..],ops[..],T0)==EA_VALIDATION;
  #assert eval_admit(DEV_ADMIT_R1,seen[..],ops[..],T0)==EA_OK;
  #assert eval_admit(DEV_ADMIT_R1,seen[..],ops[..],T0+30*86400-1)==EA_OK;
  #assert eval_admit(DEV_ADMIT_R1,seen[..],ops[..],T0+30*86400)==EA_EXPIRED;
  #assert eval_admit(DEV_ADMIT_R1,seen[1..<3],ops[..],T0)==EA_UNSEEN;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/decisions-evals.coax
```

`coax test` runs 131 checks, including the loaded files' own checks; all
pass.

The independent checks are two. Python's `json` and `hashlib` compute
every digest the example prints, sharing no code with `jcs.coax` or
`sha256.coax`. And the retired Rust implementation's own decision
fixtures (`crates/nostr/fixtures/decisions` at `0b401561a0^`, kept in
`coax/tests/nostr/data/decisions/`) were written against the same NIP by
other code: all seven bodies it admitted admit here and all seven it
refused refuse. NIP-DEC, NIP-EVAL and NIP-OPT publish no signed wire
examples with event ids, so no published id could be compared or found
wrong; NIP-DEC's examples are those same fixtures' fragments.

To regenerate the vectors:

```shell
python3 coax/tests/nostr/dec_eval_vectors.py > coax/tests/nostr/dec_eval_vectors.coax
```

### The library's own checks

`answer.coax` checks the slack at both edges, byte order, entry depth at
8 and 9, escaped IDs, every refusal of a body (state, model and question
sizes at their exact bounds, 64 and 65 questions, 255 and 256 options, 10
and 11 levels, ID lengths at 64/65 and 128/129, duplicate IDs, every
noul criteria form) and of an answer (each missing, extra, mistyped,
out-of-range, mis-keyed and mis-summed case, ties in either option
order, confidence and score at the slack). `report.coax` checks each
field's shape, each coverage count and sum, the baseline rules, digest
spelling, and every admission outcome. `plan.coax` checks duplicates
within and across arrays, unknown and missing cases, each array outside
its partition array, and each phase.

```shell
coax/bin/coax test coax/lib/nostr/dec/answer.coax --library
coax/bin/coax mutate coax/lib/nostr/dec/answer.coax --root-only --library
coax/bin/coax test coax/lib/nostr/eval/report.coax --library
coax/bin/coax mutate coax/lib/nostr/eval/report.coax --root-only --library
coax/bin/coax test coax/lib/nostr/opt/plan.coax --library
coax/bin/coax mutate coax/lib/nostr/opt/plan.coax --root-only --library
```

On 2026-11-01, `answer.coax` passed its checks (39 with `fields.coax`'s
and the JSON reader's) and `coax mutate --root-only` killed all 297
mutants; `report.coax` passed 32 and killed all 188; `plan.coax` passed 3
and killed all 39. None survived and none is marked equivalent. The
full mutation run (`COAX_MUTATION_FULL=1`) and cross-target runs were
not done.

## Summary

- A decision body is a state and up to 64 typed questions; a worker
  refuses one past NIP-DEC's bounds before any model reads it.
- `dec_answer` admits an answer only when it answers exactly the
  question set asked, with every number the probabilities fix.
- A decision-call record binds its answer to the body by the JCS digests
  of the state and the questions.
- `eval_report` shows a report as verified only when the reader holds
  its suite and partition and trusts its evaluator; a report about a
  suite the reader does not hold is shown as unverifiable.
- `eval_admit` turns several evaluations into one decision only on the
  evidence the reader can count.
- `opt_plan` keeps a study's confirmation cases exactly the held-out
  cases, so search and selection never see them.

## Dependencies

- [80. Jobs, labor and markets](ch080-jobs-labor-and-markets.md)

## Sources

- [OpenAgents NIP-DEC](../nips/openagents/NIP-DEC.md)
- [OpenAgents NIP-EVAL](../nips/openagents/NIP-EVAL.md)
- [OpenAgents NIP-OPT](../nips/openagents/NIP-OPT.md)
- [OpenAgents NIP-PRG](../nips/openagents/NIP-PRG.md)
- [OpenAgents shared contracts](../nips/openagents/contracts.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
