# 76. Releases from one source

> **Status: partly implemented.** The `#authority` single-source release-set
> ordering, build-pin, staging-lock, and canary-regression policy
> ([`coax/web/release-set-policy.coax`](../../coax/web/release-set-policy.coax)),
> `#authority` Desktop v1 SemVer/platform release policy
> ([`coax/lib/agent/desktop/release-v1-policy.coax`](../../coax/lib/agent/desktop/release-v1-policy.coax)),
> and Ed25519 update verifier ([`coax/lib/agent/desktop/update.coax`](../../coax/lib/agent/desktop/update.coax))
> run on `cpu`, grounded by
> [`openagents/examples/release-set-gating.coax`](../examples/release-set-gating.coax)
> and [`openagents/examples/release-set-gating.out`](../examples/release-set-gating.out),
> and are consumed by [`coax/web/release-set.coax`](../../coax/web/release-set.coax),
> [`coax/web/release.sh`](../../coax/web/release.sh), and
> [`apps/openagents-desktop/release-v1.coax`](../../apps/openagents-desktop/release-v1.coax).
> Remaining: cross-host reproducible web-build byte comparison on a second host,
> public store-driven `/download` generation, and signed `openagents.attest.v1`
> Nostr release attestations. Note: on 2026-11-15 the owner dropped over-the-air
> updates ([#13645](https://github.com/OpenAgentsInc/openagents/issues/13645));
> native apps ship strictly through signed releases and store builds.

## Premise

Every client passes the conformance run of chapter 75. This chapter ships
them: one source and one provenance record for every release, published to
each platform on that platform's own schedule, with a way back on every one.

## Draft

Episode 289 ends with a rollout: release candidates, a few days of finding
bugs, then stable versions on every platform, from openagents.com/download.
The audit records how that went. Version 1.0 shipped for Coder, and it did
not ship for the whole OpenAgents app on every platform.

The fix is not one pipeline that waits for everything. It is one source and
one record of where each build came from. Every release starts from one
commit. Each platform's build records that commit, the compiler that built
it and the checks it passed. One version number means the same source
everywhere. It does not mean the same day everywhere.

Platforms ship when they are ready. The website ships when its checks pass.
The iPhone app ships when App Store review approves it, which can take days,
and Android follows Google Play's own review. The download page lists a
build when its store has it, not when it was uploaded. Windows and other
additional platforms ship on their own evidence (chapter 77) and never hold
up the web or the phone.

An installed app is older than the server. People update late, and some
never do. The API answers every app version still in use (chapter 4), and a
release that changes a record adds a version beside the old one.

No change reaches an installed app without a store release. The native
apps run compiled Coax linked into the binary, with no JavaScript runtime
and no over-the-air Updates, so every change to the iPhone app, code and
content alike, ships in a TestFlight or store build.

Every release has a way back. On the web, the previous build is one step
away. On a phone, a store can't take back an installed app, so the way back
is a fix released forward. A release can be stopped only by a
regression it causes, so each check compares against what is live now.

Release candidates stay part of the process. [Episodes
254](../../docs/transcripts/254.md) and [256](../../docs/transcripts/256.md)
show the pattern we keep: ship a candidate, use it ourselves, find the
failures, fix them, and promote the build that survived.

## What we build

A plan for what this chapter ships once the draft is approved:

- A provenance record per platform build: commit, compiler, checks and the
  target evidence of chapter 75.
- Per-platform release steps that publish independently, and a download page
  generated from what each store actually lists.
- The version rule between installed apps and the API.
- A way back for each platform: the previous web build, and a fix forward
  for a store app.

It builds on what exists today:

- [`coax/remote`](../../coax/remote)
- [`coax/web/release.sh`](../../coax/web/release.sh)
- [`apps/openagents-ios/release.sh`](../../apps/openagents-ios/release.sh)
- [`coax/web`](../../coax/web)
- [`apps/openagents-web`](../../apps/openagents-web)
- [`apps/openagents-desktop`](../../apps/openagents-desktop), [`apps/openagents-desktop/Window+Update.swift`](../../apps/openagents-desktop/Window+Update.swift), and [`apps/openagents-desktop/windows/build.sh`](../../apps/openagents-desktop/windows/build.sh)
- [`apps/openagents-ios`](../../apps/openagents-ios)
- [`apps/shared/host/android`](../../apps/shared/host/android)
- [`os`](../../os)

### The chapter's check

- **Source:** [`coax/web/release-set-policy.coax`](../../coax/web/release-set-policy.coax),
  [`coax/lib/agent/desktop/release-v1-policy.coax`](../../coax/lib/agent/desktop/release-v1-policy.coax),
  [`coax/lib/agent/desktop/update.coax`](../../coax/lib/agent/desktop/update.coax),
  [`openagents/examples/release-set-gating.coax`](../examples/release-set-gating.coax),
  and [`openagents/examples/release-set-gating.out`](../examples/release-set-gating.out)
  (remaining: second-host reproducible web build byte comparison).
- **Trace:** enforces ordered progression (`api,rooms,page,true`) and reverse
  rollback (`page,rooms,api`), verifies single-commit provenance (`one_set=true`),
  build-pin/wire compatibility (`pins_ok=true`), pinned staging promotion
  (`promote_ok=true`), healthy canary pass (`canary_pass=true`), and Desktop v1
  Windows platform/SemVer admission (`win_plat=windows`, `win_ext=.zip`,
  `semver_ok=true`).
- **Refusal:** promoting a `-seeded` bad build fails with `RS_P_SEEDED`; moved
  staging fails with `RS_P_MOVED`; mismatched engine pin or partial shard wire
  coverage refuses page shipping; another holder's live staging lock refuses
  (`rs_lock_may=false`); a regressing canary fails with
  `crashed,slower-tick,fewer-seats`; and `0.0.0` fails `rv1_version_valid`.
- **Independent check:** `coax/bin/coax run openagents/examples/release-set-gating.coax`
  and `--release` match [`openagents/examples/release-set-gating.out`](../examples/release-set-gating.out),
  alongside `#authority` mutation checks in [`coax/web/release-set-policy.coax`](../../coax/web/release-set-policy.coax)
  and [`coax/lib/agent/desktop/release-v1-policy.coax`](../../coax/lib/agent/desktop/release-v1-policy.coax).

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Release status | `openagents.deploy.v1` `DeployStatus`, `LaneHead` ([spec 28.3](../../docs/protocol/openagents-protocol-spec.md#283-openagentsdeployv1)) | Nostr kinds 3205 and 30205 | Spec sketch; no `.proto` file |
| A signed build others can rebuild | `openagents.attest.v1` `Release`, `Rebuild` ([spec 27.2](../../docs/protocol/openagents-protocol-spec.md#272-openagentsattestv1)) | Nostr kinds 3202 and 3203 (NIP-ATT) | Spec sketch; no `.proto` file |

Planned paths: none. Releases extend [`coax/remote/`](../../coax/remote) and
each app's own release script.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, release candidates and the download page
  - [Episode 254, *Bug Bash*](../../docs/transcripts/254.md): a bug bash
  - [Episode 256, *Release Candidate*](../../docs/transcripts/256.md): a release candidate
  - [Episode 233, *The Monorepo*](../../docs/transcripts/233.md): one repository
  - [Episode 253, *Goodbye Bun*](../../docs/transcripts/253.md): changing the runtime under a product
- **Old code at `coder-v1.0.0`, crates:** `openagents-desktop`, `openagents-mobile`, `github-actions`, `mac-jobs`
- **NIPs:** [NIP-DEPLOY](../../nostr/nips/openagents/NIP-DEPLOY.md)
- **Also:**
  - The audit's section 5, "What else is needed besides the book", and its step 9
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P16

## Open questions for the owner

1. Which stores do we publish to at 1.0, and which platforms get a direct
   download only?
2. Which kinds of update may install on their own, and which ask the person
   first?

## Source listing

Run the Chapter 76 example in [`openagents/examples/release-set-gating.coax`](../examples/release-set-gating.coax) (committed transcript [`openagents/examples/release-set-gating.out`](../examples/release-set-gating.out)):

<!-- source: openagents/examples/release-set-gating.coax release-set-gating -->
```jai
rsg_run :: () -> bool {
  // 1. Ordered release progression (api -> rooms -> page -> done) and reverse rollback.
  s0 := rs_next(0); s1 := rs_next(1); s2 := rs_next(3); s3 := rs_next(7);
  b0 := rs_back(7); b1 := rs_back(3); b2 := rs_back(1);
  one_set := rs_one_set(true, true, true, false);
  own := RsPins.{engine = 4, rules = 9, content = 3, wire_lo = 6, wire_hi = 6};
  shard := RsPins.{engine = 4, rules = 9, content = 3, wire_lo = 5, wire_hi = 7};
  pins_ok := rs_pins_match(own, shard) && rs_pins_ship(3, 3) && rs_wire_ship(3, 3, 6, 5);
  prom_ok := rs_promote("01bdb405f0-20261126", "01bdb405f0-20261126");
  print("order=%,%,%,% rollback=%,%,% one_set=% pins_ok=% promote_ok=%\n",
    rs_part(s0), rs_part(s1), rs_part(s2), s3 == RS_DONE,
    rs_part(b0), rs_part(b1), rs_part(b2), one_set, pins_ok, prom_ok == RS_P_OK);

  // 2. Canary comparison: healthy canary passes; shared background slowness does not stop release.
  others := RsShard.{tick_p95_us = 600, refused_ds = 4, seated = 32, unsaved_ms = 5000, windows = 30};
  canary_ok := RsShard.{tick_p95_us = 640, refused_ds = 5, seated = 32, unsaved_ms = 5200, windows = 30};
  c_pass := rs_canary(canary_ok, others);
  plat_win := rv1_parse_platform("windows");
  ver_ok := rv1_version_valid("1.0.0-rc.1");
  print("canary_pass=% win_plat=% win_ext=% semver_ok=%\n",
    c_pass == 0, rv1_platform_name(plat_win), rv1_archive_ext(plat_win), ver_ok);

  // 3. Fail-closed refusals: seeded build, moved staging, mismatched wire/pins, held staging lock,
  // canary crash + slow tick + fewer seats, and 0.0.0 SemVer version.
  ref_seeded := rs_promote("01bdb405f0-seeded", "01bdb405f0-seeded") == RS_P_SEEDED;
  ref_moved := rs_promote("01bdb405f0-1", "de2783f887-2") == RS_P_MOVED;
  ref_pins := !rs_pins_match(own, RsPins.{engine = 5, rules = 9, content = 3, wire_lo = 6, wire_hi = 6});
  ref_wire := !rs_wire_ship(2, 3, 6, 5);
  ref_lock := !rs_lock_may(rs_lock(true, "tok-a", "tok-b", 50, 100, true, true));
  bad_c := canary_ok; bad_c.crashes = 1; bad_c.tick_p95_us = 9000; bad_c.seated = 0;
  c_bits := rs_canary(bad_c, others);
  wbuf: [128] u8;
  c_words := rs_c_words(c_bits, wbuf[..]);
  ref_ver0 := !rv1_version_valid("0.0.0");
  print("refuse seeded=% moved=% pins=% wire=% lock=% canary=% ver0=%\n",
    ref_seeded, ref_moved, ref_pins, ref_wire, ref_lock, c_words, ref_ver0);

  return s0 == RS_API && s1 == RS_ROOMS && s2 == RS_PAGE && s3 == RS_DONE &&
         b0 == RS_PAGE && b1 == RS_ROOMS && b2 == RS_API && one_set && pins_ok &&
         prom_ok == RS_P_OK && c_pass == 0 && plat_win == RV1_PLAT_WINDOWS && ver_ok &&
         ref_seeded && ref_moved && ref_pins && ref_wire && ref_lock &&
         c_words == "crashed,slower-tick,fewer-seats" && ref_ver0;
}
```

Output (`coax/bin/coax run openagents/examples/release-set-gating.coax`, identical in `--release`):

```text
order=api,rooms,page,true rollback=page,rooms,api one_set=true pins_ok=true promote_ok=true
canary_pass=true win_plat=windows win_ext=.zip semver_ok=true
refuse seeded=true moved=true pins=true wire=true lock=true canary=crashed,slower-tick,fewer-seats ver0=true
ok=true
```
