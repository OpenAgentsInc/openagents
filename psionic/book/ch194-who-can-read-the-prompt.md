# 194. Who can read the prompt

> **Status: partly implemented.** A trust layer runs on the CPU against
> chapter 189's simulated peers (profile trust-v1). It computes what each
> peer of a placement sees, gives each peer a tier from recorded
> evidence, turns a policy per data class into hard constraints on the
> placement solver, refuses plaintext links, names the exposure and the
> fewest peers to trust when a placement needs more trust, and checks
> consent for every peer before the first byte leaves. Still to build: the
> attested-enclave tier (the only private one), evidence and link keys
> read from live peers instead of written into a fixture, and the trust
> check wired into a running split.

When a model is split across machines, every machine that serves a stage
receives part of the request. Encrypting the links keeps the bytes away
from the network. It does not keep them away from the peer that serves
them: that peer decrypts them to compute. So the useful question is not
"is it encrypted" but "who receives what". This chapter answers it from
the placement and refuses placements whose answer the policy forbids.

Read this first: **this chapter enforces who receives what. It hides
nothing.** A peer that receives hidden-state rows (activations) can often
invert them back to the prompt; published inversion attacks recover input
text from intermediate layers. So activations are not secret, and a
policy that sends activations to a peer must trust that peer with the
prompt's content at some level. Nothing here is cryptographic hiding.

The library is [`peer/trust.coax`](../lib/peer/trust.coax). It is an
`#authority` file, because it decides which machines may receive a
person's data. It reuses chapter 189's solver
([`place.coax`](../lib/place.coax)) for placement, the consent gate of
[Coder chapter 18](../../coder/book/ch018-consent-before-sharing.md)
([`harness/consent.coax`](../lib/harness/consent.coax)) for data-class
labels and grants, chapter 188's session keys for links, and chapter
195's audits as evidence. The examples are
[`prompt-trust.coax`](../examples/prompt-trust.coax) and
[`prompt-trust-refuse.coax`](../examples/prompt-trust-refuse.coax).

```shell
coax/bin/coax run psionic/examples/prompt-trust.coax
coax/bin/coax run psionic/examples/prompt-trust-refuse.coax
psionic/check.sh --only peer/trust
```

## Tiers from evidence

A peer's tier is one of three, lowest first:

- **any**: an authenticated peer. It reads whatever it is sent.
- **vetted**: a member of a private mesh, admitted by an invitation
  ([chapter 188](ch188-peers-transport-and-discovery.md)), with at least
  `vet` passed audits and no failed one
  ([chapter 195](ch195-check-a-peers-work.md)). Vetted means "has behaved
  so far". The machine's owner can still read its memory.
- **owned**: a machine whose key is the owner's own. This machine, peer
  0, is always owned.

The tier is computed only from the evidence recorded about the peer.
Nothing else raises it:

<!-- source: psionic/lib/peer/trust.coax tiers -->
```jai
// Tiers, lowest first. any: an authenticated peer. vetted: a member of a
// private mesh (chapter 188's invitation) with enough passed audits and
// no failed one (chapter 195). owned: a machine whose key is the owner's.
WR_ANY :: 0;
WR_VETTED :: 1;
WR_OWNED :: 2;
WR_TIERS :: 3;
// The evidence recorded about a peer. Nothing else raises its tier.
WrEvidence :: struct {owner:bool;member:bool;audits:int;failed:int;}
wr_tier :: (e:WrEvidence,vet:int)->int {
    if e.owner {return WR_OWNED;}
    if e.member && e.failed==0 && e.audits>=vet {return WR_VETTED;}
    return WR_ANY;
}
```

A check removes each piece of evidence from an owned peer in turn and
sees the tier drop: without the owner key it is vetted; without
membership, with one audit too few, or with one failed audit it is any.

None of these tiers is private. A peer at any tier can read what it is
sent. The draft for this chapter named a third level, an attested hardware
enclave running a logged release ([NIP-ATT](../../nostr/nips/openagents/NIP-ATT.md)),
as the only private one. That tier is not built; see
[What remains](#what-remains).

## What each peer sees

[Chapter 186](ch186-split-a-model-into-stages.md)'s stage contract fixes
what crosses each boundary. Token ids enter the first stage. Each stage
computes hidden-state rows and sends them to the next. The last stage
computes the logits, samples a token and sends it back to this machine.
For the next step, that token enters the first stage again. So:

| Stage | Prompt tokens | Activations | Output tokens |
| --- | --- | --- | --- |
| first (stage 0) | yes: it receives them | yes: it computes them | yes: each sampled token comes back in |
| middle | no | yes | no |
| last | no | yes | yes: it samples them |
| the only stage | yes | yes | yes |

This machine holds the prompt and gets the output back, so it sees prompt
and output tokens even when it runs no stage.

<!-- source: psionic/lib/peer/trust.coax exposure -->
```jai
// What a peer sees, one bit each: the prompt's token ids, hidden-state
// rows, and the sampled output tokens.
WR_TOKENS :: 1;
WR_ACTS :: 2;
WR_OUTPUT :: 4;
WR_KINDS :: 3;
// The peer of stage s of n, under chapter 186's contract: it computes
// hidden rows. Stage 0 also gets the prompt's tokens and every sampled
// token fed back in; the last stage samples the output.
wr_sees :: (n:int,s:int)->int {
    x:=WR_ACTS;
    if s==0 {x|=WR_TOKENS|WR_OUTPUT;}
    if s==n-1 {x|=WR_OUTPUT;}
    return x;
}
// Every peer's exposure under an assignment. Peer 0, this machine, holds
// the prompt and gets the output back.
wr_exposed :: (n:int,peer:[]int)->[8]int {
    e:[8]int;e[0]=WR_TOKENS|WR_OUTPUT;
    for s:0..<n {e[peer[s]]|=wr_sees(n,s);}
    return e;
}
```

The table says the uncomfortable part plainly: the sampler sees the
answer and the first stage sees the question and the answer. Moving the
sampler to this machine would not help, because the last stage would then
send logits, and the logits name the answer.

## The policy

A request carries a label: the data classes of everything it was built
from, as Coder chapter 18 computes them (session, user, project, tool,
identity). The policy gives, for each class and each kind of exposure, the
lowest tier allowed to see it. A peer's need is the highest entry over the
classes in the label and the kinds it sees.

<!-- source: psionic/lib/peer/trust.coax policy -->
```jai
// The policy: need[class][kind] is the lowest tier that may see that kind
// (0 tokens, 1 activations, 2 output) of that class (chapter 213's
// session, user, project, tool, identity). vet: the passed audits that
// make a member vetted.
WrPolicy :: struct {need:[5][3]int;vet:int;}
// The tier a peer needs to see `sees` of a request labelled `label`: the
// highest the policy names for any class and kind it sees.
wr_need :: (p:WrPolicy,label:int,sees:int)->int {
    t:=WR_ANY;
    for c:0..<CS_CLASSES {for k:0..<WR_KINDS {
        x:=p.need[c][k];
        if (label>>c)&1==1 && (sees>>k)&1==1 {t=stg_max(t,x);}
    }}
    return t;
}
```

The chapter's policy and fleet are fixtures in the checks. What a person
typed and facts about them may be read in token form only by owned
machines, and their activations only by vetted ones. Project files and
tool output may go to vetted machines in any form, and their activations
anywhere. Identity data stays on owned machines in every form.

<!-- source: psionic/tests/peer-trust-checks.coax fixture -->
```jai
// The chapter's policy. Rows are classes (session, user, project, tool,
// identity); columns are prompt tokens, activations, output tokens.
wrx_policy :: ()->WrPolicy {
    p:=WrPolicy.{vet=3};
    p.need[0]=[3]int.[WR_OWNED,WR_VETTED,WR_OWNED];
    p.need[1]=[3]int.[WR_OWNED,WR_VETTED,WR_OWNED];
    p.need[2]=[3]int.[WR_VETTED,WR_ANY,WR_VETTED];
    p.need[3]=[3]int.[WR_VETTED,WR_ANY,WR_VETTED];
    p.need[4]=[3]int.[WR_OWNED,WR_OWNED,WR_OWNED];
    return p;
}
// Chapter 189's three peers: this machine (no evidence needed), the
// owner's desktop, and a friend's machine in the private mesh with five
// passed audits. Every link has session keys.
wrx_trust :: ()->WrTrust {
    t:WrTrust;
    t.evidence[1]=WrEvidence.{owner=true};
    t.evidence[2]=WrEvidence.{member=true,audits=5};
    for a:0..<8 {for c:0..<8 {t.sealed[a][c]=true;}}
    return t;
}
```

## Worked by hand: three peers

Take chapter 189's fleet and its two Ministral 3 3B stages. Peer 0 is this
machine, peer 1 the owner's desktop (owned), peer 2 a friend's machine
(member, 5 audits ≥ 3, no failures: vetted). Chapter 189 worked out the
latency of each assignment of the two stages:

| Stage 0 on | Stage 1 on | Latency |
| --- | --- | --- |
| 1 | 2 | 32,089,922 ns |
| 0 | 1 | 48,391,507 ns |
| 1 | 0 | 50,436,092 ns |
| 0 | 2 | 61,789,802 ns |

**Project files only** (label `project`). Stage 0 sees tokens,
activations and output: the policy's project row gives vetted, any,
vetted, so it needs vetted. Stage 1 sees activations and output: any and
vetted, so vetted. Every peer is at least vetted, nothing is denied, and
the fastest assignment stands: stage 0 on peer 1, stage 1 on peer 2,
32,089,922 ns. Peer 1 sees all three kinds (7), peer 2 activations and
output (6), and this machine prompt and output tokens (5).

**What the person typed, with project files** (label session + project).
Stage 0 now needs owned (session tokens and output), and so does stage 1
(session output). Peer 2 is vetted, so both stages deny it. Of the
assignments left, (0, 1) at 48,391,507 ns beats (1, 0) at 50,436,092 ns.
Keeping the typed text on owned machines costs 16.3 ms per token here.
Peer 2 sees nothing.

**The desktop loses its owner key.** Peer 1 is now a vetted member. Both
stages still need owned, and only peer 0 is owned, so two stages have one
peer: chapter 189's solver refuses (too few peers). The trust layer then
searches for the fewest peers to raise. Peers 1 and 2 are below owned, so
there are three sets: {1}, {2}, {1, 2}, all solved. {1} places (0, 1) and
{2} places (0, 2); both have one peer, and {1} has the lower bits, so the
fix is peer 1 (`fix` = 2). Under (0, 1), stage 0 is on this machine, which
is owned; stage 1 is on peer 1, vetted, needing owned. The first class in
the label and the first kind it sees whose entry is owned: session, output
tokens. The refusal reads "stage 1 would show session output tokens to
peer 1 (vetted), which needs owned; raise peer 1".

## Placing with trust as a hard constraint

The trust layer adds no search of its own. It turns each stage's need into
a bit mask of denied peers and hands the job to chapter 189's
`plc_place`, which this chapter extended with a per-stage `deny` mask and
a `trust` refusal. So the cost model, the tie rule (first in peer order)
and replanning are chapter 189's. Peer 0 is never denied because it is
owned.

<!-- source: psionic/lib/peer/trust.coax place -->
```jai
// What the trust layer knows of the fleet: each peer's evidence, and for
// each ordered pair whether the link has chapter 188's session keys.
WrTrust :: struct {evidence:[8]WrEvidence;sealed:[8][8]bool;}
WrStatus :: enum u8 {ok;input;plaintext;exposure;place;}
// The answer. tier and need: each peer's tier and each stage's need.
// exposed: what each peer sees under `placed`. plaintext: the live link
// from -> to has bandwidth and no session. exposure: under `placed`, which
// the fix would allow, stage `stage` would show `kind` of class `class` to
// `peer`, whose tier `have` is below `want`; fix is the fewest peers (then
// the lowest bits) whose tier must rise to owned. place: even with every
// peer owned it does not place; placed is chapter 189's refusal. tried: the
// sets of raised peers solved in the search for a fix.
WrPlacement :: struct {
    status:WrStatus;placed:PlcPlacement;label:int;tier:[8]int;need:[8]int;exposed:[8]int;
    from:int;to:int;stage:int;peer:int;cls:int;kind:int;have:int;want:int;fix:int;tried:int;
}
wr_valid :: (f:PlcFleet,t:WrTrust,p:WrPolicy,label:int)->bool {
    if label<0 || label>CS_ALL || p.vet<0 || p.vet>WR_COUNT_MAX {return false;}
    for c:0..<CS_CLASSES {for k:0..<WR_KINDS {if p.need[c][k]<0 || p.need[c][k]>=WR_TIERS {return false;}}}
    for a:0..<f.peers {
        e:=t.evidence[a];
        if e.audits<0 || e.audits>WR_COUNT_MAX || e.failed<0 || e.failed>WR_COUNT_MAX {return false;}
    }
    return true;
}
// Place with stage s denied every peer whose tier is below need[s], on top
// of the job's own denials. Peer 0 is owned.
wr_try :: (j:PlcJob,f:PlcFleet,tier:[8]int,need:[8]int,goal:PlcGoal,old:[]int)->PlcPlacement {
    d:=j;
    for s:0..<j.stages {for a:1..<f.peers {if tier[a]<need[s] {d.deny[s]|=1<<a;}}}
    return plc_place(d,f,goal,old);
}
wr_place :: (j:PlcJob,f:PlcFleet,t:WrTrust,p:WrPolicy,label:int,goal:PlcGoal,old:[]int)->WrPlacement {
    r:=WrPlacement.{status=WrStatus.input,label=label,from=-1,to=-1,stage=-1,peer=-1,cls=-1,kind=-1,have=-1,want=-1};
    if !plc_valid(j,f,old) || !wr_valid(f,t,p,label) {return r;}
    P:=f.peers;n:=j.stages;
    // Every live link that carries bytes must be sealed.
    for a:0..<P {for c:0..<P {
        if a!=c && f.peer[a].live && f.peer[c].live && f.bw[a][c]>0 && !t.sealed[a][c] {
            r.status=WrStatus.plaintext;r.from=a;r.to=c;return r;
        }
    }}
    for a:0..<P {r.tier[a]=ifx a==0 then WR_OWNED else wr_tier(t.evidence[a],p.vet);}
    for s:0..<n {r.need[s]=wr_need(p,label,wr_sees(n,s));}
    r.placed=wr_try(j,f,r.tier,r.need,goal,old);
    if r.placed.status==PlcStatus.ok {r.status=WrStatus.ok;r.exposed=wr_exposed(n,r.placed.peer[0..<n]);return r;}
    // Raise sets of the peers below owned to this machine's tier: every set
    // but the empty one, as bits of m in peer order, keeping the first of
    // the fewest that places. The last set raises them all, so trust no
    // longer binds.
    cand:=0;
    for a:1..<P {if r.tier[a]<WR_OWNED {cand+=1;}}
    full:=r.placed;
    for m:1..<(1<<cand) {
        up:=r.tier;raise:=0;i:=0;
        for a:1..<P {if r.tier[a]<WR_OWNED {if (m>>i)&1==1 {up[a]=r.tier[0];raise|=1<<a;}i+=1;}}
        q:=wr_try(j,f,up,r.need,goal,old);r.tried+=1;full=q;
        if q.status==PlcStatus.ok && (r.placed.status!=PlcStatus.ok || plc_count(raise)<plc_count(r.fix)) {r.placed=q;r.fix=raise;}
    }
    if r.placed.status!=PlcStatus.ok {r.status=WrStatus.place;r.placed=full;return r;}
    // Name the first stage whose peer is below its need, and the first
    // class and kind of its exposure that asks for that much.
    r.status=WrStatus.exposure;r.exposed=wr_exposed(n,r.placed.peer[0..<n]);
    s:=0;while r.tier[r.placed.peer[s]]>=r.need[s] {s+=1;}
    r.stage=s;r.peer=r.placed.peer[s];r.have=r.tier[r.peer];r.want=r.need[s];
    sees:=wr_sees(n,s);
    for c:0..<CS_CLASSES {for x:0..<WR_KINDS {
        if p.need[c][x]==r.want && r.cls<0 && (label>>c)&1==1 && (sees>>x)&1==1 {r.cls=c;r.kind=x;}
    }}
    return r;
}
```

The rules, in order:

1. **Encryption on every link.** A live link with bandwidth whose ordered
   pair has no session keys refuses the whole fleet, naming the first such
   link in peer order. A link with no bandwidth, to a peer that has left,
   is never used and is not checked. Session keys are chapter 188's
   per-direction keys; this layer reads whether they exist, it does no
   cryptography.
2. **Trust before cost.** Each stage is denied every peer below its need.
   The solver never trades exposure for speed.
3. **The smallest fix.** When the constrained solve fails, the layer tries
   raising sets of the peers below owned, every set but the empty one.
   The fix is the set with the fewest peers that places, ties going to the
   lower bits. With `c` such peers that is 2^c − 1 solves, at most 127.
   The refusal names the first stage, in the fix's placement, whose peer
   is below its need, and the class and kind that ask for that tier.
4. **Physics last.** If even raising every such peer does not place, trust
   is not the problem: the answer is chapter 189's own refusal (memory,
   deadline, peers or link).

The fix says whom to trust more; it does not grant that trust. Trust rises
only when the evidence does: an owner key, an invitation, more passed
audits.

## Consent before the first byte

Placement decides which machines may receive the request. Consent decides
whether the person agreed to send it. Every remote peer of a placement
receives bytes derived from the request (tokens, rows or both), so each
one needs an active grant from Coder chapter 18's ledger, to that peer, for
the purpose `compute`, covering every class of the label. The check runs
for all of them before this machine sends anything, because once stage 0's
peer has the tokens, this machine no longer controls where the rows go
next.

<!-- source: psionic/lib/peer/trust.coax consent -->
```jai
// What the gate decided. placement: the placement was refused. label: the
// parts carry a class the placement was not decided for. consent: a peer
// of the placement has no active grant covering the label. send: chapter
// 213's cs_send refused. peer: the peer refused, or the one sent to (0
// when nothing leaves this machine).
WrGate :: enum u8 {ok;placement;label;consent;send;}
WrSent :: struct {gate:WrGate;peer:int;decision:CsDecision;}
// Before any byte leaves: every remote peer of the placement gets bytes
// derived from the request, so each needs an active grant to compute on
// every class of the label.
wr_consent :: (l:CsLedger,p:WrPlacement,now:int)->WrSent {
    for s:0..<p.placed.stages {
        a:=p.placed.peer[s];
        if a==0 {continue;}
        d:=cs_decide(l,CsDest.{kind=CsKind.peer,id=a,purpose=CsPurpose.compute},p.label,now);
        if d.why!=CsWhy.ok {return WrSent.{gate=WrGate.consent,peer=a,decision=d};}
    }
    return WrSent.{gate=WrGate.ok,peer=-1,decision=cs_fail(CsWhy.ok)};
}
// This machine's message into the pipeline: the prompt's tokens to stage
// 0's peer, or, when this machine runs stage 0, its hidden rows to stage
// 1's peer. It goes through cs_send, which logs it before the bytes move.
wr_send :: (l:[]CsLedger,g:CsGraph,sink:[]CsSink,parts:[]int,p:WrPlacement,now:int)->WrSent {
    r:=WrSent.{gate=WrGate.placement,peer=-1,decision=cs_fail(CsWhy.shape)};
    if p.status!=WrStatus.ok {return r;}
    for q:parts {if q<0 || q>=g.count {r.gate=WrGate.send;return r;}}
    if (cs_label(g,parts)|p.label)!=p.label {r.gate=WrGate.label;return r;}
    c:=wr_consent(l[0],p,now);
    if c.gate!=WrGate.ok {return c;}
    // A one-stage placement leaves peer[1] at 0: nothing leaves.
    to:=p.placed.peer[0];
    if to==0 {to=p.placed.peer[1];}
    r.peer=to;
    if to==0 {r.gate=WrGate.ok;r.decision=c.decision;return r;}
    r.decision=cs_send(l,g,sink,CsMessage.{dest=CsDest.{kind=CsKind.peer,id=to,purpose=CsPurpose.compute},parts=parts},now);
    r.gate=ifx r.decision.why==CsWhy.ok then WrGate.ok else WrGate.send;
    return r;
}
```

The parts sent must carry no class the placement was not decided for: a
placement made for project files cannot carry a fact about the user. A
node of rows computed from the prompt and a project file carries both
labels, because Coder chapter 18's label propagation gives a derived node
every parent's classes. Activations are the prompt's data in another
form, and the gate treats them so.

## Running it

<!-- source: psionic/examples/prompt-trust.coax run -->
```jai
main :: () {
    j:=plcx_job();f:=plcx_fleet();t:=wrx_trust();p:=wrx_policy();zero:[1]int;none:=zero[0..<0];
    proj:=wr_place(j,f,t,p,WRX_PROJECT,PlcGoal.latency,none);
    trust_show("project files",proj);
    typed:=wr_place(j,f,t,p,WRX_SESSION|WRX_PROJECT,PlcGoal.latency,none);
    trust_show("typed in the session, with project files",typed);
    // The consent gate: a prompt, a project file, and stage 0's rows made from both.
    events:[16]CsEvent;grants:[8]CsGrant;texts:[8]string;own:[8]int;labels:[8]int;bytes:[256]u8;ends:[8]int;sent:[8]int;nb:[1]int;
    lb:=[1]CsLedger.[CsLedger.{events=events[..],grants=grants[..]}];l:=lb[..];
    gb:=[1]CsGraph.[CsGraph.{texts=texts[..],own=own[..],labels=labels[..]}];g:=gb[..];
    kb:=[1]CsSink.[CsSink.{bytes=bytes[..],ends=ends[..],events=sent[..]}];k:=kb[..];
    a:=cs_node(g,"Fix the flaky test in parser.rs.",WRX_SESSION,nb[0..<0]);a=cs_node(g,"fn parse(s: &str) -> Ast",WRX_PROJECT,nb[0..<0]);
    two:=[2]int.[0,1];rows:=[1]int.[cs_node(g,"<stage 0 rows>",0,two[..])];
    s:=wr_send(l,g[0],k,rows[..],typed,1);
    print("send stage 0's rows at tick 1: %, peer %, %\n",wr_gate_name(s.gate),s.peer,ifx s.decision.why==CsWhy.none then "no grant" else "?");
    w:=cs_grant(l,CsDest.{kind=CsKind.peer,id=1,purpose=CsPurpose.compute},WRX_SESSION|WRX_PROJECT,2,100,1,2);
    print("grant peer 1 compute on session and project, ticks 2 to 100: %\n",ifx w==CsWhy.ok then "ok" else "refused");
    s=wr_send(l,g[0],k,rows[..],typed,3);
    print("send stage 0's rows at tick 3: %, peer %, % bytes out, % events logged\n",wr_gate_name(s.gate),s.peer,k[0].used,l[0].count);
}
```

The transcript matches the hand trace. Under the session label, this
machine runs stage 0, so its outbound message is stage 0's rows to peer 1,
and the first attempt is refused before anything is logged or sent:

```text
project files: ok
  tiers: peer 0 owned peer 1 owned peer 2 vetted
  needs: stage 0 vetted stage 1 vetted
  stages on peers 1 2, latency 32089922 ns
  peer 0 sees prompt tokens, output tokens
  peer 1 sees prompt tokens, activations, output tokens
  peer 2 sees activations, output tokens
typed in the session, with project files: ok
  tiers: peer 0 owned peer 1 owned peer 2 vetted
  needs: stage 0 owned stage 1 owned
  stages on peers 0 1, latency 48391507 ns
  peer 0 sees prompt tokens, activations, output tokens
  peer 1 sees activations, output tokens
  peer 2 sees nothing
send stage 0's rows at tick 1: no consent, peer 1, no grant
grant peer 1 compute on session and project, ticks 2 to 100: ok
send stage 0's rows at tick 3: ok, peer 1, 14 bytes out, 2 events logged
```

## Refusals

| Status or gate | What it names | The smallest change |
| --- | --- | --- |
| `plaintext` | `from` → `to`, the first live link with bandwidth and no session keys | open a session on that link, or drop it |
| `exposure` | `stage`, `peer`, its tier `have`, the needed tier `want`, the class `cls` and kind `kind` | raise the peers in `fix` (fewest, then lowest bits); `tried` sets were solved |
| `place` | chapter 189's refusal with every peer trusted | chapter 189's `who` and `gap` |
| `input` | a label, policy entry, audit count or chapter 189 bound out of range | nothing to place |
| gate `consent` | the first remote peer, in stage order, without an active grant, and chapter 18's reason | a grant for that peer, `compute`, covering the label |
| gate `label` | parts with a class the placement was not made for | place again with that label |
| gate `placement` | the placement was refused | the placement's own fix |
| gate `send` | an unknown part, or `cs_send`'s refusal (a full sink) | — |

<!-- source: psionic/examples/prompt-trust-refuse.coax refuse -->
```jai
main :: () {
    j:=plcx_job();f:=plcx_fleet();t:=wrx_trust();p:=wrx_policy();zero:[1]int;none:=zero[0..<0];
    u:=t;u.sealed[1][2]=false;
    trust_why("peer 1 to peer 2 without keys",wr_place(j,f,u,p,WRX_PROJECT,PlcGoal.latency,none));
    u=t;u.evidence[1]=WrEvidence.{member=true,audits=5};
    trust_why("the desktop without its owner key",wr_place(j,f,u,p,WRX_SESSION|WRX_PROJECT,PlcGoal.latency,none));
    trust_why("identity data",wr_place(j,f,u,p,WRX_PROJECT|WRX_IDENTITY,PlcGoal.latency,none));
    trust_why("the whole model",wr_place(plcx_whole(),f,u,p,WRX_PROJECT,PlcGoal.latency,none));
    trust_why("label 32",wr_place(j,f,t,p,32,PlcGoal.latency,none));
    // The gate: a grant for peer 1 only.
    events:[16]CsEvent;grants:[8]CsGrant;texts:[8]string;own:[8]int;labels:[8]int;bytes:[256]u8;ends:[8]int;sent:[8]int;nb:[1]int;
    lb:=[1]CsLedger.[CsLedger.{events=events[..],grants=grants[..]}];l:=lb[..];
    gb:=[1]CsGraph.[CsGraph.{texts=texts[..],own=own[..],labels=labels[..]}];g:=gb[..];
    kb:=[1]CsSink.[CsSink.{bytes=bytes[..],ends=ends[..],events=sent[..]}];k:=kb[..];
    file:=[1]int.[cs_node(g,"fn parse(s: &str) -> Ast",WRX_PROJECT,nb[0..<0])];
    fact:=[1]int.[cs_node(g,"editor = helix",WRX_USER,nb[0..<0])];
    w:=cs_grant(l,CsDest.{kind=CsKind.peer,id=1,purpose=CsPurpose.compute},WRX_SESSION|WRX_PROJECT,1,100,1,1);
    proj:=wr_place(j,f,t,p,WRX_PROJECT,PlcGoal.latency,none);
    s:=wr_send(l,g[0],k,file[..],proj,2);
    print("send a project file through peers 1 and 2: %, peer %\n",wr_gate_name(s.gate),s.peer);
    s=wr_send(l,g[0],k,fact[..],proj,2);
    print("send a fact about the user to the same placement: %\n",wr_gate_name(s.gate));
    print("bytes out: %, events logged: %\n",k[0].used,l[0].count);
}
```

Each line is a refusal, and the example exits with status 0 after printing
them. No byte leaves in any of them; the one logged event is the grant.

```text
peer 1 to peer 2 without keys: plaintext link: link 1 -> 2 has no session keys
the desktop without its owner key: exposure: stage 1 would show session output tokens to peer 1 (vetted), which needs owned; raise peers 1 to owned (3 sets tried), then stages on 0 1
identity data: exposure: stage 1 would show identity activations to peer 1 (vetted), which needs owned; raise peers 1 to owned (3 sets tried), then stages on 0 1
the whole model: does not place: no peer has the memory, stage 0 misses peer 1 by 1932815360 bytes
label 32: input
send a project file through peers 1 and 2: no consent, peer 2
send a fact about the user to the same placement: label not placed
bytes out: 0, events logged: 1
```

The identity line shows why activations are in the policy at all: with
identity data, even the rows stage 1 receives need an owned machine.

## Checks

The checks are in `peer/trust.coax` (`#authority`), with their helpers in
[`peer-trust-checks.coax`](../tests/peer-trust-checks.coax):

- **Evidence.** Each piece removed from an owned peer drops its tier.
- **Exposure against a walk of the pipeline.** An independent walk sends
  two tokens through the pipeline message by message and records what each
  peer ends up holding. A `#forall` over 2,400 placements (1 to 8 stages
  on random distinct peers out of 8) compares it with `wr_exposed`.

<!-- source: psionic/tests/peer-trust-checks.coax walk -->
```jai
// Two tokens through the pipeline, message by message: what each peer
// ends up holding. This machine holds the prompt; each message into stage
// 0 carries the prompt's tokens, then the token sampled last; each stage
// computes hidden rows from what it got and sends them on; the last stage
// samples a token and sends it home.
wrx_walk :: (n:int,peer:[]int)->[8]int {
    seen:[8]int;seen[0]=WR_TOKENS;carry:=WR_TOKENS;
    for step:0..<2 {
        what:=carry;at:=0;
        for s:0..<n {
            seen[peer[s]]|=what;seen[peer[s]]|=WR_ACTS;
            what=WR_ACTS;at=peer[s];
        }
        seen[at]|=WR_OUTPUT;seen[0]|=WR_OUTPUT;carry=WR_OUTPUT;
    }
    return seen;
}
```

- **The hand-worked three peers**: both labels, the throughput goal, a
  peer two audits short, the desktop without its owner key, identity
  data, and every field of each answer.
- **The refusal table**: the plaintext link and the order links are
  checked in; links that are not refusals (no bandwidth, a peer that left,
  a peer to itself); exposure fixes of one peer and of two; chapter 189's
  refusal when nothing places however trusted; and the whole model on
  this machine needing no trust at all.
- **Input bounds**, just inside and just outside: the label, every policy
  entry, `vet`, audits and failures, this machine's evidence, and chapter
  189's own bounds.
- **The solver against a brute force.** A `#forall` over 800 random
  fleets (chapter 189's generator, 1 to 5 peers, 1 to 4 stages, both
  objectives, with and without an old assignment) with random evidence,
  policies, labels and an occasional link without keys. The brute force
  tries every set of raised peers (none first, then by size and bits) and,
  for each, every assignment of stages to distinct peers, keeping those
  whose walk exposure each peer's tier covers. The status, plaintext link,
  peers, cost, moves, fix, number of sets tried, exposures and named
  exposure all match; when nothing places, the refusal kind equals chapter
  189's enumeration.
- **The gate**: no grant, a grant, a part with fewer classes, a part with
  a class not placed, unknown parts, a placement whose stage 0 is remote
  and whose peer 2 has no grant, an expired grant on stage 0's peer, a
  refused placement, the whole model on this machine (nothing leaves) or
  on peer 1, and a full sink.
- `coax mutate --root-only` leaves no survivors (248 mutants), and
  chapter 189's `place.coax` none with its deny masks (415 mutants).

## What remains

- **The private tier.** Only an attested hardware enclave running a
  logged release keeps a request from the machine's owner. Verifying
  [NIP-ATT](../../nostr/nips/openagents/NIP-ATT.md) attestations, adding
  that tier, and failing closed when no attested peer is available are
  not built. Until then no placement here is private from the peers it
  uses; the policy only chooses which peers those are.
- **Live evidence and keys.** Evidence and `sealed` are written into a
  fixture. On a live mesh, membership comes from chapter 188's
  invitations, audits from chapter 195's ledger, and `sealed` from the
  session each link actually opened. Chapter 188's links run in process
  today.
- **Wired into a running split.** Nothing yet calls `wr_send` on a real
  pipeline, and stage peers do not re-check consent for the rows they
  forward.
- **Weight checks.** The draft asked that a weight-hash mismatch stop
  serving. Chapter 187's package verifier refuses a mismatched package;
  tying that refusal to a peer's tier is not built.
- **Coordinators.** There is no coordinator in this design: this machine
  sends directly over sealed links, and chapter 188's relays forward
  sealed frames they cannot open. A coordinator that decrypts to route or
  bill would break every claim here, and nothing checks for one.

## Dependencies

- [Chapter 186](ch186-split-a-model-into-stages.md): the stage contract
  that fixes what crosses each boundary.
- [Chapter 188](ch188-peers-transport-and-discovery.md): invitations and
  per-direction session keys.
- [Chapter 189](ch189-place-the-model.md): the solver and its deny masks.
- [Chapter 195](ch195-check-a-peers-work.md): audits as evidence.
- [Coder chapter 18](../../coder/book/ch018-consent-before-sharing.md):
  data-class labels, grants and the send gate.
- [Chapter 27](ch027-a-public-mesh.md): the trust scope of the public mesh.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [NIP-ATT](../../nostr/nips/openagents/NIP-ATT.md)
