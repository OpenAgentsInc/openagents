# Brainstorm integration for Coder V1

Proposal dated October 6, 2026. Target release: October 7, 2026.
This document proposes implementation; it does not report a shipped integration.
The [Rust client](../../crates/brainstorm-client/README.md) implements the bounded
HTTP adapter and local fixtures for REV-34 (#10841). Deployed qualification is an owner
check in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).

The [native Brainstorm module](../../crates/coder-new/src/brainstorm.rs) adds
disabled persisted settings, explicit commands, cancellation, and bounded live
conversation observations for REV-35 (#10842). REV-36 (#10843) adds the same
client's exact-input native admission and OpenRouter dispatch. The
[guidance companion](../../plugins/brainstorm/README.md) for REV-37 (#10844)
packages inert instructions, an exact descriptive host/source requirement, and
offline pilot examples through the existing EXT path. Its private checker
validates source pins and exact-key projections; manual funnel rows remain
claims. These implementations do not report a packaged release, public profile
publication, live discoverability, or qualified buyer conversion.
The Coder review uses OpenAgents commit
`f708ce07c072f96e833d17abc3eec59673c174c8`, where `coder-new` is
`1.0.0-rc.3`. Public Brainstorm discovery and read requests succeeded on
October 6 without credentials.

## Recommendation

Ship one **Brainstorm** plugin, bundled with Coder and off by default, with
two public reads: search Nostr account profiles and look up their Brainstorm
reputation. Use a small Rust host adapter against the Open Ranking HTTP API.
Users need no API key, Nostr secret, or local Brainstorm server.

Provide explicit lookups through `/brainstorm` on every model path, and expose
the same operations to OpenRouter's existing plugin dispatch. Local Codex and
Claude paths can discuss observations already collected by the host. Automatic
local-model invocation can follow separately; tomorrow's release does not
need a general plugin bridge in Microcoder.

Add full Nostr content search and signed NIP-85 score inspection next, within
this plugin. Give a future **Nostr Observer** newspaper integration its own
opt-in because generation, scheduling, and publication carry different costs
and permissions.

## Product goal: two-way discovery

The integration should eventually connect public agent and plugin discovery
in both directions: Coder can find public identities through Brainstorm, and
Brainstorm users can find OpenAgents publishers and services by their
advertised capabilities. This is more useful than a reputation meter alone.

Keep four evidence layers visible on a future discovery card: the exact
Nostr identity, supported NIP-CAP operations, scoped NIP-EVAL results or
NIP-XP awards, and Brainstorm's social reputation. Scores and evaluations
describe different things. Resolve a candidate key to signed OpenAgents
records before offering its operations; search relevance is not capability
availability or proof that a service can execute work.

For V1, prepare an optional discoverability pilot with an already-authorized
public development identity. Publish an owner-approved kind-`0` profile
containing a public description, canonical website, and links to public
capability/release records. Verify that Brainstorm's profile search finds it,
then look up that exact key in Coder. Treat absence or zero scores as limited
coverage. Pilot publication is a separate explicit action and does not happen
when the Brainstorm plugin is enabled.

Do not assume Brainstorm indexes OpenAgents kinds `30180`, `30184`, or
`3184` as searchable capability/release documents today. Its existing profile
and repository discovery can link to those records; native indexing needs a
separately verified upstream adapter. This pilot can follow the launch if
publication or indexing is unavailable and does not block the two read
operations.

A Brainstorm assistant's assertion-signing key is not automatically an
executable OpenAgents agent. Any later bridge requires authorized key custody,
an admitted host binding, declared operations, and actual presence. V1 reads
public information and imports no assistant secrets.

## How Brainstorm uses Nostr

Brainstorm ingests signed public events and projects them into a social graph.
The Python service coordinates accounts and jobs; the Java worker computes
GrapeRank; Neo4j holds graph state; Redis carries queues and relationship
caches; Postgres holds application records; Vespa serves search. The strfry
fork commits events to LMDB and forwards selected kinds into Redis. The Kotlin
Vespa relay supplies a separate retrieval path.

GrapeRank weights an endorsement by its author's influence, edge confidence,
and attenuation. Follows usually increase influence; mutes and reports usually
suppress it. Scores depend on the observer and provider policy, and on the
events that the service has ingested.

| Contract | Brainstorm's use | Integration consequence |
| --- | --- | --- |
| NIP-01 kinds `0`, `3`, `10000`, and `1984` | Profiles, follows, mutes, and user-level reports supply public inputs. | Keep identity, social signals, and calculated reputation distinct. |
| NIP-85 kind `10040` | An observer designates a provider and relay per metric. Brainstorm uses a dedicated assertion-signing key per observer; NIP-85 requires distinct keys for distinct algorithms. | Personal score consumers verify the observer's signed designation. HTTP `/setup/{observer}` is service configuration, not that signature. |
| NIP-85 kind `30382` | Providers sign pubkey scores with integer `rank` from 0 to 100 and provider-defined counts. | Verify signer, subject, timestamp, and replacement identity before presenting a signed assertion. |
| NIP-50 over NIP-01 | The full search relay returns signed events in ranked order and accepts an explicit `observer:` lens. | Search order conveys ranking; ordinary `EVENT` frames carry no numeric score. |
| NIP-42 | Authentication can supply the reader's observer; an explicit public observer also permits public search. | Public reads need no identity signing. Preserve authentication-required failures. |

The full relay at `wss://search.brainstorm.world/` is Kotlin
`vespa-relay`. The Python server's profile-only `/relay` is a different
endpoint. The UI searches notes, articles, repository announcements, specs,
and other kinds through the full relay. Author, date, kind, and tag
restrictions belong in native NIP-01 filters; do not assume every UI token
belongs verbatim in a relay's search string.

The Open Ranking HTTP API exposes the same estate with different score units:

| Value | Meaning |
| --- | --- |
| `/search/pubkeys` `rank` under `relevance` | Search relevance, with no 0-to-100 interpretation. |
| `/rank/pubkeys` `rank` under `graperank` | Raw continuous GrapeRank influence, approximately 0 to 1. |
| Kind-`30382` `rank` | A signed, quantized integer from 0 to 100. |

HTTP stats contain raw relationship counts; Brainstorm's signed assertions
contain trusted-rater counts. Keep them separate. An HTTP score response is
an HTTPS observation and carries no Nostr score-provider signature.

## Fit with Coder's plugin system

The [composition proposal](../coder-new/plugin-architecture-carry-forward.md)
separates host-owned Rust providers from installed extensions. Brainstorm fits
that direction, with these current boundaries:

| Surface | Implemented today | Work for Brainstorm |
| --- | --- | --- |
| `coder-new` plugin manager | Five compiled definitions with settings and enabled flags. | Add a sixth, disabled definition and settings form. |
| OpenRouter chat | Registers enabled native functions and dispatches typed arguments. | Register both reads against the shared adapter. |
| Local Microcoder chat | Uses Codex/Claude login and a structured command loop; it does not consume the native function registry. | Add explicit host lookups and retain their observations in conversation context. |
| NIP-EXT distribution | The companion CLI publishes and installs signed releases; installation leaves packages off. | Publish supported guidance and metadata describing the required host binding. |
| Wasm extensions | `pure` and `snapshot-read`, with resource limits and no network imports. | Keep networking in trusted Rust host code. |
| NIP-REG and general PRG invocation | Curated REG discovery is Designed; the existing Coder runtime refuses general `Invoke` steps. | Use the narrower implemented EXT path; do not depend on these wider features. |

Adding a `ToolBinding` alone would make this an OpenRouter-only integration.
The explicit lookup path is part of the release scope. Enablement controls
Brainstorm's supported operations and guidance; it does not make Microcoder's
existing general shell an offline sandbox.

### Map to OpenAgents NIPs

- **NIP-EXT:** OpenAgents publishes the integration under its own identity,
  with an immutable release, file digests, and listing. NosFabrica supplies the
  external service; source access does not authorize publication on its behalf.
  Downloaded packages cannot install native executables.
- **NIP-CAP:** Describe stable host operations, input/output schemas, network
  recipients, limits, cancellation, and observation evidence. The compiled
  binding executes V1 calls. Portable descriptors grant nothing and do not
  make general PRG invocation work.
- **NIP-PRG:** Later workflows can compose supported host reads and bounded
  snapshot processing. This integration does not require Wasm network imports.
- **NIP-POL and NIP-RUN:** Local enablement and admitted recipients govern
  calls. Retain provenance locally using existing recording mechanisms; do not
  publish query history as public RUN events or claim unfinished portable
  journal support.
- **NIP-CJ and NIP-HOST:** Social scores grant no execution, host control,
  publisher admission, or spending. NosFabrica's relay does not advertise an
  OpenAgents CJ worker. Send it neither CJ requests nor private task context.

No new event kinds or OpenAgents NIP are needed for this slice.

## V1 user experience

1. Open **Plugins**, select **Brainstorm**, and enable it. Explain that requested
   queries and public keys go to `api.brainstorm.world`. Opening the screen,
   browsing local definitions, and enablement perform no service read.
   **Test connection** makes an explicit public discovery request.
2. Show **Brainstorm house perspective**. V1 supports this perspective only;
   an existing personal observer is a follow-up setting. Provider origins come
   from host configuration, outside model arguments.
3. Support `/brainstorm search Rust` and
   `/brainstorm rank <npub-or-hex>`. Extend the current slash parser's
   exact-word grammar deliberately. Dispatch on the background worker, show
   the normal running/result row, and support Escape cancellation.
4. Retain the bounded observation in live conversation context. “Compare these
   accounts” then works with local Codex/Claude and OpenRouter. Explicit
   lookups also work without a model provider.
5. On OpenRouter, expose the same reads as native functions. The host admits
   exact public lookup inputs before dispatch: an explicit slash command
   admits its text, while a model-proposed query first needs confirmation of
   the outbound text and recipient. Reuse a host-held reference to that
   admitted input. A rank call may use admitted public keys or keys returned
   by an admitted search. Typed arguments and model guidance alone cannot
   establish public-only provenance.

Example tasks are “Find Nostr profiles mentioning Rust” and “Show Brainstorm's
reputation for this publisher key.” V1 search returns public keys and profile
links, not names or biographies; fetching metadata is additional work.
Profile links provide navigation; the API observation supplies score evidence.
A publisher lookup uses the exact signed publisher key, not a display-name
match.

Disabling removes functions and commands from supported dispatch and cancels
outstanding reads. Preserve settings. Demo mode uses fixtures.

## Adapter contract

Put transport and normalization in a new Rust crate, provisionally
`crates/brainstorm-client`; keep presentation and settings in
`crates/coder-new`. Reimplement the public contract here rather than importing
their Python, Kotlin, Java, or TypeScript products.

| Proposed operation | Public request | Evidence |
| --- | --- | --- |
| `brainstorm.search_people` | `POST /search/pubkeys`, with `query`, `algorithm: "relevance"`, and `limit`; then one rank batch. | Pubkey, separate relevance and raw influence, source, and profile link. |
| `brainstorm.rank` | `POST /rank/pubkeys`, with bounded `pubkeys` and `algorithm: "graperank"`. | Pubkey and raw influence, preserving coverage uncertainty. |

The API base is `https://api.brainstorm.world`. Verify advertised operations
through `/.well-known/open-ranking.json`; the deployed route is
`/search/pubkeys`, not `/search/profiles`. Resolve the house identity through
`/.well-known/nostr.json?name=_` for provenance and record its discovery time.
Do not hardcode today's house pubkey. These reads identify the deployment's
configured perspective, not signed user delegation.

Initial host bounds: at most 512 Unicode characters and 1 KiB per query,
10 search results, 20 rank subjects, two concurrent calls, a 15-second total deadline, 256 KiB per response, and 64 KiB
normalized output. Project at most 8 KiB into model context and account for
other messages within the local route's 56 KiB total budget. These are proposed
engineering limits, not measured service guarantees. Snapshot origins and
bounds per operation, and recheck current enablement immediately before
network dispatch. Cancel dispatched reads on disable. Limit reads before
allocating the response. Allow only the configured HTTPS origin and
refuse cross-origin redirects.

Return requested algorithm, separately discovered house identity and discovery
time, origin, fetched time, expiry, input/output digests, and completeness.
The API does not echo its effective observer. Separate discovery cannot
atomically bind that identity to a response; label the capture observational.
Join search enrichment by pubkey and preserve search relevance order, because
the rank batch has its own influence ordering. Retain provenance per response
and expire the combined result at the earliest component expiry. A failed rank
batch leaves scores unavailable; it must not manufacture zeroes. Bound caching
by the service's `ttl` and a local maximum. Retain task observations instead of a global query-history
store. A refetch is a new observation.

Unknown subjects can receive `0.0`. Preserve the value and label coverage
unknown: the API does not distinguish an absent score from a computed zero.
Reject malformed/non-finite scores, unrequested subjects, and duplicate results.

Surface timeout, cancellation, unavailable discovery, `401`, `429`, and
service errors as typed states. Honor `Retry-After` without an unbounded loop;
V1 can invite an explicit retry. Preserve `202` computing and `422`
unavailable perspective if encountered. Add no authentication fallback:
Brainstorm's optional HTTP auth uses kind-`27519` Nostr Web Tokens, a
separate contract from NIP-42 and NIP-98.

## Delivery order for October 7

1. **Client and fixtures:** implement discovery, the two reads, normalization,
   bounded responses, cancellation, and mock cases. This can run in parallel
   with plugin settings and command UI.
2. **Plugin and explicit commands:** add the disabled definition,
   backward-compatible settings, connection test, slash dispatch, result rows,
   and conversation-context retention. Add a dedicated background
   `Work::Brainstorm` and completion update; do not use the model reply's
   completion path, which also changes provider state. Reuse the private
   atomic store for settings.
3. **OpenRouter dispatch:** add typed bindings using the same client and
   enabled/configuration snapshot. Do not add a second HTTP implementation or
   route local-model reads through a generic shell wrapper.
4. **Distribution and evidence:** bundle the native binding with the host.
   Prepare a self-contained EXT guidance package through the supported CLI
   path, with pinned files. Publish it as a launch companion when its existing
   pack/install checks pass; it does not block the native integration cut.
   The assembler emits program and skill-guidance components, not capability
   components. Other descriptors can be signed files without executable
   registration. State the required native host version in guidance as
   descriptive metadata: existing `compatibility.coder` checks the older
   `coder` core's version, not `coder-new`, and cannot enforce this requirement.
   Installing the wrapper does not enable native functions. Use the
   [Coder release process](../release/terminal.md) and
   `scripts/release/coder.sh` for artifacts.

The release cut is two lookup operations, explicit provider-independent
commands, and OpenRouter dispatch. Full relay search, general PRG execution, REG, and
personalization must not block that cut. If core acceptance fails, keep the
integration unavailable rather than exposing placeholder success.

### Acceptance before shipping

- Opening, enabling, disabling, or installing guidance causes no lookup.
  Disabled native operations refuse before dispatch, including calls using a
  tool-registration snapshot from an already-running turn.
- Settings survive restart and old files load. Demo performs no reads.
- Mocked search keeps relevance and influence separate, preserves search
  ordering after rank enrichment, and handles enrichment failure. Rank
  preserves zero/unknown coverage without inventing a signed assertion.
- Invalid arguments, excessive bytes, wrong subjects, service errors,
  deadlines, cancellation, and expiry have bounded, visible outcomes.
- Explicit commands work without OpenRouter or a model key, and their
  observations enter the context used by the local-model route. OpenRouter
  native calls produce equivalent fixture observations.
- External content remains evidence. It cannot change origins, invoke another
  plugin, install code, or approve effects. A fixture with model-supplied
  private file text causes no request without exact disclosure admission.
  Automatic request construction includes no workspace files or conversation.
- Results preserve source, perspective attribution limits, algorithm, units,
  and time through rendering and subsequent turns in the current live
  conversation. Test that a following user message and the bounded observation
  both reach the local generator. Chat reopening/restart recovery is deferred;
  current chat entries are in memory. Unsupported hosts show unavailable.
- Run a short public read-only smoke on release day and record deployed
  capabilities/responses. Use scratch state and no persisted owner chat.

For implementation, run `cargo test -p brainstorm-client -p coder-new` and
`cargo fmt` with the pinned toolchain and a long-lived external target
directory. Add other crate tests only when those crates change. Follow the
release process separately; add no GitHub-billed automation. Documentation-only
updates need link/path checks; implementation changes need focused crate tests.

## Follow-up operations and plugins

1. **Full content search in Brainstorm:** add a WebSocket adapter for the full
   relay, with typed scopes for profiles, notes, articles, repository
   announcements (`30617`), and specs (`30817`). Every filter carries the
   selected public observer. Send `CLOSE` after `EOSE` or cancellation;
   verify IDs/signatures and preserve ordering. Keep unknown perspective and
   partial capture explicit. Test that query extensions cannot override the
   host's observer or trust floor.
2. **Signed scores and existing personal perspectives:** fetch the observer's
   signed kind-`10040`, resolve its provider, then fetch the provider's
   kind-`30382` subjects. Existing NIP-85 helpers are useful, but the
   provider-list parser rejects Brainstorm's `muters`, `reporters`, `hops`,
   and bare `30392` extensions. Add bounded compatibility projection after
   shared event verification, or a narrowly tested shared improvement. Do not
   silently loosen validation, fabricate a narrowed signed event, or use
   `/setup` instead of signed designation. Provisioning and publishing
   `10040` require separate user action later.
3. **Brainstorm publisher:** consider a separate write-enabled plugin for
   opt-in public agent/profile listings, capability links, and updates once
   the pilot proves visibility. Reuse authorized identities; keep publication
   separate from installation, enablement, and social-score lookup.
4. **Nostr Observer:** offer a separately enabled digest plugin after search.
   Begin with user-requested summaries of retained public evidence. Admit
   model spending, scheduled collection, Blossom upload, and publication
   separately. Upstream still needs operator setup for new readers and
   generates editions on demand.
5. **Tapestry lists and tags:** consider later curation once draft contracts
   and host operations are pinned. Social scores remain advisory discovery
   evidence; registry signatures, exact releases, revocations, and operator
   admission stay independent.

## Reviewed sources

All 11 public NosFabrica repositories are local reference clones. The
following links pin the reviewed revisions; the UI uses upstream's
`staging` branch.

| Repository and revision | Assessment scope |
| --- | --- |
| [Brainstorm-UI @ 2ad1a659](https://github.com/NosFabrica/Brainstorm-UI/tree/2ad1a6595ab0df1a9656e93a7429e3ceb1ace919) | Search routing, trust sources, and activation. |
| [brainstorm_server @ dc8c4f34](https://github.com/NosFabrica/brainstorm_server/tree/dc8c4f34bd617bdf226956173a36ecc5a07399fc) | HTTP contracts, auth, queues, and publication. |
| [brainstorm_graperank_algorithm @ 5e51ecb7](https://github.com/NosFabrica/brainstorm_graperank_algorithm/tree/5e51ecb7b69e70c279f6fcc08fb9c59fa386b424) | Observer-relative calculation and provider pinning. |
| [brainstorm_one_click_deployment @ ec6f282e](https://github.com/NosFabrica/brainstorm_one_click_deployment/tree/ec6f282e0ff6a53c42006c3e3735a74f7b7b79d5) | Service boundaries and deployment footprint. |
| [brainstorm_integration_tests @ 97d07d4d](https://github.com/NosFabrica/brainstorm_integration_tests/tree/97d07d4d3d138bb0a9292ccc483b8998a6e775e2) | Bulk calculation/reporting tooling. |
| [strfry @ c144046a](https://github.com/NosFabrica/strfry/tree/c144046aed145c75486fdb5bfba7d8b69b14ed9e) | Event storage and Redis forwarding. |
| [vespa-eventstore @ 3e29812b](https://github.com/NosFabrica/vespa-eventstore/tree/3e29812bae568bff7eea6c0a656f5e57c1b47357) | Trust tensors and query grammar. |
| [vespa-relay @ 42028dcd](https://github.com/NosFabrica/vespa-relay/tree/42028dcd5a12147db5910cd697d9c331be3c9a5a) | NIP-50 serving and observer admission. |
| [the-nostr-observer @ 49c7dc94](https://github.com/NosFabrica/the-nostr-observer/tree/49c7dc94e4d7485ce3b5cd2add4f1421463d96c0) | Newspaper generation and Blossom publication. |
| [protocols @ bc41b92f](https://github.com/NosFabrica/protocols/tree/bc41b92f4d8e34d0eccbc20a9ef20e0cc66bdb1e) | GrapeRank, Trusted Assertions, and draft protocols. |
| [brainstorm_og @ ec70af64](https://github.com/NosFabrica/brainstorm_og/tree/ec70af647f338dc15b1b76a7a534cc107f6b87cd) | Share cards and link previews. |

Primary implementation anchors:

- Coder's [definitions](../../crates/coder-new/src/plugin_definition.rs),
  [dispatch](../../crates/coder-new/src/plugin_tools.rs),
  [local-model branch](../../crates/coder-new/src/live.rs),
  [slash grammar](../../crates/coder-new/src/slash.rs), and
  [private store](../../crates/coder-new/src/plugin_store.rs).
- Existing [EXT registry](../../crates/openagents-cli/src/plugin_registry.rs),
  [installation](../../crates/openagents-cli/src/plugin_local.rs),
  [Wasm host](../../crates/plugin/src/engine.rs), and
  [NIP-85 parser](../../crates/nostr/src/domain/assertion.rs).
- [NIP-EXT](../../nips/openagents/NIP-EXT.md),
  [NIP-CAP](../../nips/openagents/NIP-CAP.md),
  [NIP-PRG](../../nips/openagents/NIP-PRG.md),
  [NIP-POL](../../nips/openagents/NIP-POL.md),
  [NIP-RUN](../../nips/openagents/NIP-RUN.md), and
  [NIP-REG](../../nips/openagents/NIP-REG.md).
- [Live capabilities](https://api.brainstorm.world/.well-known/open-ranking.json),
  [runtime endpoints](https://brainstorm.world/config.js), and
  [house identity](https://api.brainstorm.world/.well-known/nostr.json?name=_).
- [HTTP schemas](https://github.com/NosFabrica/brainstorm_server/blob/dc8c4f34bd617bdf226956173a36ecc5a07399fc/app/routers/open_ranking/schemas.py),
  [rank handler](https://github.com/NosFabrica/brainstorm_server/blob/dc8c4f34bd617bdf226956173a36ecc5a07399fc/app/routers/open_ranking/rank.py),
  [trust-source resolution](https://github.com/NosFabrica/Brainstorm-UI/blob/2ad1a6595ab0df1a9656e93a7429e3ceb1ace919/client/src/services/trustSource.ts),
  and [relay lens policy](https://github.com/NosFabrica/vespa-relay/blob/42028dcd5a12147db5910cd697d9c331be3c9a5a/relay/src/main/kotlin/com/nosfabrica/vespa/relay/server/LensRequiredPolicy.kt).

Live capabilities describe the specific deployment; source describes the
reviewed revision. Some explanatory docs lag code: the GrapeRank draft's
eight-hop candidate limit differs from the current worker's unbounded reachable
query, and designated provider keys are pinned at influence `0.95`. Consume
published results in V1 rather than promising to reproduce the ranking locally.
