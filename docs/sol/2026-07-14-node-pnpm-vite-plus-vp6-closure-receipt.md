# Node, pnpm, and Vite Plus VP-6 closure receipt

- Class: receipt
- Date: 2026-07-14
- Status: complete
- Dispatch: no; closure evidence for #8800 and epic #8777
- Parent: #8777

## Result

No supported runtime, build, test, package, hook, release, deploy, container,
or operator path requires Bun. Root and nested Bun lockfiles/configs, the
SQLite Bun adapter, React Native Bun compatibility shim, Bun test imports,
Bun Docker/runtime commands, and the non-MVP Sites applications are deleted.
The perimeter allowlist is empty.

The public `/treasury` route and all payment, wallet, credit, payout,
settlement, tip, paid-capacity, and Sites route families are rejected by the
retired-capability route registry. Historical ledger vocabulary and audit
fixtures may remain as inert evidence; they grant no reachable mutation or
custody authority. Wallet recovery is deliberately out-of-band and documented
in `docs/ops/2026-07-14-vp1-treasury-wallet-recovery-runbook.md`.

## Closure scans

- retained production source: zero `Bun.*`, `bun:*`, Bun module imports, or
  Bun shebangs;
- shell/Docker/hooks: zero Bun installers, commands, or base images;
- lock/config authority: one pnpm lockfile and no Bun lock/config file; and
- permitted residual text: scanners that recognize forbidden syntax,
  historical audits, and literal non-operational fixtures only.

## Verification matrix

- frozen install, check, typecheck, build, Vite Plus plugin fixtures, Sol
  consistency, exact-tarball offline install, and focused DOM rerun: pass;
- full Vite Plus suite: traversed the repository corpus without reporting a
  failure, then required a bounded interrupt after the runner stopped emitting
  output and did not exit; focused regression suites passed independently, so
  the non-exiting open-handle/reporter behavior remains tooling debt rather
  than a represented green terminal exit; and
- OCI daemon build: unavailable on the proof host, bounded and disclosed in
  the VP-5 receipt rather than represented as green.

This closes the supported conversion boundary. Reintroducing Bun or any money
path is a new policy/product change requiring its own authority and invariant
evidence.
