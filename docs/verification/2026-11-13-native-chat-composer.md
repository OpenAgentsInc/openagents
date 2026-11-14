# Native chat composer, 1071

The implementation through `f1b11e55f5` adapts the regular phone composer
from T3 Code `f313d0a6cb`; [the contract](../../coax/native/composer.md)
records measurements, provenance and scope. It uses the shared kit composer,
UITextView, native glass and the existing account/Updates services.

## Checks

* `apps/openagents-ios/home/app.coax`: 221 checks passed, including save
  before inference, draft retention on failure, explicit retry, reply save
  and account expiry. The account wire dependency's earlier 102 checks passed.
* `account-chat-policy.coax`: 28 root mutants killed, zero survivors or
  equivalents; exact person ownership, team funding and revision/size bounds.
* `components/layout/native-composer.coax`: compact, wrapped and capped
  reference geometry check passed.
* Native XCTest verified a retained editor/selection, 378-point surface at
  402-point viewport width, 48-point compact surface, 160-point expanded
  surface and scrolling beyond four lines. Native auth fields and malformed
  command refusal passed in the earlier run; only changed checks were rerun.
* The final `ChatUITests` passed email code → new chat → send → model reply →
  saved list → relaunch → reopen → multiline input, keyboard docking and
  pinch stability. The keyboard remains present after the answer. These use
  an independent fake mailbox/model and real native Keychain and transport.
* The focused API run passed under ASan/UBSan: 16 history/account chat checks,
  including email-only login, ownership, compare-and-swap, team-only funding
  and the actual inference adapter with a fake upstream. Its supporting
  CLI/auth checks (72) and anime API checks (77) also passed. Migration
  snapshot matches all 199 migrations; the route answer check passed.
* The portable app and Extras artifacts matched real WebAssembly command
  frames and actions. The oracle includes save → inference → reply save.
* Wire-check, layer-check, components-check, tools-check and parity passed.
  The pre-push checks passed in 19 seconds, including touched Coax suites.

The fixture first attached a strong key to test legacy history, then tried
to write using its limited email session. The correct step-up refusal was
preserved. The corrected fixture removes only its own fake key before the
separate email-only write checks. No production gate was weakened.

## Production backend

The staging and production deployments use the same immutable image:
`sha256:f07b7b296129ca0d4ca0b074bf59f728bf8ca668d6d80f51ebd55aaca1712d80`,
built from `1e2009a2957f`. Routed health and Cloud Run traffic identity
verification passed for `oa-api-staging-00218-g6f` and
`oa-api-00155-hl5`, each serving 100% of its service's traffic.

A live probe using the existing admin CLI identity exercised the new
account endpoints: create 200, real Gemini completion 200 with the expected
marker, reply save 200, exact transcript reopen, and membership in the
account history list. The newly created verification record was then
removed (exactly one row). This did not use the owner's email session or
change the owner's linked keys/chats. The owner's email account was checked
read-only and is a person with the team funding flag. The probe's sanitized
receipt is retained beside the archive.

## Delivery

Native build **1071**, marketing version 1.0.0, bundle `com.openagents.app`.
Runtime:
`b0fab8971ad78461f2ab23f9312a513c10e298995130b0b392e83e20d95bc6ff`.
Apple accepted the uploaded archive and reports VALID / IN_BETA_TESTING.
The exact build is verified in the existing **Owner** group, with 1071
release notes. The archive was produced from `21a23ea402`; later changes
only extend verification records/oracles.

Signed OTA **generation 1** contains exactly the tested native-profile app
and Extras resources. Both `updates.openagents.com` and the pinned Cloud Run
fallback served the exact retained signed envelope; all five object hashes
and bytes match. This package targets 1071's runtime, not 1070.

The archive, IPA, runtime, signed package, manifest and native screenshots
are retained in `~/.openagents/releases/openagents-ios-1071/`. No credentials
or wallet secrets appear in those verification observations. Owner phone
acceptance remains in `NEEDS_OWNER.md`.

The current profile is text-only, fixed Gemini 4 Hydrogen and team-funded.
The regular phone layout is source-measured; no running T3 screenshot was
available for a zero-difference image claim. T3's large-control/iPad profile,
attachments and custom dictation are not claimed. Legacy key-owned full
conversation viewing remains on the web. The new account table participates
in account export, merge and purge.
