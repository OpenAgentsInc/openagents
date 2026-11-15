# Durable Nostr messaging foundation

Issue #13628 extends the feed's installed relay carrier with account-scoped
custody (operation 21), journal (22) and authenticated relay sessions (23).
DM screens remain the next Arcade stage; this change installs their shared
capabilities without changing the current chat/project/world UI.

A seal prepares every peer's NIP-59 envelope and the sender's copy. All actual
signed kind-10050 heads are checked before encryption. Exact ciphertexts,
frozen relay targets and the first operation receipt commit in one account
transaction before a success or external EVENT. A lost response replays the
first batch. Linked-device plaintext and private/ephemeral keys do not travel
to the account server; account-managed identities use the server provider.
Private opening is never cached. Kind-15 file opening is refused until the
attachment SDK can retain decryption keys outside generic UI payloads.

## Qualification

- NCJ2 original core: 575 mutants, 574 killed, zero survivors, one explained
  equivalent. Owned-event/reference extension: 321 mutants, 320 killed, one
  equivalent; exact-original tightening: 85/85. Completed ACKs freeing the
  active queue and checkpoint recovery: 501 mutants, 497 killed, zero
  survivors, four explained equivalents (`build/journal-final2-merged.log`).
- Store policy/outbox: 85/85; incoming arrival-time gate: 18/18. Larger receive
  frames preserve the old public-read bound: 31/31 changed queue mutants.
- Shared list editor: 112 mutants, 111 killed, zero survivors, one equivalent.
  Unknown tags and opaque content survive a real signed same-second edit.
- Shared peer helpers: 69/69. The final NIP-17 plan change and extracted peer
  owner: 128/128. Relaxing relay-only retention/quota checks for the client's
  verified history uses the shared structural reader: 29/29 changed mutants.
- Authenticated socket send policy: 63 mutants, 60 killed, zero survivors,
  three explained equivalents. Session-state shards qualify actual-generation
  ownership, AUTH, read/EOSE/CLOSED barriers and retirement; receipts are retained
  in `build/relay-session-expanded2-1.rec` and `build/relay-session-expanded3-2.rec`.
- Actual HTTP/PostgreSQL: **41 checks passed** in 376 seconds
  (`build/messaging-complete-api-linked3.log`). Includes actual managed-key encryption,
  complete two-copy staging, exact receipt retry, wrong-account opening refusal,
  failed receipt insertion rolling back the entire batch, injected journal
  failure, independent account isolation, exact outbox originals/URLs, stale
  revision refusal, fresh-proof wipe and atomic receipt deletion, and corrupt
  committed-row refusal. Fixtures are disposable accounts.
- Native simulator SDK: **five tests passed** after complete-send integration
  (`build/messaging-complete-native1.log`). Actual held-key encryption/opening,
  no plaintext in linked HTTP bodies, SDK recreation and lost-response recovery,
  source/identity mismatch refusal, old-owner HTTP cancellation and post-open
  revocation refusal. A live relay test used two fresh AUTH challenges at
  `wss://relay.openagents.com`; the old socket could no longer authorize and the
  public read operation could not read the authenticated socket.
- Earlier unchanged native regressions: 21 AppAuth tests passed, including
  logout/new-login racing an old refresh response; 12 installed messaging/feed/
  account tests passed, including public feeds over real relays. Original
  NCJ1 bytes and the independent signing fixture remain byte-for-byte unchanged.
- Shared complete-send check opens both peer and sender copies to the same
  verified rumor ID, refuses a changed inbox head before appending, mounts the
  two pending originals after staging and recovers the exact sender bytes.

The SDK recreation test models process retirement against retained server
state. It is not an observed physical-phone force-quit test. Public read tests
used UIKit lifecycle notifications, not a hardware Home/lock cycle. Tests
published no Nostr EVENT. Live AUTH used only a publicly known test scalar,
never an account or wallet key. Old runtime feeds are retained.

## Delivery

- Final custody policy: 105 mutants, 103 killed, zero survivors, two equivalents.
  Session state: 173 mutants, 168 killed, zero survivors, five equivalents.
  Complete-send admission: 49 mutants, 47 killed, zero survivors, two equivalents.
  Receipt verification: 29 mutants, 26 killed, zero survivors, three equivalents.
  SDK provider/result/outbox ownership: 101 mutants, 99 killed, zero survivors,
  two equivalents. File opening refuses the actual encrypted kind-15 test.
- Linked HTTP qualification uses an actual independently held test scalar and
  signed inbox heads. All recipient and sender copies roll back when receipt
  insertion fails; a retry commits the whole set and recovers both originals.
  Its fixture explicitly models the strong login required once a key is linked.
- Build **1093** is **VALID / IN_BETA_TESTING** in **Owner**. Upload accepted,
  delivery `be9302e6-81f5-4d89-a116-d9fefe7ac414`; 21,054,971-byte IPA.
  Frozen archive, Generated SDK, twenty OTA resources and signed envelope:
  `~/.openagents/releases/openagents-ios-1093/`.
- Runtime `83086eb3ea6fd10a0c1673e84c230a28fcee93de12300f24cfb2fb0f54220704`.
  Generation 1 is published without changing older feeds. Both public update
  origins were compared byte-for-byte with the archive resources.
- Backend compilation is a frozen remote snapshot (`c6ed53d109`) of source
  `94937b4489`; its only extra file is the untracked build ignore marker.
  The cloud SDK deploy uses the Mac's existing credentials. Staging migration
  and revision `oa-api-staging-00232-xb7` passed. Production migration and revision `oa-api-00169-568` passed.
  Both environments run the same immutable image
  `sha256:e3f951b3ab7b717c5c0621a6f83cc2e09500885bb0cbf4c8e4d2bec3e039011c`.

After this issue's deploy, the owner's requested **Microduck Home viewer**
interrupts the Arcade sequence. Keep #13629 and subsequent stages paused.
