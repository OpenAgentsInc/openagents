# CLI realm edit boundaries — 2026-11-02

Independent loopback observations for GitHub #12635. The harness runs the
built CLI and serves controlled HTTP replies. It loads no CLI or API product
implementation. The shared fixture contributes HTTP framing helpers; all
replies and acceptance observations are authored in
[`cli-edit-boundaries.coax`](../../../coax/web/cli-edit-boundaries.coax).

The harness uses one fresh in-memory test key, passed only through the child
environment and erased afterward. It creates no real realms, makes no
external requests, and does not qualify server authentication or deployment.

## Observations

| Case | Observed result |
| --- | --- |
| Move already at the requested position, concurrent writer wins after GET | CLI still sends one save; server 409 becomes exit 5 with revision 8. |
| Spec exceeds the 262,144-byte CLI buffer, with another move after that boundary | Exit 1 with complete error JSON; no POST. |
| Write returns malformed JSON | Exit 1 with complete error JSON. |
| Write returns 200 without a revision | Exit 1 with complete error JSON. |
| Valid document contains a raw object larger than the 65,536-byte report | Exit 1 with complete error JSON. |
| Yaw `0001` | Posted JSON number is exactly `1`; negative X and positive Z are preserved. |
| Yaw `-000` | Posted JSON number is exactly `0`. |
| Published document has `base: null` | One POST to the legacy revisioned `/publish` route. |
| Published document has `base: "everglade"` | One POST to `/edit-v1/publish`. |
| Built-in fork publish returns 409 | Exit 5 with revision 8 and status 409. |
| Publication document omits `base` | Exit 1 before any POST. |
| Publication document has numeric `base` | Exit 1 before any POST. |

The first seven observations belong to CLI image
`5f71e30df03c98aea260b43ea8721644b5e959489cba1c3d3122dd48dcc148cc`.
The five publication-profile observations belong to final CLI image
`6c9a73ef744e21f10f4cb01f119fe0f09579392e5e98fa7535850e6636d685c0`.
Each receipt confirms that its CLI image stayed unchanged during that run.
Passing transport observations were not repeated after the publication-profile
work. These are separate producer observations, not one claimed full-image run.

## Retained evidence

- [Transport/output receipt](transport-and-output.json) and [raw log](transport-and-output.out).
- [Publication-profile receipt](publish-profiles.json) and [raw log](publish-profiles.out).
- [Harness checks](harness-checks.out): 110 checks passed, including five new
  oracle checks and the loaded libraries' checks.
- [Manifest](manifest.json): every retained member's byte count and SHA-256,
  with successful readback, plus the compiler and harness executable hashes.
- [Dependency inventory](harness-dependencies.json) and `source-0.coax.txt`
  through `source-8.coax.txt`: exact Coax sources captured by the manifest.
- `retainer.coax.txt`: the Coax copier that refused overwrites and read back
  every member. `harness-first-refusal.out` preserves an earlier unsupported
  global-variable declaration in the harness; it is not a CLI product result.

Harness source SHA-256:
`941b3a004f517328d156492e3abf035913ce3c883c91d91cdb3cefac9b5cfa65`.
Harness executable SHA-256:
`94610ec302979c047e3db9d64c9b317c61f5fb1782a0420556ff9999134478e5`.
Compiler executable SHA-256:
`69713521d6dd0540d6ff669048719d9b847955edcaa557da1eef06184d31e3b6`.

Each child has a 25-second bound, each connection a five-second bound, and the
whole harness a ten-minute deadline. All 12 observations finished normally.
