# CUT-26 RC5 installed-artifact closure

- Date: 2026-07-12
- Issue: [#8706](https://github.com/OpenAgentsInc/openagents/issues/8706)
- Result: closed
- Release: `0.1.0-rc.5`
- DMG bytes: `234540944`
- DMG SHA-256: `cf17f5d987f26f4fda732e48fd86e662b3c9a54ac5d0f39d189a18b0753e8f2b`
- Apple notarization submission: `663afd64-ad95-422d-ad38-8976ed124f51` (`Accepted`)

The accepted subject was the exact public DMG, downloaded from the production
release URL and copied from its mounted volume into a clean Applications
directory. Code-signature, staple, notarization, bundle version, and artifact
digest checks passed before application smoke.

The installed application passed the complete packaged smoke twice: once after
the clean copy and again after explicit removal and reinstall. The smoke covered
the hardened shell and Runtime Gateway, workspace/search/save conflicts,
durable editor reload, commands and history, diagnostics export notice and
preferences, Codex and Fable streams, structured question and metadata, Fleet,
Git review, PTY, image attachment, catalog reload, and clean teardown. A normal
launch used the durable `openagents-app://renderer/index.html` origin and showed
the real 1,625-session Codex/Claude history with Codex, GPT-5.6, and Medium
selected.

Update recovery was exercised against public bytes. A download was interrupted
at 9,195,520 bytes, resumed through HTTP range continuation, and finished with
the published byte length and digest. Production feed traffic was then rolled
back from RC5 revision `oa-updates-00107-rob` to retained RC2 revision
`oa-updates-00105-qiw`; the exact Desktop monotonicity seam rejected RC5 to RC2
as `not_strictly_newer`. Traffic was restored to RC5 and its signed live
manifest reverified. This proves recoverable service promotion while preserving
the unconditional client downgrade refusal.

Named isolated Codex accounts `codex-2`, `codex-4`, and `codex-5` were proven
ready through the Pylon inventory/refresh boundary. The intentionally detached
normal installed launch reported `pylon_runtime_unavailable` in Fleet instead
of inventing account evidence; release acceptance therefore keeps runtime
availability distinct from packaged-client integrity.

The production feed uses signing kid `2dbe811d19f67528`. Mobile OTA remained
HTTP 200 through promotion and rollback, while every legacy Desktop feed
continued to return the typed 410 lockout. Focused release verification passed
38 tests with 216 assertions, Desktop typecheck passed, and all eight release
preflight gates passed.

The issue's final public receipt is recorded in
[#8706's closing comment](https://github.com/OpenAgentsInc/openagents/issues/8706#issuecomment-4952597319).
