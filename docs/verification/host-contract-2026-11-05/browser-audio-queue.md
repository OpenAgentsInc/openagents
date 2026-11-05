# Bounded actual browser audio offers

The browser now leases command retention through the existing shared Coax
inbox queue before creating a readiness/decode continuation. It uses the
actual 1,024-byte line scratch, permitting 64 offers and four scratch
buffers of encoded bytes. Refusal is counted explicitly. Accepted commands
own independent byte copies; no queued closure retains a substring of an
entire producer packet. A lease lasts through the actual worklet reply or
owner cancellation. Whole-packet capacity and pre-contract behavior stay
unchanged.

The actual SDK fixture passes 38 required observations: exact 1,024-byte
multibyte UTF-8, the 4,096-byte total, 64 empty offers, complete refusal,
release/reuse, duplicate stale release, oversized encoding refusal,
immutable copies after scratch reuse, and separate old/new engine owners.
The check-first observation authority kills all 46 root mutants without
equivalents; the unchanged 106-mutant queue policy gate is reused.

The real `host.js` workload holds the actual
`AudioWorklet.prototype.addModule` effect. Exactly 64 six-byte controls
remain owned (384 bytes); the 65th is refused before another continuation
is created. Releasing the real SDK effect delivers all 64 controls and
returns both queue counters to zero. The pre-gesture PCM clock and decoded
word count remain zero. All eight product observations pass, and its
16-mutant authority has zero survivors or equivalents.

Artifacts under
`~/.openagents/aethyrforge/host-contract-12869/browser-audio-queue/` include
the exact 19,919-byte reactor, cache C/headers/objects, old published DSP
reactor, fixture realm/shader, actual receipts, observers and SDK sources.
The generated service C joins 13 source files with zero differences under
the same genuine c224/admitted 598 preparation. Compiler policy and
graphics products are unchanged. Exact hashes are in the committed record.

These receipts qualify bounded per-engine offer retention. Global PCM
ownership through decoder scratch, worklet installs and complete old-node
retirement remains open, along with browser socket queues and the other
whole-host exits. No DSP sample tolerance, display/default selection or
issue status changed. Both browser runs were headless and muted.
