# NIP-17 inbox verification — in progress

Issue [#13629](https://github.com/OpenAgentsInc/openagents/issues/13629)
remains open. This work has **not** been pushed or deployed. The delivery
contract is [Nostr direct messages](../../protocol/nostr-direct-messages-v1.md).

The implementation uses one Coax messenger and conversation view in the
native iOS app and `/messages`. Account-managed custody stays behind the
existing installed services and API. Device-held custody, public feed reads,
agent chats and environment sessions keep their existing owners.

## Completed checks

These are component results, not a claim that the release gate is complete.
Logs below live in the isolated `oa-arcade-dm` worktree's ignored `build/`.

| Check | Result | Evidence |
| --- | --- | --- |
| Actual PostgreSQL/API journal, custody, delivery and private read-position routes | 56 checks passed under ASan | `dm-api-read-position-tests.log` |
| Read-position wire codec | 147 mutants: 145 killed, 2 reviewed equivalents, no survivors | `dm-read-wire-qualified.log` |
| Read-position projection | 234 mutants: 228 killed, 6 reviewed equivalents, no survivors | `dm-read-projection-qualified.log` |
| Delivery observation state | 152 mutants: 150 killed, 2 reviewed equivalents, no survivors | `dm-delivery-state-transition-qualified.log` |
| Opened inbox admission and bounded chronology | 210 mutants: 207 killed, 3 reviewed equivalents, no survivors | `dm-inbox-final-qualified.log` |
| Public-key display adapter | 66 mutants: 58 killed, 8 reviewed equivalents, no survivors | `dm-public-key-qualified.log` |
| Changed installed program-size and HTTP admission rules | 14 mutants killed, no survivors | `dm-native-metadata-policy.log` |
| Immutable outgoing intent, recovery and sender-copy recognition | 244 mutants: 225 killed, 19 reviewed equivalents, no survivors | `dm-outgoing-recovery-qualified.log` |
| Extracted unsigned-rumor adapter | 63 mutants killed, no survivors | `dm-rumor-initial-qualified.log` |
| Extracted kind-10050 relay projection | 78 mutants killed, no survivors | `dm-receipt-baseline-and-relay-projection.log` |
| Actual native recipient picker/composer and accessible delivery glyph | Passed on the isolated signed simulator | `dm-native-final-ui-tests.log` |
| Shared DOM adapter, including complete digest IDs, focus and field values | 19 checks passed | `dm-full-id-modules.log` |
| Compiled native/web/reference exchange, native cold history-row selection, exact committed-send retry | Passed, signed iOS simulator and real relay/API | `dm-id128-live-tests.log`, `dm-id128-live-reference.log` |
| Browser cold recovery through the real history row | Passed; both texts recovered | `dm-id128-cold-result.log`, `dm-id128-cold-final.png` |
| Tools, wire registry and component layers | Passed | `dm-tools-latest.log`, `dm-wire-latest.log`, `dm-components-latest.log` |

Equivalent annotations name the specific redundant condition or unobservable
local cleanup they cover. They do not remove the production checks or cleanup.

## Live client work

The opt-in `NativeMessagingTests` integration case runs the normally compiled
Coax phone module through the installed native custody, journal, relay and HTTP
services. The browser uses the normally compiled `/messages` module. The
independent client uses `coax/tests/nostr/nip17_live_reference.py` and the
independent NIP-01/44/59/17 arithmetic in `nip17_reference.py`.

The fixture uses two disposable email accounts on a throwaway development API
and PostgreSQL database, plus a disposable reference key on
`wss://relay.openagents.com`. It never loads the owner's account, tokens, wallet
or identity backup. Development email codes stay in a private log; no email is
sent. The loopback proxy supplies disposable sessions to both SDK consumers.

The `--lose-seal-response` mode returns one 503 **after** the real server has
committed a seal, then checks that the retry's complete request is byte-identical.
It does not replace the custody, journal, cryptography or relay implementation.

The complete live case passed at 17:49 on November 15: native → web → native,
native cold recovery through the actual conversation row, then an independent
client's verified open and encrypted reply. Counts are exactly one body for
each message and exclude reply previews. The fixture confirmed both the lost
committed response and a byte-identical retry. A fresh browser also recovered
both messages by selecting the history row. The native test waits for the
actual recipient picker/title before interacting with it.

The fixture hit the development API's accumulated account-creation limit after
repeated interrupted test sessions. Only `auth.limit_counts` in the isolated
throwaway PostgreSQL database was reset before this final fixture run. No
production account, limit, key, inbox or service was changed.

## Defects corrected during verification

- Queued navigation now redraws the selected page after its current request
  drains; it cannot leave the previous private composer visible.
- A recoverable NIP-42 authentication closure no longer permanently marks an
  otherwise completed catchup as incomplete. Earlier genuine incompleteness
  remains retained.
- Native HTTP admission explicitly includes the two registered private metadata
  endpoints; custody and journal effects remain on their installed services.
- A send pressed beside an already queued SDK action retains its frozen intent
  until native entropy can be requested.
- An explicit uncertain-send retry can wait behind a relay effect without
  replacing that effect's request or generating another intent.
- Same-owner settings refreshes preserve current SDK requests and resume a
  paused messenger. They cannot silently change its owner.
- Shared DOM slot IDs now retain up to 128 bytes. The previous 64-byte bound
  truncated complete digest-based conversation IDs while drawing the full ID,
  so a visible history row could not dispatch its own selection. The fix also
  preserves complete field/focus IDs and prevents suffix collisions.

## Remaining release work

1. Finish initial mutation qualification of the remaining new coordinator,
   relay, journal, outgoing, receipt-codec/cache and extracted primitive units.
   Retained survivor/timeout reports are not successful qualification.
2. Rebase onto current main, retain its concurrent app/world work, and run the
   final touched checks and existing agent/account/environment regressions.
3. Deploy the API and web page. Generate the complete native release from the
   final source tree, with its current world resources and runtime fingerprint;
   the older world binaries copied for simulator tests are not release assets.
4. Push main and upload a new TestFlight build. Do not wait for Apple processing
   before continuing the next issue.
