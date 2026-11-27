# 54. Releases as signed events

> **Status: partly implemented.** The `#authority` Nostr kind `30212` weight-module
> registry policy ([`psionic/lib/peer/module-registry.coax`](../../psionic/lib/peer/module-registry.coax)),
> canonical RFC 8785 JCS `ExwModule` codec ([`psionic/lib/executor-wire.coax`](../../psionic/lib/executor-wire.coax)),
> Class-E compiler ([`psionic/lib/module-v2-wave2.coax`](../../psionic/lib/module-v2-wave2.coax)),
> and live relay CLI commands (`openagents module publish`, `search`, `install` in
> [`apps/openagents-cli/module.coax`](../../apps/openagents-cli/module.coax)) run
> today and are grounded on `cpu` by
> [`openagents/examples/signed-module-registry.coax`](../examples/signed-module-registry.coax)
> and [`openagents/examples/signed-module-registry.out`](../examples/signed-module-registry.out).
> Remaining: general NIP-EXT plugin listings in `openagents/registry/` and the
> in-app registry browsing page.

## Premise

Chapter 53 sets the terms. This chapter builds the registry that meets the
first of them: a release is a signed event that anyone can publish, on our
relay or any other.

## Draft

A registry is usually a server that someone owns. You ask it to accept your
package, and it can say no. Ours is a set of signed records.

A release is one event. It names a package, lists its components with the hash
of each, and is signed by the author's key. A release never changes. A fix is
a new release. A listing is a second, replaceable record that says what the
package is and which release is current.

Because a release is a signed event, where it is stored does not matter. We
run a relay that keeps them. Anyone else can run one. A registry reader
gathers releases from the relays it is told about and checks every signature
and hash itself. If our relay went away, every release would still be valid
wherever a copy exists.

Publishing needs a key and nothing else. There is no review before a release
appears. Review happens after, in public, as the evidence of chapters 43 to 52. A release
with no evidence is listed as a candidate, and the agent treats it as one.

We have done this before in small. [Episode 49](../../docs/transcripts/049.md)
sets up a plugin registry, and [Episode 66](../../docs/transcripts/066.md)
tests a community contribution that puts plugin discovery on Nostr. The idea
waited for the rest of the system to catch up.

The same registry holds every kind of member. Chapter 73 adds realms and assets
to it.

## What we build

A plan for what this chapter ships once the draft is approved:

- Release and listing events, with a publish command that signs and sends
  them.
- A registry reader that gathers from several relays and verifies every
  signature and hash.
- A registry page on each surface that shows a package, its releases and its
  evidence.

It builds on what exists today:

- [`psionic/lib/peer/module-registry.coax`](../../psionic/lib/peer/module-registry.coax) (`#authority` kind `30212` module publish, search & relay install policy)
- [`psionic/lib/executor-wire.coax`](../../psionic/lib/executor-wire.coax) (`openagents.executor.module.v1alpha1` canonical JCS codec)
- [`apps/openagents-cli/module.coax`](../../apps/openagents-cli/module.coax) (`openagents module publish` / `search` / `install` over `CliRelay`)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/relay.coax`](../../coax/lib/nostr/relay.coax)
- [`nostr/relay/relay_server.coax`](../../nostr/relay/relay_server.coax)
- [`proto/openagents/executor/v1alpha1/release.proto`](../../proto/openagents/executor/v1alpha1/release.proto), [`psionic/lib/peer/executor-release.coax`](../../psionic/lib/peer/executor-release.coax), and [`psionic/lib/peer/executor-release-sign.coax`](../../psionic/lib/peer/executor-release-sign.coax) (signed `ExecutorReleaseManifest` events pinning promoted models and Class-E compiled weight modules)
- [`coax/lib/nostr/event_sign.coax`](../../coax/lib/nostr/event_sign.coax)
- [`apps/openagents-cli/relay.coax`](../../apps/openagents-cli/relay.coax)
- [`apps/openagents-cli/skill.coax`](../../apps/openagents-cli/skill.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Release, listing and revocation | `openagents.extension.v1` `Release`, `Listing`, `Revocation` ([spec 20.2](../../docs/protocol/openagents-protocol-spec.md#202-openagentsextensionv1)) | Nostr kinds 3184, 30184 and 3185, public (NIP-EXT) | Spec sketch; no `.proto` file. `coax/lib/nostr/ext` handles the NIP's JSON |
| Signed Class-E module manifest | `openagents.executor.module.v1alpha1` (`ExwModule`, [`executor-wire.coax`](../../psionic/lib/executor-wire.coax)) | Nostr kind 30212 addressable event (`d` = `weight_sha256`) | Live in [`module-registry.coax`](../../psionic/lib/peer/module-registry.coax) and `openagents module publish`/`search`/`install` |

### The chapter's check

- **Source:** [`psionic/lib/peer/module-registry.coax`](../../psionic/lib/peer/module-registry.coax),
  [`apps/openagents-cli/module.coax`](../../apps/openagents-cli/module.coax),
  [`openagents/examples/signed-module-registry.coax`](../examples/signed-module-registry.coax),
  and the committed stdout transcript [`openagents/examples/signed-module-registry.out`](../examples/signed-module-registry.out)
  (remaining: general NIP-EXT `openagents/registry/` multi-relay reader).
- **Trace:** compiles `oa-exec-patch-rebase` (`14592` bytes, SHA-256 prefix
  `685c0985c26cc61b`), writes canonical `openagents.executor.module.v1alpha1`
  JCS + kind `30212` NIP-01 tags, verifies round-trip manifest read (`read=ok`),
  relay install admission (`install_admit=ok`), and substring search match
  (`search_match=true`).
- **Refusal:** wrong Nostr kind (`30210`) refuses with `bad_kind`; mismatched
  `d`-tag refuses with `bad_d_tag`; 1-byte tampered digest refuses with
  `digest_mismatch`; self-review (`author_key == reviewer_key`) refuses with
  `self_review`.
- **Independent check:** `coax/bin/coax run openagents/examples/signed-module-registry.coax`
  and `--release` match [`openagents/examples/signed-module-registry.out`](../examples/signed-module-registry.out),
  `coax/bin/coax test psionic/lib/peer/module-registry.coax` (`120/120`, `136/136`
  mutants killed), and live `openagents module publish`/`search`/`install` E2E
  against `nostr/relay/bin/relay` (#13960).

Shipped paths: [`openagents/registry/registry.coax`](../../openagents/registry/registry.coax) (`#14060`, NIP-EXT 3184/30184, NIP-REG catalog admission, NIP-EVAL 3189 quorum).

Planned paths: `openagents/registry/sync-daemon.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:00, the registry
  - [Episode 49, *Plugin Registry Setup*](../../docs/transcripts/049.md): a plugin registry
  - [Episode 55, *Plugin Registry UI*](../../docs/transcripts/055.md): a registry UI
  - [Episode 66, *Nostr Plugin Registry*](../../docs/transcripts/066.md): plugin discovery on Nostr
  - [Episode 288, *Three DevDays Later*](../../docs/transcripts/288.md): point your agents at the NIPs
- **Old code at `coder-v1.0.0`, crates:** `plugin`, `discovery`, `nostr-relay`, `nostr-transport`
- **NIPs:** [NIP-REG](../../nostr/nips/openagents/NIP-REG.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-SKILL](../../nostr/nips/openagents/NIP-SKILL.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P10, P12

## Open questions for the owner

1. Do we mirror releases from any relay by default, or only from a list we
   choose? An open mirror needs a plan for spam.

## Source listing

Run the Chapter 54 example in [`openagents/examples/signed-module-registry.coax`](../examples/signed-module-registry.coax) (committed transcript [`openagents/examples/signed-module-registry.out`](../examples/signed-module-registry.out)):

<!-- source: openagents/examples/signed-module-registry.coax signed-module-registry -->
```jai
smr_run :: () -> bool {
  // 1. Compile real Class-E module oa-exec-patch-rebase and serialize to safetensors with SHA-256.
  la: [MV_MAX_LAYERS] EvLayerDesc; ha: [MV_MAX_HEADS] EvHeadDesc; wa: [65536] f64;
  m_pr := mv_compile_patch_rebase(la[..], ha[..], wa[..]);
  sb: [65536] u8;
  ser := ev_serialize(m_pr, sb[..]);
  sha_b: [64] u8;
  sha := smr_hex32(ser.digest, sha_b[..]);
  print("compiled=oa-exec-patch-rebase bytes=% sha256=%\n", ser.written, sha[0..<16]);

  // 2. Encode canonical openagents.executor.module.v1alpha1 JCS manifest + kind 30212 NIP-01 tags.
  ch_b: [64] u8; cs_b: [64] u8;
  _ := exw_canonical_sha256("psionic-module-v2-exec-vm-v1", ch_b[..]);
  _ = exw_canonical_sha256("{\"passed\":24,\"total\":24,\"exact_replay\":true}", cs_b[..]);
  exw := [1] ExwModule.[ExwModule.{
    compiler_hash = cast(string) ch_b[..],
    conformance_suite_sha256 = cast(string) cs_b[..],
    input_abi = ExwAbi.{max_value = 67108864, min_value = -67108864, scalar_type = "i64", slot_count = 7},
    max_steps = m_pr.n_layers,
    memory_bytes = ser.written,
    module_id = "oa-exec-patch-rebase",
    output_abi = ExwAbi.{max_value = 67108864, min_value = -67108864, scalar_type = "i64", slot_count = 3},
    profile_id = "exec-vm-v1",
    n_seams = 0,
    verification_class = ExwVerifClass.e_exact_replay,
    version = "0.2.0",
    weight_sha256 = sha
  }];

  jbuf: [2048] u8; tbuf: [512] u8;
  jw := exw_module_write(exw[..], jbuf[..]);
  tn := mr_build_tags(exw[..], tbuf[..]);
  content := cast(string) jbuf[0..<jw.len];
  tags := cast(string) tbuf[0..<tn];

  // 3. Verify kind 30212 event + relay install admission + search match.
  author_pk := "6cd81f74c59cf72c8cf9933f0f0565f27d2acbc34996955a3b6bff2288b16d3c";
  parsed: [1] ExwModule; arena: [2048] u8;
  rd := mr_read_manifest(MR_KIND_MODULE, author_pk, tags, content, parsed[..], arena[..]);
  adm := mr_admit_relay_install(parsed[..], sha, ser.written, 24, 24, true);
  q_hit := mr_query_matches(parsed[0].module_id, parsed[0].weight_sha256, "patch-rebase");
  print("read=% install_admit=% search_match=%\n", mr_status_name(rd), mr_status_name(adm), q_hit);

  // 4. Fail-closed refusals: wrong kind, mismatched d-tag, 1-byte tampered digest, self-review.
  bad_kind := mr_read_manifest(30210, author_pk, tags, content, parsed[..], arena[..]);
  bad_d := mr_read_manifest(MR_KIND_MODULE, author_pk, "[[\"d\",\"0000000000000000000000000000000000000000000000000000000000000000\"]]", content, parsed[..], arena[..]);
  bad_sha := "0000000000000000000000000000000000000000000000000000000000000000";
  tamper := mr_admit_relay_install(exw[..], bad_sha, ser.written, 24, 24, true);
  self_rev := ma_admit_submission(MaSubmission.{
    sig_ok = true, author_key = 101, reviewer_key = 101, digest_match = true,
    weight_bytes = ser.written, label = MvLabel.exact, conformance_passed = 24, conformance_total = 24,
    exact_replay_ok = true, tamper_refused_ok = true, heldout_lift_bps = 400, noise_2sigma_bps = 150, retained_drop_bps = 0
  });
  print("refuse_kind=% refuse_d=% refuse_tamper=% refuse_self_review=%\n",
    mr_status_name(bad_kind), mr_status_name(bad_d), mr_status_name(tamper), ma_status_name(self_rev));

  return rd == MrStatus.ok && adm == MrStatus.ok && q_hit &&
         bad_kind == MrStatus.bad_kind && bad_d == MrStatus.bad_d_tag &&
         tamper == MrStatus.digest_mismatch && self_rev == MaStatus.self_review;
}
```

Output (`coax/bin/coax run openagents/examples/signed-module-registry.coax`, identical in `--release`):

```text
compiled=oa-exec-patch-rebase bytes=14592 sha256=685c0985c26cc61b
read=ok install_admit=ok search_match=true
refuse_kind=bad_kind refuse_d=bad_d_tag refuse_tamper=digest_mismatch refuse_self_review=self_review
ok=true
```
