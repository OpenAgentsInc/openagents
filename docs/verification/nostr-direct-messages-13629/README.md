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
| Actual native recipient picker/composer and accessible delivery glyph | Passed on the isolated signed simulator | `dm-native-final-ui-tests.log` |
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

Observed live stages include native → web, web → native, recovery of sent and
received history by a cold native client, and retry after the deliberately lost
seal response. An earlier independent-client run reached both reply directions,
but its final assertion incorrectly counted the reply preview as another
message. That oracle now counts only message-body nodes.

The current complete live case is **not yet green**: switching the cold native
client to the independent recipient exposed repeated installed relay refusals.
The test now waits for the actual recipient picker/title and records the SDK's
public refusal reason. This must be resolved and the full case rerun before
closing the issue. A Mac restart interrupted the subsequent diagnostic run;
the retained API database is separate from that Mac's process lifetime.

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

## Remaining release work

1. Resolve the independent-recipient transition and pass the complete live
   native/web/reference case, including cold web recovery and exact retry.
2. Finish initial mutation qualification of the remaining new coordinator,
   relay, journal, outgoing, receipt-codec/cache and extracted primitive units.
   Retained survivor/timeout reports are not successful qualification.
3. Rebase onto current main, retain its concurrent app/world work, and run the
   final touched checks and existing agent/account/environment regressions.
4. Deploy the API and web page. Generate the complete native release from the
   final source tree, with its current world resources and runtime fingerprint;
   the older world binaries copied for simulator tests are not release assets.
5. Push main and upload a new TestFlight build. Do not wait for Apple processing
   before continuing the next issue.
