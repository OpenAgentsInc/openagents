# Learned Pylon onboarding verification — 2026-10-30

This records the native learned-model join path. The user-facing instructions remain the [canonical connect guide](../connect-a-pylon.md). The source changes are `61e8cc5eed` and `a4a6ec949c`; the model-manifest exporter is `62c328ee2c`.

## What the installer now proves

`--model elephant-1 --self-test` builds the owned Coax native runtime for Metal or CUDA. It compiles the actual generated GPU kernels and asks the existing Coax device plan for a 512-token placement before downloading model data. Unsupported devices fail before the 18.8 GB transfer; there is no CPU or toy fallback.

The source of truth for the four public asset URLs, revision, hashes, model identity and capacity is `psionic/lib/ops/model-manifest.coax`, reading the existing `ps_target` registry. The installer does not keep a second set of pins. The downloaded original weights are 18,819,722,392 bytes at revision `b3f8a93462fb1051a9de031e94a7ff380e254072`:

```
elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba
```

Download-cache checks use the complete SHA-256. The native Coax engine independently authenticates the original checkpoint and metadata before loading it. A six-case frozen native doctor then has to pass before the Pylon may advertise. Finally, a separately built native Fleet buyer pins the new provider's public key and exact checkpoint identity, sends an encrypted request through the public relay, and checks the returned `wet` decision. The script has no hosted-API fallback. A successful self-test leaves the node foreground serving until stopped.

## Focused checks

The recorded [checks](../../psionic/fixtures/pylon/learned-selfserve/2026-10-30/checks.json) cover eight real HTTPS/cache tests and four lifecycle tests. The tests use a real temporary TLS server with an independent byte fixture; they verify transfer, actual Range resume, full-checksum cache hits, same-length corruption repair, rejection of bad complete files, HTTPS downgrade refusal, symlink preservation and an exclusive cache lock. A one-second interrupted transfer retains its prefix and the next invocation resumes it to the exact digest.

The lifecycle test was red for stop requests during build/warmup before the fix. It now proves that `--stop` cancels the owned child and never launches a Pylon afterward. Restart retains the private label and slug. Actual Git checks prove the default standalone clone excludes `/state/`, `/models/` and `/build/` without changing a developer's `--source` checkout. Private labels are absent from argv and terminal output.

Actual Metal and CUDA preflight checks compile the generated kernels, accept the 512-token model placement and refuse a one-MiB budget or 513-token shape without reading checkpoints or opening a listener. A separate CPU binary refuses `--check-device` with no CPU fallback. The CUDA check caught and fixed an adapter error: its maximum storage buffer can hold Mammoth's 527 stream flags, while Elephant's Coax plan requires its exact 275-element view. The probe now gets that view length from `dp_streamable(dp_model(0))`; model arithmetic and the planner were unchanged.

Compiler qualification, model numerics and Nostr crypto gates were not repeated for unrelated remote commits. The actual changed adapter and installer failure paths were exercised directly.

## Runtime boundaries

The onboarding path serves original Elephant on the qualified native GPU backends. It does not claim that the legacy small-model Rust server or the Coax GGUF container reader implements this runtime. CPU Elephant is deliberately not offered as a fallback. The qualified configurations are the M5 Max 128 GB Mac with macOS/Xcode 26 and the existing Linux CUDA hosts; the actual device/kernel/memory probe remains required.

The new node has a private throwaway demo identity, retained across restarts with its provider-bound monotonic generation. The production key audit gate is unchanged. Model server and buyer HTTP listeners remain loopback-only. These unpaid demo tasks use synthetic prompts. A signed advertisement is not hardware attestation or a proof that an arbitrary operator is running the named model.

Joining and proving the user's own provider does not enroll that key in the hosted authenticated API's operator-pinned provider routes. The existing approved production model providers were not restarted or changed by these tests.

## Full-flow receipts

### macOS / Metal

The [Mac receipt](../../psionic/fixtures/pylon/learned-selfserve/2026-10-30/mac.json) records the final `a4a6ec949c` runtime source on macOS 26.4 / M5 Max / 128 GB. An isolated APFS clone of the existing four original public files passed full installer checksums and independent native admission; the user's originals were unchanged. The real 23-kernel probe passed before model reads. Native placement kept 15,140 MiB resident with no streaming. The six-case doctor passed in 1,917 ms with largest error 10,345 billionths against tolerance 500,000. The encrypted relay task returned `yes` with probability `0.757192428` in 438 ms from fresh provider `1c9599a342973507c598a616e74aca52214626b73d34f49fc5568c725a1dc3bd`. Final setup, including input/cache checks and builds, took 166.121 seconds. The joined node was stopped cleanly, emitted an offline beacon, removed its lock and released its model engine.

A genuinely empty Mac download attempt encountered curl error 56 after retaining 8.84 GB. Real HTTP 206 ranges and retained samples match the original bytes. The corrected downloader resumed that partial without rewinding it. An explicit stop returned zero, removed the join lock, retained 8,886,970,905 bytes and never launched an engine or Pylon. This is interruption/resume/stop evidence, **not a completed cold Mac download**. The successful Metal flow used the authenticated isolated cache; the Linux receipt below covers a full cold public transfer.

### Linux / CUDA

The [Linux receipt](../../psionic/fixtures/pylon/learned-selfserve/2026-10-30/linux.json) records a completed cold setup on the existing RTX PRO 6000 96 GB host. The dedicated cache did not exist before invocation. All four original public Hugging Face files were downloaded into new files and passed their complete SHA-256 checks; none of the existing production model files or the preliminary hardlinked cache was reused. The runtime independently authenticated the new originals and kept its verified-file record only after admission.

The actual 21-kernel CUDA preflight passed before downloading. Native placement kept 15,140 MiB resident without streaming. The six-case doctor passed in 745 ms, worst error 4,910 billionths against 500,000. Fresh provider `3a2b63db8c26dbb5bf183bac8234240a08ceed9c751f5a8188e9690eac872d17` answered the real encrypted relay task with `yes = 0.757197863` in 91 ms. The observed launcher-to-answer setup time was 375.82 seconds, reusing the qualified Linux Coax compiler and installed CUDA/TLS prerequisites while building the native services. This is a specific measured run, not a promised installation time.

The source checkout was `b9845e5` with the published `a4a6ec949c` installer/native-host files applied. Their exact hashes are recorded. The remaining Psionic runtime and Coax compiler source were unchanged between those commits; only the installer and the host adapter/test files differed. The official compiler was built/qualified once on Linux and its matching provenance was verified by the installer.

After the answer, the new node emitted its offline generation-2 beacon, removed its lock, closed its model listener and released the GPU context. Device use returned to its pre-test 66,590 MiB of 97,887 MiB. Both retained production model servers and both retained Pylon units were still active. No VM, GPU, public model port or persistent service was added for this proof.

### Production endpoint continuity

The parent operator also exercised the guide's authenticated [production API request](../../psionic/fixtures/pylon/learned-selfserve/2026-10-30/production-api.json) during the cold setup. The helper required HTTP 200 and returned the correct operator-pinned Elephant identity/provider, `yes = 0.757197863`, with native latency 92 ms. No auth header, private key or token is included in the receipt. Existing auth/refusal gates and model arithmetic were not repeated.
