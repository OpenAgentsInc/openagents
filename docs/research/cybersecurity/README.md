# Cybersecurity realms: initial research notes

Status: speculation, 2026-10-31. This is a starting point for web research,
not a plan. Market claims below come from memory and need checking before
anyone relies on them. Each section ends with open questions and searches to
run.

## Why this fits the Verse

Episode 290 introduced realms as games. Realms can also be training grounds
and test environments. Cybersecurity is a strong fit because the field
already learns through play. Capture the flag (CTF) contests, cyber ranges,
and red-team versus blue-team exercises are all games with rules, scores, and
opponents.

Our stack has four parts that security training platforms rarely have
together:

| Part | What it gives a security realm |
| --- | --- |
| Aethyr and the Verse | A spatial, multiplayer, web-first view of a network: hosts as buildings, traffic as movement, alerts as events you can see and watch. |
| Psionic, Pylons, and decision models | Local and pooled inference for adversary and defender agents, plus training and evaluation of those agents. |
| Coder and the environment NIPs | Real sandboxes and real terminals bound to objects in the world (see [NIP-ENV](../../../nostr/nips/openagents/NIP-ENV.md) and [NIP-TERM](../../../nostr/nips/openagents/NIP-TERM.md)). |
| Bitcoin wallets and Nostr | Payouts for challenge authors, bounties, signed scoreboards, and portable skill records (see [NIP-EVAL](../../../nostr/nips/openagents/NIP-EVAL.md), [NIP-XP](../../../nostr/nips/openagents/NIP-XP.md), and [NIP-ATT](../../../nostr/nips/openagents/NIP-ATT.md)). |

The working thesis: most security training is a terminal in a browser tab
plus a scoreboard. It is hard to watch, hard to explain to a non-expert, and
the opponents are static. A realm can make the network visible, make the
opponents adaptive, and make the whole exercise replayable and spectatable.

## The existing market, roughly

Verify every name and claim here. These are the categories as we remember
them:

- **Hands-on labs for individuals.** Hack The Box, TryHackMe, PortSwigger Web
  Security Academy, OffSec Proving Grounds, picoCTF. Mostly offensive, mostly
  single-player VMs with a flag to find.
- **Enterprise cyber ranges.** SimSpace, Cyberbit, RangeForce, Immersive
  Labs, Cloud Range, and government ranges. Expensive, sold to SOC teams and
  defense, often with scripted attack replays.
- **CTF hosting.** CTFd, rCTF, and event-specific infrastructure. Good at
  scoring, not at environments.
- **Adversary emulation tools.** MITRE Caldera, Atomic Red Team, and lab
  builders such as GOAD (Game of Active Directory). Powerful, but you assemble
  them yourself.
- **AI security agents and benchmarks.** DARPA's AI Cyber Challenge (AIxCC),
  Google's Big Sleep, XBOW, PentestGPT, and benchmarks such as Cybench and the
  NYU CTF set. Labs and governments run cyber capability evaluations on
  frontier models and need many fresh, graded, reproducible tasks.

Questions:

- Who sells blue-team (defender) training to individuals, not enterprises?
  Our guess is that this segment is thin. Offensive labs dominate the
  individual market because breaking things is more fun than reading logs.
- What do enterprise ranges cost per seat or per exercise?
- Which ranges use live, adaptive AI opponents instead of scripted replays?
- How do model labs and AI safety institutes source cyber evaluation tasks
  today, and what do they pay for them?

Searches: "cyber range pricing per seat", "blue team training platform
individuals", "AI adversary cyber range", "AIxCC results open source",
"Cybench benchmark tasks", "cyber evaluation frontier models task sourcing".

## Gaps we might fill

### 1. Defense that is fun to play

Blue-team work is watching, correlating, and responding. In a terminal it
looks like reading. In a realm it can look like Everglade: you stand in a town
that is the network, and an intruder moves through it. Hosts are buildings,
users are townspeople, logs are what the guards saw, and lateral movement is
someone walking between houses who should not be there.

This is the same idea as the Everglade guard ladder: escalating defenders
that react to the player. Swap the roles. The player is the guard captain and
the AI is the intruder.

Open question: does a spatial view help people learn real skills, or is it a
gimmick? The view has to map to real artifacts (real logs, real process
trees, real packet captures), so that what you learn transfers to a real
SIEM. Look for research on visualization in security operations and on
game-based security education outcomes.

### 2. Adaptive AI opponents

Most ranges replay a fixed attack. A range with an agent attacker that
changes tactics each run is closer to the real world and cannot be beaten by
memorizing a walkthrough. Episode 290 already describes the goal for game
NPCs: "MMOs where the NPCs are able to coordinate using like AI
conversations to be like truly formidable enemies." A coordinated red-team
NPC crew is the security version of that.

Decision models matter here. A quarter-second decision on a gaming computer
is fast enough to drive an opponent's next move in real time, and it is
cheap enough to run per player on a Pylon or in the browser.

### 3. Agent versus agent, with humans watching

Attack and defense CTFs, where each team defends its own services while
attacking the others, are the most exciting format and the hardest to host.
A realm can run agent teams against each other, let humans join either side,
and stream the match the way esports stream games. Spectating is the part
current platforms do not offer. It is also the part that grows an audience.

### 4. Ranges as training and evaluation environments for defensive agents

Every range run produces a graded trajectory: what the attacker did, what the
defender saw, what the defender did, and whether the attack worked. That is
reinforcement learning data and evaluation data for defensive agents.

- Psionic can train small defensive policies and decision models on these
  runs.
- NIP-EVAL reports can publish attributable results per agent, per suite.
- Deterministic engine replay (the lockstep work for Belt 2200) means any run
  can be replayed exactly for after-action review or to reproduce a failure.

The market angle: labs and evaluation groups need fresh, uncontaminated
cyber tasks. Realm authors who build challenges could sell evaluation access
to them, paid in bitcoin, while public versions stay free to play.

### 5. Industrial and physical systems (ties to robotics)

Operational technology (OT) and industrial control system (ICS) security
training is expensive because it needs physical hardware: PLCs, pumps,
valves. Our robotics simulation work and the engine's physics give us a
digital twin instead. A simulated water treatment plant, substation, or
warehouse robot fleet where an attack has a visible physical result is
memorable and far cheaper than a hardware lab. The ERCOT API specifications in the
workspace reference set (`projects/ercot/`) are a possible source for a realistic grid scenario.

Searches: "ICS security training lab cost", "PLC simulator security
training", "digital twin cyber range OT", "Modbus honeypot simulation open
source".

### 6. Challenge authors who get paid

The Verse plan already pays creators from paid usage. Applied to security:
anyone can build and fork a challenge realm, and the author earns when people
play it, when a lab licenses it for evaluation, or when a team buys a private
copy for an exercise. CTF authors today mostly work for free or for one event.

### 7. Skill records you own

A Nostr-signed record of what you solved, when, and under what conditions is
a portable credential that no single platform controls. It could sit next to
certifications such as OSCP as evidence of hands-on work. Research what
hiring managers trust today and whether signed, replayable proof of work
would change that.

### 8. Our own code as the first target

The OpenAgents repository is open source and Episode 290 promises XP and
rewards for contributions. A real responsible-disclosure program for our own
realms, relay, and Pylon software, with bitcoin bounties and XP, is a
low-cost first step. It also keeps the first "real" targets ones we own.

## Ways to run the environments

Each option trades realism against cost and risk.

| Tier | How | Cost | Realism | Notes |
| --- | --- | --- | --- | --- |
| Simulated | Hosts, services, and logs modeled inside the realm, in Coax. | Near zero. Runs in the browser. | Low to medium. | Good for blue-team basics, onboarding, and phones. No real exploits. |
| Emulated in the browser | A small Linux system emulated in WebAssembly (for example v86) with a terminal in the world. | Near zero server cost. | Medium. | Real shell, real tools, sealed in the player's tab. Research performance limits. |
| Sandboxed VMs | Per-player or per-team VMs through env-host on our GCP, with a terminal bound to a realm object. | Real cost per hour. | High. | Must stay inside the $1,500 a month GCP limit. Short leases, idle shutdown, paid tiers. |
| Pylon-hosted | Ranges on contributor machines. | Paid in bitcoin to the host. | High. | Hard no for vulnerable VMs until isolation is proven. A vulnerable VM on a gamer's home network is a risk to that gamer. |

Firecracker (microVM isolation) and wterm (a browser terminal) in the
workspace reference set are relevant to the sandboxed and emulated tiers.

## Boundaries

These are rules for us, not open questions.

- **Sealed targets only.** Every target is a system we run or the player runs
  in a sandbox. Nothing in a realm points at hosts we do not own. Network
  egress from range VMs is blocked by default.
- **Defense first.** Offensive skills are taught because defenders need them,
  inside sealed ranges, against intentionally vulnerable targets. We do not
  ship working attacks against real software that has no fix.
- **Agent capability gating.** An attacker agent that works inside a range
  must not be usable as a general attack tool outside it. Its tools are scoped
  to the range's addresses and leases.
- **Isolation before scale.** Sandbox escape is the main technical risk. Prove
  isolation on our own VMs before anything runs on Pylons.
- **Legal review.** Research computer-misuse law (the US CFAA and equivalents)
  as it applies to hosting ranges and to user-authored challenges.

## First realm ideas

Three small, original concepts to test the thesis. Each is a sketch.

1. **Watchtower (blue team, browser only).** A walled town is a small
   company network. An AI intruder enters through a phished user and moves
   between buildings. You read what the guards saw (real-format logs), find
   the intruder, and lock doors (firewall rules, account resets) before it
   reaches the treasury. Runs fully simulated, so it works on a phone and costs
   nothing to host. Tests whether defense can be fun.
2. **Two Keeps (attack and defense, agents and humans).** Two teams each hold
   a keep running a few vulnerable services on sandboxed VMs. Each team
   patches its own services and attacks the other's. Agent teams fill empty
   seats. Spectators watch from the ramparts. Tests the esports and
   agent-versus-agent ideas.
3. **Pumphouse (industrial twin).** A simulated water plant with pumps,
   tanks, and a control network. An attacker changes setpoints and the tanks
   visibly overflow. The defender spots the change in the control traffic and
   restores safe operation. Tests the digital-twin idea and links to the
   robotics work.

## Open research questions

- Which security training segments are underserved, by price and by learner
  type?
- Does game-based or spatial security training measurably improve skill
  transfer? Look for academic studies.
- What do model labs and AI safety institutes need from cyber evaluation
  environments, and who supplies them today?
- How good are current AI attacker and defender agents in public benchmarks,
  and where do they fail?
- What isolation stack do commercial ranges use (Firecracker, gVisor, full
  VMs), and what does a per-player hour cost?
- How far can browser emulation go: which real tools run in a WebAssembly
  Linux, and how fast?
- What would make a Nostr-signed skill record credible to employers?
- What are the legal and policy constraints on hosting user-authored
  offensive challenges?

## Next steps

1. Run the searches above and add findings with sources to this folder, one
   file per topic.
2. Pick one first realm (we lean toward Watchtower, because it costs nothing
   to host and tests the least-served segment).
3. Write its game design document in `verse/` next to the other realms.
