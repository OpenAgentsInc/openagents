# Creator publication and account safety evidence

Issue [#10744](https://github.com/OpenAgentsInc/openagents/issues/10744)
implements V26 of the [Verse audit](../../../../docs/audits/2026-10-04-verse-engine-audit.md).
`checks.json` records the final source binding and executable identities;
`sha256.json` binds the retained artifacts. The world package passes 607 tests
with three ignored benchmarks. The content compiler package passes 29 tests
with one ignored optional import. The final content hardening check is recorded separately in
`content-final-tests.log`. `consumer.log` records the native remote-chamber
example check after integration with current main.

`namespace.json`, `namespace-recipe.py`, and `namespace.log` retain an actual
isolated build and release export. Bubblewrap unshares the network and mounts
only the public checkout, pinned Rust toolchain, public Cargo caches, Nix tools,
existing warm target, and scratch output. Private game directories, private
sibling repositories, and the owner's OpenAgents state are absent. Cargo builds
the compiler offline with the lockfile fixed. The original procedural ritual
recipe initializes a new workbench and exports a public release. This is a
content release fixture built in the contributor dev profile; it is not a full
engine production release or the repository release gate.

`release.json` inventories 15 payload files, 6,923,154 bytes, and 51 admitted
asset identities. Its origins declare Apache 2.0, CC BY 3.0, and OFL 1.1. It
contains no host template, journal, preview, or Studio data. The descriptor
retains exact payload hashes; the large payload remains reproducible through
the retained recipe and committed compiler sources. Provenance declarations
do not certify license compliance. Retained source notices remain in the repo.

`tls-loop.json` and `tls-loop.log` record the actual two-listener TLS fixture.
Two authenticated accounts use typed SDK block and report calls; the block
target comes from the bounded public avatar-to-account contact projection; the trusted
operator handle privately reads and completes the report. Exact retries and
restart preserve the outcomes. The fixture also exercises the previously
implemented party, gear trade, quest cycles, transfer, and logout/resume loop.
It is a correctness fixture, with 64 authority ticks and 41 client requests,
not a raid load, sustained 30 Hz target, or phone/browser moderation UI test.
Two authentication refusals are expected from fenced connections; no host,
frame, or work-budget failure occurred. During the concurrent compiler build,
the scheduler dropped about 8 ms of elapsed time and peak authority work was
about 90 ms. This receipt does not establish a frame-time or tick-latency target.

Package checks cover unapproved publication, signed artifact/authority binding,
suspension, withdrawal with missing files, failed-review poisoning, canonical
JSON including discarded duplicate entries, resealed hidden fields, byte and
symlink substitution, incompatible profiles, account privacy, blocked contact,
report quotas, completed retries, and restart. Five process-death boundaries
select either the old report/preferences/receipt set or the complete new set.
The existing world-only Studio view and operation denial passes, and closed
public scene and wire schemas refuse host execution, Studio reads, publisher
approval, and report-resolution payloads.

The publication book is a trusted local operator SDK and CLI. It is not a
marketplace, distributed moderation service, or automatic running-world
withdrawal mechanism. Consumers resolve again before new distributions and
stop withdrawn running instances. Account blocks fence contact rather than
hiding physical avatars or removing existing members. Report status records
an operator verdict and grants no execution authority. Active state and rates
are bounded; immutable receipts remain subject to host storage and retention.
