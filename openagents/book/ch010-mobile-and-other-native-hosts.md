# 10. Mobile and other native hosts

> **Status: partly implemented.** The unified desktop/iOS GPU host carrier
> ([`apps/openagents-ios/ios-app.coax`](../../apps/openagents-ios/ios-app.coax),
> [`apps/openagents-ios/gpu-page.coax`](../../apps/openagents-ios/gpu-page.coax),
> [`apps/openagents-ios/gpu-host.c`](../../apps/openagents-ios/gpu-host.c))
> and the `#authority` phone-to-host engine convergence policy
> ([`coax/lib/agent/environment/phone-agent-policy.coax`](../../coax/lib/agent/environment/phone-agent-policy.coax),
> `#14060`, `#14073`) run on `cpu`, grounded by
> [`openagents/examples/phone-host-agent-policy.coax`](../examples/phone-host-agent-policy.coax)
> and [`openagents/examples/phone-host-agent-policy.out`](../examples/phone-host-agent-policy.out),
> and are consumed by [`openagents/daemon/host.coax`](../../openagents/daemon/host.coax),
> [`openagents/daemon/host-phone.coax`](../../openagents/daemon/host-phone.coax),
> [`apps/shared/antigravity/client-core.coax`](../../apps/shared/antigravity/client-core.coax),
> and [`apps/openagents-ios/Sources/GpuRootViewController.swift`](../../apps/openagents-ios/Sources/GpuRootViewController.swift).
> Remaining: running chapter 6's `coax drive` acceptance case and native
> accessibility UI tests across `macos-arm64`, `ios-simulator`, `ios-device`,
> Linux Wayland, and `android-emulator` (`#14068`).

## Premise

Chapters 1 to 9 built the chat page, history, chips and settings grounded
in the primary desktop app (`apps/openagents-desktop`) on shared Coax logic
that iOS already consumes. This chapter completes full native host parity
across iOS, Linux desktop, and Android. It adds no feature. It qualifies
every native host against the same acceptance case, and it states the rule
that keeps them one app.

## Draft

[Episode 289](../../docs/transcripts/289.md) says it plainly: go to
openagents.com, or open the terminal, desktop or mobile app, and you are
talking to the same thing. That is promise P1. It is a rule about code as
much as about product.

We have broken it before. The archive holds a web app, a phone app, a
desktop app and a terminal, built at different times in different stacks,
and each reset left the others behind ([Episodes
183](../../docs/transcripts/183.md), [196](../../docs/transcripts/196.md) and
[275](../../docs/transcripts/275.md)). A feature on one surface was missing
on the next. This book builds one target at a time instead, and each new
target reuses what the first one built.

"One app" means three separate things, and the book never mixes them up:

- **Shared application code.** The state, actions, routes and components of
  chapters 1 to 9 are Coax, compiled once to C for every native host. A host
  draws them and runs the effects they ask for. No host has its own product
  logic.
- **Shared account and conversation data.** Your account and your
  conversations belong to you, not to a device. Chapter 35 moves a
  conversation between devices.
- **Agent execution.** A task runs in one place at a time: on a computer you
  enrolled or on our hosted service. That place is the task's execution
  owner. Every other client presents the task and never repeats its tool
  calls, model requests or payments. Chapter 37 builds the rule.

The native hosts already exist and draw the same display-list kit: AppKit
with Metal 4 on the Mac (the primary desktop host from chapter 1), UIKit with
Metal on iOS (unified GPU carrier in [`apps/openagents-ios/gpu-host.c`](../../apps/openagents-ios/gpu-host.c),
`#14023`), Wayland with Vulkan on Linux, Win32 with Vulkan on Windows
([`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows),
`#14046`, chapter 77), and the Vulkan host on Android
([GPU components, stages G6 and G7](../../components/gpu-components.md#stages)).
Each supplies the same three things: the focused field's editor
(`NSTextInputClient` on the Mac, `UITextField` on iOS, the Wayland
text-input protocol on Linux, `InputConnection` on Android), an
accessibility bridge built from the committed rows (`NSAccessibilityElement`,
`UIAccessibilityElement`, AT-SPI, virtual nodes), and system sheets. A wide
window shows the sidebar beside the chat page; the layout reads the window's
width, not the platform's name.

The desktop app already owns the primary chat workflow and configures the
computer it runs on as an environment (chapter 8): a resident environment
service (`coder-host`), runnable without the window, owns the agent's turns,
so closing the app never stops them (`#14027`, `#14032`). Both `coder-host`
and [`phone-agent-policy.coax`](../../coax/lib/agent/environment/phone-agent-policy.coax)
converge onto OpenAgents Core (`oa-worker`, capability `oa.core.v1`, models
`oa:auto`, `oa:gemini-4-neon`, and optional `$0` `oa:inkling-2-mini`) as the
primary engine across macOS and iOS Tailscale surfaces, while external
harnesses (`PA_AGY`, `PA_CLAUDE`) are gated behind opt-in delegated
sub-agent execution (`OA_ALLOW_LOCAL_HARNESSES=1`, `#14060`, `#14073`).
Phone-to-desktop reachability routes over **Tailnet → LAN → Nostr relay
failover** (`#13993`, `#14030`, `#14032`) without requiring a public port.
The [desktop and environment plan](../../docs/protocol/environments-claude-and-world-editing.md#31-one-desktop-product-two-independent-lifetimes)
owns that service.

This chapter qualifies the remaining native hosts—iOS, Linux desktop, and
Android—against the same shared modules and chapter 6's acceptance case
(tracked for mobile feature parity in `#14068`).
Each host claims nothing until chapter 6's case passes there with a real
input method and native accessibility (VoiceOver, AT-SPI, or TalkBack).

Each host qualifies on its own evidence. A host runs chapter 6's case
through the same `coax drive` operations and must give the same trace. A
host that lacks an operation refuses it. The web is a different
presentation and has its own chapter (chapter 11).

## What we build

A plan for what this chapter ships once the draft is approved:

- Full native host parity for the chat page, history, chips and settings on
  iOS (`apps/openagents-ios`), Linux desktop, and Android, consuming the
  same shared Coax modules as the primary macOS desktop app.
- Chapter 6's case and its device checks on each native host, with the
  targets recorded as they pass.

It builds on what exists today:

- [`apps/openagents-desktop`](../../apps/openagents-desktop/README.md)
- [`apps/shared/host/apple`](../../apps/shared/host/apple)
- [`apps/shared/host/linux`](../../apps/shared/host/linux)
- [`apps/shared/host/android`](../../apps/shared/host/android)
- [`components/gpu-components.md`](../../components/gpu-components.md)
- [`coax/native.md`](../../coax/native.md#layers-and-execution-ownership)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A message and its reply, between a surface and the hosted runtime | `openagents.chat.v1` `ConversationRequest`, `ConversationResult` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | Nostr kinds 25900 and 26900, encrypted; HTTP chat routes | Spec sketch; no `.proto` file ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), steps C5 and C9) |
| One conversation on every device | `openagents.sync.v1alpha1`, a log per conversation ([chapter 35](ch035-sync-across-devices.md)) | Binary frames on one WebSocket | Planned |

### The chapter's check

- **Source:** [`coax/lib/agent/environment/phone-agent-policy.coax`](../../coax/lib/agent/environment/phone-agent-policy.coax),
  [`apps/openagents-ios/ios-app.coax`](../../apps/openagents-ios/ios-app.coax),
  [`apps/openagents-ios/gpu-page.coax`](../../apps/openagents-ios/gpu-page.coax),
  [`openagents/examples/phone-host-agent-policy.coax`](../examples/phone-host-agent-policy.coax),
  and [`openagents/examples/phone-host-agent-policy.out`](../examples/phone-host-agent-policy.out)
  (remaining: chapter 6's `openagents/examples/cases/chat-page.ops` across
  `macos-arm64`, `ios-simulator`, `ios-device`, and `android-emulator`).
- **Trace:** offers solely `OpenAgents Core` (`offer=1:OpenAgents Core`) when
  `oa-worker` is available, maps `oa:auto`, legacy `gemini-4-neon`, and `$0`
  `oa:inkling-2-mini` onto `PA_CORE` wire model IDs (`oa:auto`, `oa:cloud`,
  `oa:local`), clamps local `xhigh` effort to `high`, and emits the fallback
  notice when an external harness is asked for primary chat.
- **Refusal:** `pa_subagent_harness_allowed` refuses `PA_AGY` and `PA_CLAUDE`
  while `allow_local_harnesses=false`; missing local runtime falls back to
  `oa:auto`; `pa_agy_catalog_slug_ok` refuses bare sentinels (`default`),
  leading dashes (`-flag`), and whitespace (`oa:bad slug`). On native hosts, a
  host lacking a `coax drive` operation refuses it by name and is not named as
  a target.
- **Independent check:** `coax/bin/coax run openagents/examples/phone-host-agent-policy.coax`
  and `--release` match [`openagents/examples/phone-host-agent-policy.out`](../examples/phone-host-agent-policy.out),
  alongside `#authority` tests and mutation checks in
  [`coax/lib/agent/environment/phone-agent-policy.coax`](../../coax/lib/agent/environment/phone-agent-policy.coax)
  (remaining: each platform's own accessibility UI test with no `coax drive`).

Planned paths: none. The desktop app is
[`apps/openagents-desktop`](../../apps/openagents-desktop/README.md); hosts
extend [`apps/shared/host`](../../apps/shared/host).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, one agent behind every surface
  - [Episode 172, *Sync Engine*](../../docs/transcripts/172.md): state that syncs across browsers
  - [Episode 187, *Mobile Sync*](../../docs/transcripts/187.md)
  - [Episode 196, *Ditch the TUI*](../../docs/transcripts/196.md): against a terminal-only product
  - [Episode 275, *Coder*](../../docs/transcripts/275.md): one suite across computers, cloud and mobile
- **Old code at `coder-v1.0.0`, crates:** `openagents-chat`, `openagents-chat-app`, `openagents-ui`, `coder-sync`, `openagents-connect`
- **NIPs:** [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md)
- **Also:**
  - [The apps roadmap](../../docs/apps/2026-11-10-apps-roadmap.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P16

## Open questions for the owner

1. The [sync spec](../../docs/sync/README.md) recommends syncing a
   conversation through our service, readable by the server at first
   (chapter 35). Confirm that, and say which collections get end-to-end
   encryption first.

## Source listing

Run the Chapter 10 example in [`openagents/examples/phone-host-agent-policy.coax`](../examples/phone-host-agent-policy.coax) (committed transcript [`openagents/examples/phone-host-agent-policy.out`](../examples/phone-host-agent-policy.out)):

<!-- source: openagents/examples/phone-host-agent-policy.coax phone-host-agent-policy -->
```jai
phap_run :: () -> bool {
  // 1. Converged host with OpenAgents Core + local Inkling + opt-out harnesses.
  f := PaFacts.{claude = true, gemini = true, agy = true, inkling = true, core = true, core_local = true, allow_local_harnesses = false};
  offered: [5] int;
  n_off := pa_offer(f, offered[..]);

  p_auto := pa_pick(f, "oa:auto");
  p_legacy := pa_pick(f, "gemini-4-neon");
  p_local := pa_pick(f, "oa:inkling-2-mini");
  eff_local := pa_effort(p_local.engine, p_local.model, "xhigh");
  print("offer=%:% auto=%/% legacy=%/% local=%/% effort=%\n",
    n_off, PA_ENGINE_NAME[offered[0]],
    PA_ENGINE_NAME[p_auto.engine], pa_core_task_model(p_auto.model),
    PA_ENGINE_NAME[p_legacy.engine], pa_core_task_model(p_legacy.model),
    PA_ENGINE_NAME[p_local.engine], pa_core_task_model(p_local.model), eff_local);

  // 2. External harnesses never hijack primary chat when OpenAgents Core is ready;
  // sub-agent delegation stays gated behind explicit allow_local_harnesses opt-in.
  p_claude := pa_pick(f, "claude-sonnet-4-5");
  note_buf: [256] u8;
  note := pa_note(p_claude, note_buf[..]);
  sub_core := pa_subagent_harness_allowed(f, PA_CORE);
  sub_agy_off := pa_subagent_harness_allowed(f, PA_AGY);
  sub_claude_off := pa_subagent_harness_allowed(f, PA_CLAUDE);

  f_opt := f;
  f_opt.allow_local_harnesses = true;
  sub_agy_on := pa_subagent_harness_allowed(f_opt, PA_AGY);
  sub_claude_on := pa_subagent_harness_allowed(f_opt, PA_CLAUDE);
  print("fallback=% label=% note=%\n", p_claude.fell_back, pa_label(p_claude), note);
  print("subagent core=% off=(agy=%,claude=%) on=(agy=%,claude=%)\n",
    sub_core, sub_agy_off, sub_claude_off, sub_agy_on, sub_claude_on);

  // 3. Refusals: missing local runtime falls back to cloud; bare/flag/malformed slugs refuse.
  f_nolocal := f;
  f_nolocal.core_local = false;
  p_nolocal := pa_pick(f_nolocal, "oa:inkling-2-mini");
  slug_ok := pa_agy_catalog_slug_ok("oa:gemini-4-neon");
  slug_bad_sentinel := pa_agy_catalog_slug_ok("default");
  slug_bad_flag := pa_agy_catalog_slug_ok("-flag");
  slug_bad_space := pa_agy_catalog_slug_ok("oa:bad slug");
  print("refuse_nolocal=%/% slug_ok=% refuse_slug=(%,%,%)\n",
    p_nolocal.fell_back, p_nolocal.model, slug_ok,
    !slug_bad_sentinel, !slug_bad_flag, !slug_bad_space);

  return n_off == 1 && offered[0] == PA_CORE &&
         p_auto.engine == PA_CORE && pa_core_task_model(p_auto.model) == "oa:auto" &&
         p_legacy.engine == PA_CORE && pa_core_task_model(p_legacy.model) == "oa:cloud" &&
         p_local.engine == PA_CORE && pa_core_task_model(p_local.model) == "oa:local" &&
         eff_local == "high" && p_claude.fell_back && p_claude.engine == PA_CORE &&
         sub_core && !sub_agy_off && !sub_claude_off && sub_agy_on && sub_claude_on &&
         p_nolocal.fell_back && p_nolocal.model == "oa:auto" &&
         slug_ok && !slug_bad_sentinel && !slug_bad_flag && !slug_bad_space;
}
```

Output (`coax/bin/coax run openagents/examples/phone-host-agent-policy.coax`, identical in `--release`):

```text
offer=1:OpenAgents Core auto=OpenAgents Core/oa:auto legacy=OpenAgents Core/oa:cloud local=OpenAgents Core/oa:local effort=high
fallback=true label=OpenAgents Auto note=Claude Code isn't set up on this computer, so this chat runs on OpenAgents Auto.
subagent core=true off=(agy=false,claude=false) on=(agy=true,claude=true)
refuse_nolocal=true/oa:auto slug_ok=true refuse_slug=(true,true,true)
ok=true
```
