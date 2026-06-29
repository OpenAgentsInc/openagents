# Forge Docker Verification Runner

`apps/pylon/src/forge-verification-runner.ts` is the first Pylon execution
primitive for Forge-owned verification at fleet scale.

The runner accepts a `ForgeDispatchVerificationCommand` whose
`runner_ref` is `forge.verification.runner.docker_bun.v0.1` and builds a
single `docker run` argv for Bun verification. It does not invoke a shell.

The container boundary is fixed by default:

- `--network none`
- `--pull=never`
- `--read-only`
- read-only `/workspace` bind mount
- `--cap-drop ALL`
- `--security-opt no-new-privileges`
- CPU, memory, memory-swap, PID, and timeout limits
- noexec tmpfs mounts for `/tmp` and `/home/bun`

The result is a redacted receipt:

- command, workspace, image, argv, and verification refs
- pass/fail/timeout/error status
- exit code
- stdout/stderr byte counts and digest refs
- the isolation settings that were applied

It deliberately does not persist raw stdout, stderr, source contents, provider
payloads, git tokens, wallet material, or local absolute paths beyond hashed
refs. Tests use an injected command runner so CI can prove the isolation argv
and receipt shape without requiring Docker on the test host.
