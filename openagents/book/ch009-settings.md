# 9. Settings

> **Status: partly implemented.** The shared Coax identity settings state
> machine ([`apps/shared/app/settings/identity.coax`](../../apps/shared/app/settings/identity.coax))
> runs on `cpu` with its runnable book example
> [`openagents/examples/settings-identity.coax`](../examples/settings-identity.coax)
> and committed debug/release transcript
> [`openagents/examples/settings-identity.out`](../examples/settings-identity.out),
> exercising signed-in public metadata loading, explicit confirmation gates for
> key rotation and restore, refusal to confirm restore before `secret_ready` is
> set, public-only `FX_IDENTITY` effect emission, and logout state clearing.
> Remaining for native hosts: GPU settings page scroll driver, large-text
> XCUITest, and `openagents/examples/cases/settings.ops`.

## Premise

Chapter 8's chat page has everything you need to send work. This chapter
adds the one page you open from the sidebar's foot: Settings, with your
account, your limits and Log out, as a scrolling page of GPU components.

## Draft

Settings is a route, `settings(section)`, that you open from the account
foot of the sidebar. Its sections come first as rows: Account, Limits and
About. Each is a page of the kit's settings set, drawn on the GPU like the
chat page. Log out is a row on the Account page and runs chapter 5's logout.

Settings shows only what works. A switch changes something you can see; a
row with nothing behind it is not drawn. Every limit appears with its
number before you reach it. That is a product rule across the whole app.

This is the first page that scrolls as a page rather than as a transcript,
so it settles the scroll driver for every later page. Coax owns the offset
as keyed state, and the host drives it from the platform's scroll physics.
Settings is also where large text has to work: the rows lay out from the
text size iOS reports, and a row that can't fit wraps rather than clips.
Chapter 5's login card draws at a fixed scale today, and the same rule
applies to it.

What a page keeps is split by owner. Which section you last opened is this
device's preference. Your account's settings come from the service and are
the same on every device.

## What we build

A plan for what this chapter ships once the draft is approved:

- The `settings(section)` route, opened from the sidebar's account foot.
- Account, Limits and About sections from the kit's settings set.
- The page scroll driver and large-text layout.

It builds on what exists today:

- [`apps/shared/app/settings/identity.coax`](../../apps/shared/app/settings/identity.coax)
- [`components/kit/settings.coax`](../../components/kit/settings.coax)
- [`docs/app-shell.md`](../../docs/app-shell.md)
- [`components/gpu-components.md`](../../components/gpu-components.md#scrolling)
- [`components/core/scroll.coax`](../../components/core/scroll.coax)

### The chapter's check

- **Source:** [`apps/shared/app/settings/identity.coax`](../../apps/shared/app/settings/identity.coax),
  [`openagents/examples/settings-identity.coax`](../examples/settings-identity.coax),
  and the committed stdout transcript [`openagents/examples/settings-identity.out`](../examples/settings-identity.out)
  (remaining: `openagents/examples/cases/settings.ops` and native large-text XCUITest).
- **Trace:** after sign-in, `iks_sync` queues one read effect (`read_queued=true held=1`),
  parses the canonical auth-identity payload (`loaded=true source=2`), arms and
  cancels `IKS_ROTATE` cleanly, confirms `IKS_RESTORE` only once `secret_ready`
  is set with a public-only `FX_IDENTITY` body, and wipes all loaded metadata on
  logout (`after_logout_loaded=false pubkey_empty=true`).
- **Refusal:** calling `IKS_CONFIRM` for `IKS_RESTORE` before `secret_ready` is
  set is refused (`restore_refused_early=true`) with no effect queued; remaining
  for native hosts is redirecting logged-out `settings(account)` deep links to
  the login card.
- **Independent check:** `coax/bin/coax run openagents/examples/settings-identity.coax`
  and `--release` produce identical transcripts matching
  [`openagents/examples/settings-identity.out`](../examples/settings-identity.out),
  beside `coax/bin/coax test apps/shared/app/settings/identity.coax`.

Planned paths: `openagents/examples/cases/settings.ops`. The app is
[`apps/openagents-ios`](../../apps/openagents-ios).

## Sources

- **Also:**
  - [The app shell and settings](../../docs/app-shell.md)
  - [GPU components on native](../../components/gpu-components.md), stage G2
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Source listing

Run the Chapter 9 example in [`openagents/examples/settings-identity.coax`](../examples/settings-identity.coax) (committed transcript [`openagents/examples/settings-identity.out`](../examples/settings-identity.out)):

<!-- source: openagents/examples/settings-identity.coax settings-identity -->
```jai
si_run :: () -> bool {
  c: [1] AppCore;
  app_init(c[..]);
  s: [1] IdentitySettings;

  // Sign in and sync: Settings queues one public identity read effect.
  c[0].auth = AUTH_IN;
  iks_sync(s[..], c[..]);
  op := s[0].op;
  print("read_queued=% held=%\n", op > 0, fx_held(c[0].out[..]));

  // Deliver canonical auth-identity response through AppCore guest dispatch.
  _ := fx_take(c[0].out[..], op);
  b: [512] u8;
  n := aiw_write("fixture", AIW_G, 2, b[..]);
  gen := fx_generation(c[0].out[..], IKS_OWNER);
  _ = app_dispatch(c[..], AppEvent.{kind = EV_DONE, op = op, generation = gen, outcome = FXO_DONE, status = 200, text = cast(string) b[0..<n]});
  loaded := iks_guest(s[..], c[..], cast(string) b[0..<n]);
  print("loaded=% source=% pubkey_prefix=%\n", loaded && s[0].loaded, s[0].source, iks_pubkey(s[..])[0..<12]);

  // Confirmation gate: IKS_ROTATE requires explicit confirmation and can be cancelled cleanly.
  rot_arm := iks_request(s[..], c[..], IKS_ROTATE);
  rot_cancel := iks_request(s[..], c[..], IKS_CANCEL);
  print("rotate_armed=% cancel_ok=% confirm=%\n", rot_arm, rot_cancel, s[0].confirm);

  // Refusal: IKS_RESTORE cannot confirm until secret_ready is true; once ready it queues public-only FX_IDENTITY.
  _ = iks_request(s[..], c[..], IKS_RESTORE);
  refused_early := !iks_request(s[..], c[..], IKS_CONFIRM) && s[0].op == 0;
  s[0].secret_ready = true;
  confirmed := iks_request(s[..], c[..], IKS_CONFIRM) && s[0].op > 0;
  at := fx_find(c[0].out[..], s[0].op);
  pub_only := at >= 0 && c[0].out[0].slots[at].kind == FX_IDENTITY && fx_body(c[0].out[..], at) == AIW_G;
  print("restore_refused_early=% restore_confirmed=% public_only_effect=%\n", refused_early, confirmed, pub_only);

  // Sign out wipes loaded identity metadata immediately.
  c[0].auth = AUTH_OUT;
  iks_sync(s[..], c[..]);
  print("after_logout_loaded=% pubkey_empty=%\n", s[0].loaded, iks_pubkey(s[..]) == "");

  return loaded && refused_early && confirmed && pub_only && !s[0].loaded;
}
```

Output (`coax/bin/coax run openagents/examples/settings-identity.coax`, identical in `--release`):

```text
read_queued=true held=1
loaded=true source=2 pubkey_prefix=79be667ef9dc
rotate_armed=true cancel_ok=true confirm=0
restore_refused_early=true restore_confirmed=true public_only_effect=true
after_logout_loaded=false pubkey_empty=true
ok=true
```
