# Login flow: integration

The email-code login for the shared app (GitHub #13645): enter the email,
send a code, enter the code, log in. It is Coax with no effects. Its auth
requests go through the shared app core's outbox (`../core/`), and their
completions come back as later actions.

## Files

| File | What it is |
| --- | --- |
| `flow.coax` | `LoginFlow`: the steps, refusals, resend, cancel, unknown outcomes, owner generations, the kept intent, the draft across a relaunch |
| `service.coax` | `LoginRequest` and `LoginResult`: the flow's requests (`LS_SESSION`, `LS_START`, `LS_VERIFY`, `LS_LOGOUT`) and results |
| `card.coax` | `lf_card`: the card on the G0 frame context, built from the form set (`components/kit/form.coax`) |
| `bridge.coax` | `lfc_step`: the flow over the core. This is the one file that reads the core's names |

The shared rules (what an email and a code must be, and their words) come
from `apps/shared/app-login-flow.coax` and `apps/shared/app-login.coax`.

## What to load

The root loads `bridge.coax` once. It loads `card.coax`, `flow.coax`,
`service.coax` and `../core/core.coax`. A root that loads the core itself
loads `card.coax` instead and `bridge.coax` after the core.

The prefix is `lf_` and `LF_` (`lfc_` in the bridge), so the flow loads beside
the core (`LOGIN_` steps) and the login screen in `apps/shared/login/screen.coax`
(`login_` host calls).

## What to call

```coax
b: [1] LoginBridge; l: [1] LoginFlow;
lf_begin(l[..]);
_ := lf_restore(l[..], kept);              // the bytes the host kept at the last save, if any
app_host_launch(c, mono, wall, rejoin);
lfc_step(b[..], l[..], c);

// after every app_dispatch or app_host_* call, and after every lf_ call below
lfc_step(b[..], l[..], c);

// each frame, in the root's frame (between uiframe_begin and uiframe_end);
// put lf_focus_id(l) into the frame's UiInput as the focused id
h := lf_card(f, l[..], x, y, w, editing);

// a press on a committed row (a tap, VoiceOver, or the driver)
act := surface_action(surface, row);
if act != 0 && lf_press(l[..], act) { lfc_step(b[..], l[..], c); }

// the platform editor's whole value after each edit
_ := lf_set(l[..], LF_FIELD_EMAIL, text);    // or LF_FIELD_CODE
lf_blur(l[..]);                              // the keyboard went away
_ := lf_submit(l[..]);                       // Return in the editor

// a "Log in" button somewhere else
lf_open(l[..]);
```

`lf_card` draws nothing and returns 0 while the card is closed. It opens by
itself when a Send while logged out answers `APP_LOGIN`: the bridge reads the
core's `login_wanted`. The composer's draft stays in the core and is never
touched by the login.

To read the account: `lf_logged_in`, `lf_known` (false until the host
answers), `lf_who` (the email that logged in, "" after a launch read),
`lf_step` (`LF_READING`, `LF_OUT`, `LF_EMAIL`, `LF_REQUESTING`, `LF_CODE`,
`LF_VERIFYING`, `LF_CHECKING`, `LF_IN`). To log out: `lf_logout`.

## Stable IDs

| ID | Row | Action |
| --- | --- | --- |
| `login` | region "Log in" | |
| `login.title` | heading: "Log in", or "Email code" | |
| `login.sent` | the email the code went to (code step) | |
| `login.email` | field "Email address" (email step) | `LF_ACT_EMAIL` |
| `login.send` | "Send code" (email step) | `LF_ACT_SEND` |
| `login.code` | field "Email code" (code step) | `LF_ACT_CODE` |
| `login.verify` | "Log in" (code step) | `LF_ACT_VERIFY` |
| `login.resend` | "Resend code" (code step) | `LF_ACT_RESEND` |
| `login.change` | "Change email" (code step) | `LF_ACT_CHANGE` |
| `login.cancel` | "Cancel" (both steps, also while busy) | `LF_ACT_CANCEL` |
| `login.note` | the words, a live row | |

## Services

The flow needs no service of its own beyond the core's `FX_AUTH_STATE`,
`FX_AUTH_EMAIL`, `FX_AUTH_VERIFY`, `FX_AUTH_CANCEL` and `FX_AUTH_LOGOUT`
(see `../README.md`). The host keeps one more thing: the draft.

- Save: when `lf_draft_rev(l)` changes, `n := lf_draft(l, buf)` (at most
  `LF_DRAFT_MAX` bytes). Keep `buf[0..<n]`. When `n` is 0, forget what you kept.
  The bytes are the typed email behind a version line. They hold no secret,
  so keep them in app preferences, not the Keychain.
- Restore: after `lf_begin` and before the first `lfc_step`, call
  `lf_restore(l, kept)`. The code and the code step do not survive a
  relaunch, because the email attempt does not.

The core has no storage service yet, so the host does the save and restore.

## Checks

```sh
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/login/bridge.coax --library --traversal 1000000000
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/login/card.coax
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/login/flow.coax
```
