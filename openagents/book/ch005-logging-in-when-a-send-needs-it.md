# 5. Logging in when a send needs it

> **Status: partly implemented.** The login gate flow
> ([`apps/shared/app/login/flow.coax`](../../apps/shared/app/login/flow.coax)),
> logout pending-turn fence ([`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax)),
> and runnable example [`openagents/examples/login-gate-flow.coax`](../examples/login-gate-flow.coax)
> pass on `cpu` in debug and release. The account service runs in `coax/web/api`,
> and the GPU login screen runs in the iOS Simulator
> ([#13645](https://github.com/OpenAgentsInc/openagents/issues/13645)).
> Remaining: physical-phone VoiceOver, one-time-code AutoFill, and IME checks.

## Premise

Chapter 4 sends a message, and the service answers only an account. This
chapter asks you to log in at the moment a send needs it: the login card
opens over the chat page, you log in with an email code, and your message
goes out. Tokens stay in the native auth adapter and never enter Coax.

## Draft

You can type before you log in. When you press Send with no session, the
app keeps your draft and opens the login card over the chat page. The card
owns input while it is open. When the login succeeds, the card closes,
focus returns to the composer, and the send you asked for goes out once.
When you close the card, nothing is sent and your draft stays.

The card is the GPU login screen, already built on the kit
([G1 as built](../../components/gpu-components.md#g1-as-built)):
[`apps/shared/login/screen.coax`](../../apps/shared/login/screen.coax) holds
its state, layout, paint and rows, email first and then the code, with the
form set in [`components/kit/form.coax`](../../components/kit/form.coax).
It records `login.email`, `login.send`, `login.code`, `login.verify`,
`login.resend` and `login.change`, and the same rows answer VoiceOver and
the driver. The words are "Log in". The code field takes one-time-code
AutoFill through the platform editor. In this chapter the card sits over
the chat page instead of covering the whole app.

The native auth adapter owns the session.
In [`AppAuth.swift`](../../apps/shared/host/apple/AppAuth.swift),
`emailStart` and `emailVerify` call the service, the tokens go to the
Keychain, and the app learns only whether you are logged in. No token, code
handle or pending request ID enters Coax, a trace or a log. A request the
app makes with your session asks the adapter to attach it.

Logging out ends everything the session started. A pending reply is
cancelled with chapter 4's rule, its late parts are dropped, and the
transcript and draft are cleared. A held reply can't appear in the next
person's screen.

The account behind the card is already built
([docs/auth](../../docs/auth/README.md)), and on 2026-11-26 the locked
**WorkOS hybrid `acct_` spine** (`#14097`–`#14103`,
[`docs/auth/2026-11-26-workos-coax-sdk.md`](../../docs/auth/2026-11-26-workos-coax-sdk.md))
extended it without giving up self-sovereign keys or wallets. An account is
still an opaque `acct_...` ID in Cloud SQL Postgres. WorkOS mediates human
login (Email Magic Auth codes, Google/GitHub/Apple OAuth via native PKCE and
`ASWebAuthenticationSession` / Custom Tabs, Enterprise SSO, and Optional
Organizations) behind our own **Custom Coax UI** (no Hosted AuthKit redirect),
while `oa-api` mints our own opaque sessions (`__Host-oa_session` on web,
device-bound `oa_at_`/`oa_rt_` with `OA-Device-Proof` on native). An email
address, a Nostr key and a first-party WebAuthn passkey attach to the same
`acct_...` in any combination, and a Nostr key alone is enough (D8). There are
no passwords, and we never collect a phone number (D3). Levels decide what a
session may do: every WorkOS login (email code, social OAuth, or SSO) enters
at `L1`; a first-party passkey, a Nostr key or a recovery code gives `L2`;
wallet backups, spend limits, key removal and agent grants still require an
`L2`/`L2-fresh` step-up from the last five minutes. The server's rules are
Coax in [`coax/web/api/auth-policy.coax`](../../coax/web/api/auth-policy.coax),
and the Sans-I/O `#authority` Coax WorkOS SDK lives in
[`coax/lib/workos/`](../../coax/lib/workos) with checked-in OpenAPI bindings
emitted by [`tools/oagen-coax/`](../../tools/oagen-coax).

An agent is an account too. It has an owner and its own way in: a Nostr key
that signs every request (NIP-98), or an `oa_key_` API key for tools that
can't sign. It never uses its owner's key, email or session. Its grant says
what it may do: scopes, sats a day, sats a payment, an expiry of 90 days by
default and a year at most, and the addresses it may call from. The owner
approves the grant at L2. An agent can never add or remove a way in, change
recovery, make or widen a grant, raise a cap, export a seed or delete an
account. The owner's key signs that the agent's key acts for it (NIP-OA).
Later chapters use this grant (chapters 17, 33 and 59); this chapter's app
shows none of it yet.

## What we build

Built in the repository today ([docs/auth](../../docs/auth/README.md)):

- Email-code and Nostr login, sessions and the audit log (#13200 to #13206).
- Agent accounts, keys, scopes and grants (#13211).
- Step-up, linking and the 72-hour recovery (#13212, #13213).
- Passkeys on the web and in the iPhone app (#13215), and device keys
  (#13216).
- Locked WorkOS hybrid `acct_` authentication architecture (`#14097`–`#14103`),
  in-repo Sans-I/O `#authority` Coax WorkOS SDK ([`coax/lib/workos/`](../../coax/lib/workos)),
  and checked-in OpenAPI codegen emitter ([`tools/oagen-coax/`](../../tools/oagen-coax)).
- The GPU login screen and its native side, checked in the iOS Simulator
  (#13645).

Planned:

- The login card over the chat page, opened by a send with no session, with
  the send held until the login ends.
- Logout, with the pending-turn fence.
- The card's physical-phone checks: VoiceOver, one-time-code AutoFill, an
  input method.

It builds on what exists today:

- [`apps/shared/login/screen.coax`](../../apps/shared/login/screen.coax)
- [`apps/shared/native/NativeLogin.swift`](../../apps/shared/native/NativeLogin.swift)
- [`apps/shared/host/apple/AppAuth.swift`](../../apps/shared/host/apple/AppAuth.swift)
- [`components/kit/form.coax`](../../components/kit/form.coax)
- [`docs/auth/identity-and-methods.md`](../../docs/auth/identity-and-methods.md)
- [`docs/auth/sessions-and-tokens.md`](../../docs/auth/sessions-and-tokens.md)
- [`coax/web/api/auth-policy.coax`](../../coax/web/api/auth-policy.coax)
- [`coax/web/api/agent-policy.coax`](../../coax/web/api/agent-policy.coax)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/web/api/nip98.coax`](../../coax/web/api/nip98.coax)
- [`coax/lib/nostr/nipoa.coax`](../../coax/lib/nostr/nipoa.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Grant request, grant and agent key | `openagents.account.v1alpha1` `GrantRequest`, `Grant`, `SpendCaps`, `AgentKey` ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, as binary protobuf (`application/x-protobuf`) | In use: one generated codec, `coax/web/api/agent-wire.coax` |
| The owner's attestation of an agent key | `openagents.account.v1alpha1` `OwnerAttestation`; on the network it is an `auth` tag (NIP-OA) | HTTP inside the grant request; Nostr on the agent's events | In use in the API. NIP-OA is an external format |
| A signed request | A NIP-98 event | HTTP `Authorization` header | Built. External format |
| Account, authenticators, sessions, audit log | `openagents.account.v1` and `openagents.credential.v1` in the spec ([spec 15.1](../../docs/protocol/openagents-protocol-spec.md#151-openagentsaccountv1), [spec 15.2](../../docs/protocol/openagents-protocol-spec.md#152-openagentscredentialv1)) | HTTP JSON | Hand-written JSON. The [protocol review](../../docs/protocol/2026-11-11-protocol-review.md) orders its rewrite as `v1alpha1` |

### The chapter's check

- **Source:** [`apps/shared/app/login/flow.coax`](../../apps/shared/app/login/flow.coax),
  [`apps/shared/app/login/service.coax`](../../apps/shared/app/login/service.coax),
  [`apps/shared/login/screen.coax`](../../apps/shared/login/screen.coax),
  [`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax),
  and [`openagents/examples/login-gate-flow.coax`](../examples/login-gate-flow.coax).
- **Trace:** committed debug/release stdout in [`openagents/examples/login-gate-flow.out`](../examples/login-gate-flow.out):
  `lf_need(INTENT_SEND)` while logged out returns `LF_ASKED` and opens the card;
  submitting `you@example.com` enters `LF_CODE`; verifying `123456` closes the card,
  sets `logged_in=true`, `who=y...@example.com`, and offers `offer=1` without auto-sending.
- **Refusal:** a wrong code (`000000`) leaves the card open with `That code isn't right.`;
  `lf_logout` advances `generation` (`1 -> 2`) so a late verify result returns `false`
  with `dropped=1`; logging out while a reply is pending drops subsequent `chat.part`
  chunks (`late part dropped`) and clears `state transcript` to `(empty)`; and
  `state account.token` is refused by the inspection allowlist.
- **Independent check:** `coax/bin/coax test apps/shared/app/login/flow.coax`,
  `coax/bin/coax test apps/shared/app/login/service.coax`,
  `coax/bin/coax test coax/web/api/auth-policy.coax --library`, plus XCUITest
  through the accessibility tree on native hosts (remaining for `ios-device`).

## Source listing & runnable example

Run [`openagents/examples/login-gate-flow.coax`](../examples/login-gate-flow.coax) (committed transcript [`openagents/examples/login-gate-flow.out`](../examples/login-gate-flow.out)):

<!-- source: openagents/examples/login-gate-flow.coax login-gate-and-logout-fence -->
```jai
main :: () {
  // Part 1: Login gate state machine (apps/shared/app/login/flow.coax).
  l: [1] LoginFlow;
  q: [1] LoginRequest;
  r: [1] LoginResult;
  lf_begin(l[..]);
  _ := lf_take(l[..], q[..]);
  r[0] = ls_result(q[0], LS_DONE, 200, false, "", "");
  _ = lf_result(l[..], r[..]);

  // Pressing Send while logged out asks for login and keeps the intent (composer draft untouched).
  gate := lf_need(l[..], INTENT_SEND, 0);
  print("send_logged_out gate=% card_open=% step=%\n", gate == LF_ASKED, l[0].open, lf_step(l[..]));

  // Email -> code requested.
  _ = lf_set(l[..], LF_FIELD_EMAIL, "you@example.com");
  _ = lf_press(l[..], LF_ACT_SEND);
  _ = lf_take(l[..], q[..]);
  r[0] = ls_result(q[0], LS_DONE, 200, false, "", "");
  _ = lf_result(l[..], r[..]);
  print("email_sent step=%\n", lf_step(l[..]));

  // Refusal 1: wrong code keeps card open with the service's words and clears the code field.
  _ = lf_set(l[..], LF_FIELD_CODE, "000000");
  _ = lf_press(l[..], LF_ACT_VERIFY);
  _ = lf_take(l[..], q[..]);
  r[0] = ls_result(q[0], LS_FAILED, 400, false, "That code isn't right.", "");
  _ = lf_result(l[..], r[..]);
  print("wrong_code open=% logged_in=% words=%\n", l[0].open, lf_logged_in(l[..]), lf_words(l[..]));

  // Valid code logs in, closes card, and offers the held send intent without auto-sending.
  _ = lf_set(l[..], LF_FIELD_CODE, "123456");
  _ = lf_press(l[..], LF_ACT_VERIFY);
  _ = lf_take(l[..], q[..]);
  saved_verify_q := q[0];
  r[0] = ls_result(q[0], LS_DONE, 200, true, "", "y...@example.com");
  _ = lf_result(l[..], r[..]);
  print("verified open=% logged_in=% who=% offer=% gen=%\n",
    l[0].open, lf_logged_in(l[..]), lf_who(l[..]), lf_offer(l[..]), lf_generation(l[..]));

  // Refusal 2: logout bumps generation and fences any late verify result from an older generation.
  ended := lf_logout(l[..]);
  r[0] = ls_result(saved_verify_q, LS_DONE, 200, true, "", "y...@example.com");
  late_ok := lf_result(l[..], r[..]);
  print("logout_fence ended=% logged_in=% gen=% late_ok=% dropped=%\n",
    ended, lf_logged_in(l[..]), lf_generation(l[..]), late_ok, l[0].dropped);

  // Part 2: Logout while a reply is pending drops late parts and clears the transcript.
  a: [1] OapApp;
  tr: [1] OapTrace;
  oap_play(a[..], OAP_T_LOGIN, tr[..]);
  _ = oap_drive(a[..], "tap shell.new");
  _ = oap_drive(a[..], "type composer.input hello");
  _ = oap_drive(a[..], "tap composer.send");
  _ = oap_drive(a[..], "clock data release chat.send");
  _ = oap_drive(a[..], "clock data release chat.part");
  print("logout_pending=%\n", oap_drive(a[..], "tap shell.logout"));
  print("after_logout_part=%\n", oap_drive(a[..], "clock data release chat.part"));
  print("after_logout_transcript=%\n", oap_drive(a[..], "state transcript"));
  print("token_guard=%\n", oap_drive(a[..], "state account.token"));
}
```

```shell
coax/bin/coax run openagents/examples/login-gate-flow.coax
```

Expected output:

```text
send_logged_out gate=true card_open=true step=1
email_sent step=4
wrong_code open=true logged_in=false words=That code isn't right.
verified open=false logged_in=true who=y...@example.com offer=1 gen=1
logout_fence ended=true logged_in=false gen=2 late_ok=false dropped=1
logout_pending=tapped shell.logout (home)
after_logout_part=released chat.part: late part dropped
after_logout_transcript=(empty)
token_guard=account.token is not inspectable; state takes route, account.step, account.words, account.who, account.waiting, composer.text, turn, transcript, service.messages, clipboard, clock
```

Planned paths: none new — CPU login gate and logout fence run in [`apps/shared/app/login/flow.coax`](../../apps/shared/app/login/flow.coax), [`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax), and [`openagents/examples/login-gate-flow.coax`](../examples/login-gate-flow.coax). Server rules stay in [`coax/web/api/`](../../coax/web/api).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): reputation, identity and payments figured out once
  - [Episode 82, *Nostr Login*](../../docs/transcripts/082.md)
  - [Episode 207, *Your Keys, Your Coins, Your Identity*](../../docs/transcripts/207.md): one seed for a Nostr key and a wallet key
- **NIPs:** [NIP-98](../../nostr/nips/official/98.md), [NIP-OA](../../nostr/nips/block/NIP-OA.md), [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md)
- **Also:**
  - [OpenAgents authentication](../../docs/auth/README.md) and its [decisions](../../docs/auth/decisions.md)
  - [Nostr From Scratch, chapter 87](../../nostr/book/ch087-owner-attestation-and-agent-authentication.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P14

## Open questions for the owner

1. Does the lean app's login card include "Use a Nostr key" in this
   chapter, or only email until the key login is rebuilt on the kit?
2. Do a member's grant (chapter 17) and an agent's grant become one record,
   or stay two?
3. Which scopes come after `profile:read` and `wallet:spend`?
