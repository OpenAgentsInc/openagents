# ngit and GRASP — Source Analysis and OpenAgents Use Plan

**Date:** 2026-07-21
**Lane:** Reference analysis (`docs/ngit/`). This document flips no promise
state, changes no runtime authority, mints no issue, and dispatches no work.
Candidate uses require normal Sol admission.
**Sources (all read at the named commits, cloned under
`~/work/projects/repos/` and tracked in the workspace `projects/manifest.txt`):**

| Repo | Commit | State |
| --- | --- | --- |
| `DanConwayDev/ngit-cli` | `6d806d5` (2026-07-10, v2.6.3) | Active, MIT |
| `DanConwayDev/ngit-relay` | `632be04` (2026-05-06) | **Archived**, superseded by `ngit-grasp` |
| `DanConwayDev/gitworkshop` | `b049b16` (2026-07-19) | Active, **no license file** |
| `soapbox-pub/shakespeare` | `5d02627` (v9.14.0) | Active, AGPLv3 |
| `soapbox-pub/nostrify` | `ec68767` (2026-07-06) | Active, MIT |

**Companions:**
[`2026-07-21-soapbox-what-is-ngit-source.md`](2026-07-21-soapbox-what-is-ngit-source.md)
(the captured Soapbox article this folder responds to),
`docs/teardowns/2026-07-21-buzz-teardown.md` §7 (the Buzz git deep dive),
`docs/fable/2026-07-21-nostr-native-pivot-analysis.md` (the pivot
architecture; ngit is the concrete substance of its Plane 3 and stage N5).

---

## 1. Summary

ngit is the reference implementation of git collaboration over Nostr: a Rust
CLI plus a `git-remote-nostr` transport helper that make `nostr://` a normal
git remote. Its architecture separates three planes cleanly — discovery
(signed kind 30617 repo announcements), ref authority (signed kind 30618
state events), and object transport (ordinary git smart HTTP against any
listed server). GRASP is the thin server convention that co-locates a Nostr
relay and a git server on one host so both planes can be self-hosted
together, mirrored across several hosts at once.

The single sharpest idea in the stack: **the signed state event is the push
credential.** The archived GRASP reference server accepts a branch push if
and only if the pushed commit already equals the ref value in the latest
maintainer-signed 30618 event. No accounts, no tokens, no SSH keys — the
signature on intent authorizes the mutation. That is the verification thesis
OpenAgents runs on, applied to git hosting, and it is the pattern worth
porting even where the specific implementation is not.

The Soapbox article that occasioned this folder conflates two things worth
keeping separate: ngit's protocol family and Buzz's forge. They share the
NIP-34 event vocabulary but differ materially on the PR model, auth, and
hosting (§5). That divergence is precisely the standards-play opportunity
the pivot analysis names.

## 2. What ngit actually is

Two binaries from one Rust codebase (`ngit-cli`, ~69k LOC with tests, on
`rust-nostr` and libgit2):

- **`ngit`** — the porcelain: `init` (announce a repo), `send` (patches),
  full `pr`/`issue` subcommand families (list/view/checkout/apply/close/
  merge/label), account management, sync.
- **`git-remote-nostr`** — a standard git remote helper. Git invokes it for
  any `nostr://` remote; it speaks git's stdin/stdout helper protocol
  (`capabilities`/`list`/`fetch`/`push`), translating between Nostr events
  and git operations.

### 2.1 The three planes

1. **Discovery — kind 30617 (addressable).** `ngit init` publishes the repo
   announcement: `d` identifier (defaulting to the first 7 chars of the
   earliest unique commit), `r <commit> euc` anchor, `name`, `description`,
   `clone` URLs, `relays`, `maintainers`, `web`, optional `blossoms`.
   Unknown tags round-trip verbatim — a deliberate forward-compatibility
   posture worth copying. The `euc` (earliest-unique-commit) anchor groups
   forks and re-announcements of the same repo across hosts and authors.
2. **Ref authority — kind 30618 (addressable).** One tag per ref
   (`refs/heads/*`, `refs/tags/*` with peeled `^{}`, `HEAD` symref). On
   fetch, the helper advertises refs *from the signed state event*, not from
   any git server — the server supplies objects, never truth. On push, git
   data goes to every listed server, then a new signed 30618 is published.
   Ref authority lives with the maintainer keys, not the host.
3. **Collaboration — the NIP-34 event family plus ngit extensions.** Patches
   (1617, real `git format-patch` mbox in content), issues (1621), status
   (1630–1633), comments (NIP-22 kind 1111), labels (NIP-32 kind 1985), a
   custom cover-note kind (1624), and the ngit/GRASP pull-request dialect:
   kinds **1618/1619**, where the PR event carries a pointer to commit
   objects pushed to `refs/nostr/<event-id>` on a git server rather than
   inline patch content. When no GRASP server is available, ngit falls back
   to plain 1617 patches.

### 2.2 Keys and signing

ngit's custody posture is modern and matches ours: full **NIP-46 bunker
signing** (bunker:// and interactive nostrconnect:// QR flows, persisted in
git config), **NIP-49** scrypt-encrypted local keys (`ncryptsec` in
`nostr.nsec`), and the same signer answering relay NIP-42 AUTH. An agent or
developer never needs a plaintext nsec on disk to use ngit.

## 3. GRASP, precisely

GRASP ("git collaboration over relays and smart HTTP") is a server
convention, not a protocol rewrite. Its spec lives at
`gitworkshop.dev/spec/grasp` — authored by the gitworkshop/ngit side, not in
`nostr-protocol/nips`:

- **GRASP-01**: one host serves both a Nostr relay (`wss://host`) and git
  smart HTTP at `https://host/<npub>/<identifier>.git`. Servers advertise
  support via NIP-11 `supported_grasps: ["GRASP-01"]`.
- **GRASP-06**: a contributor endpoint `/prs/<signer-npub>/<identifier>.git`
  so non-maintainers can upload PR objects without write access to the
  canonical repo path.
- **Kind 10317**: a user's replaceable list of preferred GRASP servers as
  `g` tags — the git analog of a NIP-65 relay list.

The archived reference server (`ngit-relay`, Go: khatru relay + nginx +
`git-http-backend` + hook binaries under supervisord) shows the mechanics:

- A kind 30617 announcement naming the host in both `clone` and `relays`
  tags **provisions the bare repo automatically** — announcement as
  infrastructure request.
- The relay stores only events that relate to hosted repos (pointer-graph
  scoping), plus rate limits and size guards.
- Push authorization is the pre-receive hook rule described in §1: state
  event match for `refs/heads/*` and `refs/tags/*`; `refs/nostr/<event-id>`
  always accepted (PR objects); `refs/heads/pr/*` rejected over git (PRs
  must travel as events). No NIP-98, no NIP-42, no accounts.
- Multi-host resilience is proactive: `post-receive` and a sync daemon pull
  from the other announced git servers, and gitworkshop's client pushes
  signed state to every mirror, force-aligning laggards.

The successor, **ngit-grasp** (single-binary rewrite), is not on GitHub —
its development is hosted over ngit itself, which is both dogfooding and a
practical note: our sync lane can mirror it only once we can pull
`nostr://` remotes.

## 4. The ecosystem around it

- **gitworkshop.dev** (525 files, React/applesauce, extremely active, sole
  author) is the mature web client: a custom in-browser smart-HTTP git stack
  (capability negotiation, packfile build/parse, SHA-1 hashing via WebCrypto
  — not isomorphic-git), two-tier content-addressed object caching
  (memory + IndexedDB), outbox-model relay selection, and a full
  issue/PR/review UI including inline suggestions and CI events on Nostr.
  Its **trust model** is the most thought-through part: a repo's maintainer
  set is the fixed point of mutually-listed 30617 announcements reachable
  from the user's one chosen trust anchor; state, status, and label events
  are authoritative only from that recursive set. This guards the
  "unilateral listing" attack (announcing a repo with someone else's name
  in the maintainers tag). Caveat: **no license file** — reference-only,
  never a code source.
- **Shakespeare** (Soapbox, AGPLv3) proved adoption: an in-browser AI app
  builder (isomorphic-git + LightningFS + esbuild-wasm) where every project
  can be announced as a NIP-34 repo and pushed to GRASP servers with one
  click — thousands of repos, which is how the protocol got real users. Its
  AI-provider menu (including Lightning/Nostr-paid inference) is an
  existence proof of the paid-agent rails the pivot analysis targets.
- **nostrify** (Soapbox, MIT) is the underlying generic Nostr framework
  (pool, relay policies, signers incl. NIP-46, Postgres/IndexedDB relay
  backends). It has **no NIP-34 support** — the git logic lives in the apps.
  Useful as a comparison point for `nostr-effect`'s architecture, not as a
  dependency.

## 5. ngit versus Buzz git — the precise relationship

The article's "now shipping inside Buzz" framing overstates convergence.
From source (both sides audited; neither codebase mentions the other):

| Dimension | ngit/GRASP | Buzz forge |
| --- | --- | --- |
| Ref authority | Maintainer-signed kind 30618; servers follow | Object-store CAS pointer; **relay-signed** 30618 emitted after commit |
| PR model | Kinds 1618/1619 pointing at `refs/nostr/<event-id>`; 1617 mbox fallback | Kind 1618 with `target-branch` tag; reviews as labeled kind-1 notes; desktop-driven merge |
| Read access | Open; no auth to clone | Members only; NIP-98 on every git route |
| Push auth | Signed state event is the credential (pre-receive match) | NIP-43 membership + `buzz-protect` policy hook |
| Hosting | Any git server; GRASP-01 co-located relay+git; multi-mirror | The relay itself over smart HTTP; S3 CAS packs; single workspace relay |
| `nostr://` remotes | Core feature (`git-remote-nostr`) | Absent; HTTP(S) clone URLs pinned to the workspace relay |
| Kind 10317 | Core (GRASP lists) | Not registered |
| Commit signing | Git PGP sig preserved in patch tags | NIP-GS Schnorr x509-interface signing |

What they truly share: kinds 30617/30618/1617/1621/1630–1633, the NIP-34
tag vocabulary, and the sovereignty argument. What has forked: the PR
dialect (two incompatible 1618s), auth philosophy (open-plus-signed-state
versus membership), and hosting topology (many mirrors versus one relay).
There are now **three PR dialects** in the wild — stock NIP-34 patches,
ngit's ref-pointer PRs, and Buzz's target-branch PRs — which is exactly the
fragmentation a second implementer with standards intent can help resolve,
per the pivot analysis §VIII. `nostr-effect` at `c160378` already implements
both sides' vocabularies (1617–1633, 30617/30618, 10317 with `euc` and
GRASP-list builders, plus Buzz's NIP-GS), which makes OpenAgents the only
party positioned to write the harmonization.

## 6. How OpenAgents should use this

Candidate uses, ordered by leverage; all map onto the pivot analysis
(Plane 3, stages N5–N7) and require normal admission:

1. **Adopt the state-event-as-credential pattern.** The teardown's §7.9 git
   profile already says "refs or an admitted object-store pointer are
   repository authority; publish 30618 as a signed projection." ngit-relay's
   pre-receive rule is the complementary write-side: admit a push only when
   it matches signed maintainer intent. For OpenAgents-hosted repos this
   composes with our verification thesis directly — the same signed-intent →
   admitted-mutation → receipt shape as everything else we build.
2. **Dogfood as a client first (cheap, immediate).** ngit is an installable
   MIT tool. Mirror selected OpenAgents repos to public GRASP servers with
   `ngit init` (announcement + state only; GitHub remains canonical), and
   let a coding agent submit one patch/PR over Nostr end-to-end. This is
   N5's first receipt and costs almost nothing — the fleet can run `ngit`
   exactly as it runs `git` today.
3. **Interop-test `nostr-effect` against the live dialects.** We have the
   event builders; what we lack is proof against ngit, gitworkshop, and
   Buzz behavior. Concrete matrix: our 30617/30618 events readable by
   gitworkshop.dev; ngit-cli clones a repo we announce; our reader consumes
   ngit's 1618/1619 + `refs/nostr/*` PRs and Buzz's `target-branch` 1618s.
   Vector failures become upstream issues — filed over ngit, which is also
   the introduction to the maintainer.
4. **GRASP capability for an owned relay.** If/when N1 (owned relay on
   Google Cloud) is admitted, add GRASP-01/06 as a policy module: serve git
   smart HTTP beside the relay, provision from admitted announcements only
   (unlike ngit-relay's open provisioning — our admission gates stay),
   enforce the pre-receive state-match rule. The Go implementation is
   archived reference material; ours would be `nostr-effect` + our existing
   infra. Do not port khatru.
5. **Agent trust import via the maintainer chain.** gitworkshop's
   recursive-maintainer trust model plus the `euc` coordinate is a working
   web-of-trust for code. An agent card (pivot §VII Plane 0) can cite
   NIP-34 history — repos maintained, patches merged by whom — as
   verifiable reputation, complementing NIP-39 GitHub proofs during the
   long GitHub-coexistence period.
6. **Portable sessions name repos by coordinate.** `30617:<pubkey>:<id>` +
   `euc` + a GRASP list is a host-independent repo name — precisely what
   the portable-coding-sessions spec needs instead of a host account URL.
7. **Watch, don't adopt, the browser git stacks.** gitworkshop's in-browser
   packfile client is impressive but unlicensed; Shakespeare's
   isomorphic-git stack is AGPL. Both are architecture references for any
   future web-side repo viewing, nothing more.

## 7. Risks and limitations

- **Dialect churn.** Kinds 1618/1619 mean different things in ngit and
  Buzz; GRASP is specified outside `nostr-protocol/nips`; ngit's own server
  story just moved (`ngit-relay` → `ngit-grasp`). Pin exact commits and
  re-verify before any interop claim.
- **Bus factor.** ngit-cli and gitworkshop are effectively one very prolific
  author (OpenSats-funded). Sole-maintainer risk cuts both ways: fragile,
  but also receptive — a serious second implementer changes the project's
  slope.
- **License gaps.** gitworkshop has no license; treat as read-only
  reference. Shakespeare is AGPL — patterns yes, code no.
- **Open reads are a feature and a bug.** GRASP's no-auth reads fit public
  open source, not private customer repos. Our profile needs the Buzz-style
  authenticated-read option without importing Buzz's membership coupling —
  both postures behind one admitted policy.
- **Dependency maturity.** ngit pins `rust-nostr` 0.45 alpha; the ecosystem
  moves fast and breaks (khatru breakage is why ngit-relay died). Another
  argument for interop at the event layer, not the dependency layer.

## 8. Watch items

- **`ngit-grasp`** — the single-binary GRASP server, self-hosted over Nostr
  (not on GitHub). Becomes syncable by us once a `nostr://` pull path
  exists in our tooling; its shape will define GRASP's next revision.
- **GRASP spec drift** at `gitworkshop.dev/spec/grasp` (GRASP-01 through at
  least GRASP-06) — candidate for co-authored upstreaming alongside the
  Buzz collision fixes (pivot §VIII.2).
- **gitworkshop CI events** — CI results as Nostr events is a small,
  unclaimed spec surface adjacent to our verification receipts; if we emit
  build/verification receipts on Nostr, aligning kinds with gitworkshop
  costs little and buys a rendering client.
- **NostrHub and MKStack** (Soapbox, GitLab-hosted, no GitHub mirrors) —
  browser repo creation and GitHub→Nostr mirroring; revisit if the sync
  lane grows a GitLab path.
