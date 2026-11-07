# 84. One realm on every host

> **Status: partly implemented.** Aethyr has a versioned host contract and
> shared lifecycle rules. Browser, Linux Vulkan and Mac Metal have scoped
> verification records; full realm and service adoption remains separate.

The host owns platform effects: the window, devices, GPU, audio, files and
sockets. Coax owns gameplay, admission and recovery policy.
[`aethyr/host/README.md`](../host/README.md) specifies `aethyr-host-v1`,
including initialization, asynchronous ownership and failure behavior.
Missing a contract version keeps a legacy face; it does not qualify that
face as a v1 host.

## One session, several hosts

[`aethyr/session/lifecycle.coax`](../session/lifecycle.coax) defines blur,
hide, return, socket replacement, device loss and module trap behavior.
Blur releases held inputs. A long suspension revokes pending commands and
asks for accepted state. Device loss keeps the session while drawing stops
and resources are restored; it does not replay commands.

[Chapter 95](ch095-offline-is-a-local-server.md) runs the same room tick in
native C and a browser worker, including shared math.
[Chapter 98](ch098-versions-deploys-and-reconnect.md) explains the seat,
socket and owner lifetimes. The server's authoritative clock continues when
a page hides; pausing local prediction does not pause the room.

The lifecycle contract's client and room sides are adopted in Everglade.
That fact does not establish every platform event on every native face.
The versioned host adapters and their evidence are documented in
[`aethyr/host/`](../host/README.md) and
[`docs/verse/native-gpu.md`](../../docs/verse/native-gpu.md).
The Mac host is selected explicitly with `--world metal`; WebKit remains
the desktop default pending complete three-host service migration.

## Verify the boundary you use

The bounded streaming WebGPU carrier adopts
[render/loss-policy.coax](../render/loss-policy.coax) for the existing
ten-second recovery deadline and three-losses-per-minute contract. Actual
loss while rendering or building, late device completion and stopped
cleanup have [separate receipts](../../docs/verification/aethyr-streaming-12990/walk-automatic-loss.md).
The Coax face refuses paid frames during rebuilding and, after a stop,
admits only the session's real closing phase. This fixture is separate
from host-v1 adoption. The [native draw-result profile](../../docs/verification/aethyr-streaming-12990/walk-native-loss.md)
now retains a submitted command pool/fence and resource borrows through a
pending result, then uses the same checked loss policy for reported draw
loss. Real Vulkan rendering with labelled SDK-result injection qualifies
this boundary; the broader native factory/upload and rescue surface remains
unqualified. The additive
[whole-reactor browser rescue profile](../../docs/verification/aethyr-streaming-12990/walk-persistent-rescue.md)
retains allocator-backed sources and queued facts, restores one exact-build
save and suppresses another save before 300 frame starts. Actual trap,
failed-adapter and pending-load reloads qualify a two-MiB desktop profile;
they do not establish native or larger-memory rescue.

```shell
coax/bin/coax test aethyr/session/lifecycle.coax --library
```

Contract checks establish policy, not device behavior. Host qualification
also needs actual input, transport, audio, resource retirement and recovery
observations. See the
[host verification records](../../docs/verification/host-contract-2026-11-05/README.md)
and the [Mac acceptance record](../../docs/verification/metal-host-2026-11-05/current-85c-final.md).
Do not infer a cross-architecture session digest or a universal recovery
latency from a component's byte comparison. Each shipped realm and host
needs its own complete evidence.
