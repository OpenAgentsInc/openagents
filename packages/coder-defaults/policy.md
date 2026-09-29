# Coder defaults adoption policy

A tool is a candidate for Coder's defaults when a published result on a
test set is Better, checks by at least three distinct trainers confirmed
it, and at least one Better result on a second test set externally
validates it. A trainer's linked keys count as one trainer, and a check by
the result's trainer or the test set's author doesn't count. A validating
test set counts only when someone other than the tool's author released
it, after the tool's release, on the same task distribution: reproduction
on the author's own tests proves reproducibility; this proves the result
wasn't fitted to them.

A candidate is adopted only when an OpenAgents operator decides to adopt
it. The decision is an `openagents.eval-admission.v1` document that cites
the confirmed reports and the validating result, and the next
`coder-defaults` release depends on the tool's release and cites the
decision. Once the defaults hold anything, the decision should also cite a
marginal report, current defaults plus the candidate against current
defaults alone, and a regression check across the whole set; the evidence
asked of a candidate rises with what a wrong decision by it could do.

The admission's lifetime follows the tool's identity: the weaker the
identity, the sooner the evidence expires. A content-addressed tool
(every extension under a lock) is admitted for 365 days, a
version-addressed subject for 90, an endpoint-addressed one for 14, and a
subject whose identity is unresolved is never adopted;
`microcoder xp adopt --expires-days` overrides the default. A gate change
reinterprets the cited reports and reopens nothing; a change to the tool,
the defaults, the agent build, the grant, or the graders does.

Adoption earns XP under NIP-XP's `eval-adopt` rule. XP is never spent,
transferred, or converted, and adoption pays no money.

## How a release reaches runtimes

A release is a NIP-EXT `3184` signed by this package's root key. Its
manifest (pinned by digest) lists the adopted tools' release IDs as
`dependencies` and cites each admission in `provenance.receipts`; the
manifest and each admission are kept in `documents/` here, named by
digest, and the adopter publishes a NIP-94 locator for each so a runtime
can fetch them from this repository. A runtime that consumes the defaults
does what a ledger does before crediting an adoption
(`xp_ledger::defaults::current`): it reads the root's releases, takes the
newest whose manifest it holds, and admits a dependency only under a
live admission it also holds: the admission's bytes are the ones the
manifest cites, its decision is `admit`, its subject's release is that
dependency, and its `expires_at` hasn't passed. A dependency with no
such admission is named as lapsed and admits nothing; a release whose
manifest a runtime can't get admits nothing at all. Every run records
the defaults lock it admitted under (`openagents.coder-defaults-lock.v1`:
the release, the manifest digest, and each admitted subject with its
admission digest and expiry).

Two runtimes consume it today:

- **The hosted runner** (`crates/eval-runner`) reads the relay at each
  admission, with the documents from its host's
  `~/.openagents/coder-defaults/documents` (where `microcoder xp adopt`
  keeps them on the referee host) and then from the locators. It admits
  the adopted extensions it holds in its catalog in **both** arms, so a
  report is marginal (`meta.ext_eval.defaults` names the release, and each
  arm's lock names the defaults lock); an adopted extension outside its
  catalog is logged as not held and admits nothing.
- **Coder on a computer** (`coder -p`, the terminal, and the turns the
  app dispatches to a connected computer) reads one directory
  (`CODER_DEFAULTS`, else `~/.openagents/coder-defaults`) that
  `openagents ext defaults sync` writes: `lock.json`, the admitted
  extensions' programs, and their skills. The session grants the admitted
  programs on top of the operator's `CODER_PROGRAMS` (never widening the
  effects ceiling), appends the skills to its instructions, notes the
  lock in its trace, and records the lock's digest in each program run's
  run-state claim. The sync resolves an adopted release against the
  `--catalog` directories it is given and the extensions installed under
  `~/.openagents/extensions`, by the manifest digest the local bytes
  would release as; a release it can't match is named and admits
  nothing.

Neither runtime widens what an operator allowed: a default admits a
program, and the operator's effects ceiling still bounds what it may do.
