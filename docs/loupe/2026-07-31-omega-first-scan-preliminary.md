# Loupe scan of Omega — results

Status: **SCAN COMPLETE. NOTHING VERIFIED.**

## Plain summary

- The scan finished. It looked at **99 files** of our own code and reported
  **132 possible problems** — 12 rated High, 106 Medium, 14 Low.
- **Nothing has been double-checked.** Not by a second agent, not by a human.
- The double-check step could not run, because **Loupe has a bug** (§6). Its
  worker reports "verdict saved," its server replies "no verdict exists," the
  job retries three times and dies. Every one of the 132 is stuck part-way.
- **The scanner machine is stopped** so it stops costing money. Its disk is
  kept, so the findings are still there.

**So: treat every line below as an unconfirmed claim by one AI, and nothing
more.** Some will be real. Some will be wrong. Nobody knows which yet, and
finding out is the next job.

This is the first time we have run Project Loupe against our own code. See
[`docs/loupe/README.md`](README.md) for what Loupe is and how it works.

---

## 1. Run parameters

| Field | Value |
| --- | --- |
| Target | `https://github.com/OpenAgentsInc/omega.git` |
| Scanned SHA | `8e45faa7bcedcb89c18895710206e77afd35e5e8` (`origin/main`) |
| Host | GCE `oa-loupe-scanner-1`, n2-standard-8, Debian 12, `us-central1-a`, project `openagentsgemini` |
| Loupe commit | `c94aac5` |
| Agent | `codex` / `gpt-5.5` / effort `xhigh`, for both scan and verify |
| Sandbox | **bubblewrap active** (`bubblewrap available; LLM scanners sandboxed`) |
| Reporting | `manual` — nothing auto-files an issue |
| Verification | enabled |
| Approval gate | enabled |
| Scan started | 2026-07-31 21:51:39 UTC |

### Scope

Omega carries **1,718 `.rs` files**, but only ~86 live in the 25 `crates/omega*`
crates we own — the rest is inherited Zed. Loupe launches **one agent session
per source file**, so an unscoped run would have spent roughly twenty times as
much to review upstream code we did not write.

The repo was registered through `POST /v1/repos` with a 203-entry
`exclude_path_substrings` list covering all 199 non-`omega` crates plus
`tooling`, `extensions`, `target`, and `assets`. `loupectl repo add` does not
expose `scanner_config`, so direct API registration is the only route.

Result: **99 files in scope.**

### Final run figures

| | |
| --- | --- |
| Files scanned | **99 / 99 (complete)** |
| Discovery wall time | ~3h20m (21:51 → ~01:15 UTC) |
| Rate | ~0.5 files/min at 4 concurrent |
| **Findings** | **132** — 12 High, 106 Medium, 14 Low |
| Scan job | `Succeeded` |
| Verify jobs | **0 succeeded, 14 failed, 118 never ran** (see §6) |
| Findings verified | **0** |
| Discovery errors / auth failures / rate limits | 0 |

Roughly 1.3 findings per file. Discovery itself was completely clean — no
provider errors, no rate limiting, no timeouts. The failure is entirely in the
verification stage.

---

## 2. The first 17 findings

The full set is 132. Only the first 17 are tabulated here, from the early part
of the run; they are kept because the surrounding analysis refers to them. The
remaining 115 live in the scanner's database (§7 explains how to get them out)
and the later ones are more interesting — the alphabetical walk did not reach
our own security-relevant crates until well past this table.

All 132, including these, are unverified.

| # | Sev | Location | Title |
| --- | --- | --- | --- |
| 12 | **High** | `crates/omega/src/zed/remote_connections.rs:68` | Option-like SSH host is treated as an ssh option |
| 1 | Medium | `crates/omega/build.rs:116` | Escape OUT_DIR paths before building PowerShell commands |
| 2 | Medium | `crates/omega/src/main.rs:2414` | Load conpty.dll through a safe Windows search path |
| 3 | Medium | `crates/omega/src/main.rs:1209` | Validate git clone deep-link URLs before cloning |
| 7 | Medium | `crates/omega/src/reliability.rs:73` | Validate remote minidumps before uploading |
| 8 | Medium | `crates/omega/src/zed/open_listener.rs:269` | Validate git commit link revisions before invoking git |
| 9 | Medium | `crates/omega/src/zed/open_url_modal.rs:57` | Reject zed-cli URLs in the open URL modal |
| 10 | Medium | `crates/omega/src/zed/remote_debug.rs:12` | Gate remote debug disconnect actions from production startup |
| 11 | Medium | `crates/omega/src/zed/windows_only_instance.rs:48` | Unauthenticated Windows pipe accepts forged CLI IPC |
| 14 | Medium | `crates/omega_acp_server/src/omega_acp_server.rs:969` | Bound newline frames before reading them |
| 15 | Medium | `crates/omega_acp_server/src/omega_acp_server.rs:773` | Limit sessions per served connection |
| 16 | Medium | `crates/omega_acp_server/src/omega_acp_server.rs:945` | Limit idle served ACP connections |
| 17 | Medium | `crates/omega_agent_detect/src/omega_agent_detect.rs:394` | Ignore relative PATH entries during agent discovery |
| 4 | Low | `crates/omega/src/reliability/hang_detection/telemetry.rs:249` | Hang diagnostics bypass telemetry diagnostics opt-out |
| 5 | Low | `crates/omega/src/zed/mac_only_instance.rs:165` | Static localhost handshake lets local users spoof an existing instance |
| 6 | Low | `crates/omega/src/zed/mac_only_instance.rs:121` | Port squatting makes singleton startup fail open without a lock |
| 13 | Low | `crates/omega/src/zed/windows_only_instance.rs:31` | Predictable instance mutex lets fake first instance steal CLI requests |

Distribution: 1 High, 12 Medium, 4 Low. By crate: 13 `crates/omega`,
3 `omega_acp_server`, 1 `omega_agent_detect`.

### Apparent clusters

Three themes are visible even at 30% coverage. Whether they survive
verification is exactly the open question.

1. **Untrusted input reaching a subprocess or a URL handler** — findings 12, 3,
   8, 9, 1. Deep links, commit links, clone URLs, and the open-URL modal all
   pass caller-controlled strings toward `git`, `ssh`, or a shell.
2. **Local IPC and singleton-instance trust** — findings 5, 6, 11, 13. The
   single-instance mechanisms on macOS and Windows authenticate weakly or not
   at all, so a local process can impersonate the first instance and receive
   CLI requests.
3. **Unbounded resources in the ACP server** — findings 14, 15, 16. No frame
   length bound, no per-connection session cap, no idle-connection reaping.
   `omega_acp_server` is the surface that speaks to external agents, which
   makes this the cluster most specific to what Omega is for.

---

## 3. The High finding, in full

`crates/omega/src/zed/remote_connections.rs:68`, CWE-78, fingerprint
`70d4395b66a941e4…`. Quoted from the finding body:

> `RemoteSettings::connection_options_for` copies the caller-controlled SSH
> host directly into `SshConnectionOptions` with `host.into()` and does not
> reject destination strings that begin with `-`. Those options are later
> consumed by the in-tree SSH transport, which appends `ssh_destination()` to
> the `ssh` argv after its own flags without a `--` end-of-options delimiter. A
> host such as `-oProxyCommand=sh${IFS}-c${IFS}id` is therefore interpreted by
> OpenSSH as an option, not as a destination, causing `ProxyCommand` to execute
> locally as the Zed user before any remote authentication.

The agent's PoC is a regression test asserting that `--` immediately precedes
the destination in the built argv:

```rust
let dangerous_destination = "-oProxyCommand=sh${IFS}-c${IFS}id";
let command = build_command_posix(/* … */, dangerous_destination, Interactive::No)?;
let destination_index = command.args.iter()
    .position(|arg| arg == dangerous_destination)
    .context("missing ssh destination")?;
let previous_arg = destination_index.checked_sub(1)
    .and_then(|index| command.args.get(index))
    .context("ssh destination was first argument")?;
assert_eq!(previous_arg.as_str(), "--");
```

This is the argument-injection class: a hostname beginning with `-` consumed as
an `ssh` flag rather than a destination. It is a well-known shape, it is the
kind of bug that survives review for years, and the PoC is falsifiable — apply
it and it should fail on HEAD.

### It probably is not ours

**The finding is at an Omega call site, but the PoC patches
`crates/remote/src/transport/ssh.rs` — an inherited Zed crate that we excluded
from this scan.** The missing `--` delimiter is in upstream's argv
construction, not in our code.

If that holds after verification, this is **an upstream Zed vulnerability that
Omega inherits**, not an Omega-specific defect. That changes what to do with
it: it becomes a responsible-disclosure matter with the Zed project, and it
means the same bug is present in shipping Zed.

Two consequences worth stating plainly:

- **Do not open a public issue on this before that question is settled.**
  Publishing a vulnerability claim is a public claim, and unsupported public
  claims are reserved under `AUTHORITY.md`. Confirm the upstream/Omega boundary
  first, then follow disclosure norms.
- **The scope decision that saved ~20x cost also hid the fix site.** Excluding
  inherited crates was right for cost, but the agent found a bug whose remedy
  lives in the excluded region. It could only do so because the *call site* was
  in scope. A future run that wants upstream coverage needs a different scope,
  and should expect a materially larger bill.

---

## 4. Caveats that bound every number above

- **Nothing is verified, and now nothing can be** without fixing the bug in §6.
  All 132 are stuck in `validating`. The verifier — an independent Codex
  session, and per Loupe's design one whose verdict locks before it is allowed
  to write a fix — ran and produced nothing the server kept. Expect a
  meaningful share of these to be dismissed once it works; that dismissal rate
  is the single most useful unknown here.
- **Nothing has been read by a human.** No one has triaged any of the 132.
- **Severity is the discovering agent's own rating**, and Loupe's prompt tells
  it to be conservative. It is not a calibrated score.
- **The table in §2 shows 17 of 132.** It is an early slice, not a sample —
  the alphabetical walk had not yet reached `omega_community`,
  `omega_effectd`, `omega_identity`, `omega_signer_broker`, or
  `omega_zero_base`. Do not infer where bugs concentrate from it.
- **A finding is not a vulnerability until its PoC is applied and observed to
  fail on HEAD.** Loupe's `validate_poc` only proves the diff *applies*
  (`git apply --check`); it does not run the test. Applying and running each PoC
  is a separate step nobody has done.
- **This scan is bound to `origin/main` at GitHub**, not to a local working
  checkout. An earlier aborted run on macOS targeted the local checkout at
  `acd0f5324a`; findings are not directly comparable across the two.

---

## 5. The aborted macOS run

The first attempt ran on the owner's Mac and was stopped and discarded. It is
recorded here because the reason is a real constraint, not an accident.

**bubblewrap has no macOS port**, so that run required
`LOUPE_DISABLE_SANDBOX=1` and the worker warned `LLM scanners running without
isolation`. Upstream treats that as a dev-only override, and it is why the
README states LLM scanning runs on Linux workers only. Moving to GCE was not
housekeeping — it is the difference between the sandbox engaging and not.

Everything else was held constant between the two runs (same exclusion list,
same 99 files, same agent, model, and effort), so the Linux run supersedes it
cleanly. The macOS run's server, worker, and orphaned `codex exec` children were
all terminated to avoid double-billing the same key.

Incidental finding: Omega is a **public** repository, so the VM clones directly
from GitHub with no credentials on the box at all. The only secret on the
scanner host is the OpenAI key at `~/loupe-openai.env`, mode `0600`. An attempt
to ship the source as a git bundle produced a 529MB artifact and was abandoned
— `git bundle` in this git version rejects `--depth`. Checking repository
visibility first would have skipped that entirely.

---

## 6. Why nothing got verified — a bug in Loupe

Discovery finished cleanly. **Verification never produced a single verdict**,
and the cause is a defect in Loupe itself, not in our setup or our key.

### What happens

Loupe queued one verify job per finding — 132 of them. Each one ran, and each
one ended like this:

```
INFO  verifier submitted verdict via MCP (runner skipping POST)
WARN  server returned 409 Conflict: successful verify completion
      requires a submitted verdict
```

Read those two lines together. **The worker says the verdict was saved. The
server says no verdict exists.** The worker then reports the job complete, the
server refuses with a 409, the job retries three times and fails.

### Why the worker believes it

`crates/loupe-worker/src/runner.rs:235-246`. When the verifier returns
`VerifyOutcome::Submitted`, the runner deliberately skips its own POST:

> MCP-driven verifier already POSTed via the MCP child's session-end flush.
> POSTing again from here would land a duplicate verification row; the runner
> stays out of the way.

The intent is right — avoid a duplicate row. The flaw is that **the runner
trusts that claim without ever confirming the server accepted it.** When the
child's flush does not land, the verdict is silently lost, and the only symptom
is a 409 at the very end.

### Likely mechanism — hypothesis, not established

The sandbox runs with `--die-with-parent`. When the codex process exits, the
MCP child may be killed **before** its session-end flush completes. That would
make this specific to sandboxed runs. We cannot confirm it from this run: the
earlier unsandboxed macOS attempt (§5) was stopped long before it reached
verification, so there is no comparison. Confirming or refuting this is one
experiment: run verification once with the sandbox disabled and see whether
verdicts land.

### Why this is worth writing down

It is the same failure shape this repository has been finding in its own code
all week: **one component believes work happened, the authority disagrees, and
nothing reconciles the two.** A ceiling that read zero for weeks, a sweep that
reported healthy while unable to close a loop it opened, a refusal reason with
no code path — and now a verifier that reports success into a void.

There is an uncomfortable lesson in it. Loupe's design discipline is genuinely
good, and the companion documents in this folder praise it at length —
emission only through a typed tool, verdicts locked before fixes, PoCs that
must fail on HEAD. **That discipline still did not prevent a fail-open gap
between its own child process and its own server.** Typed contracts at one
boundary do not protect the boundary you did not type.

### Cost consequence

14 verify jobs failed at 3 attempts each before this was caught; 118 never ran.
Had they run, all 132 would have failed identically — roughly **396 paid agent
sessions producing nothing**. The worker was stopped and the VM terminated for
that reason.

---

## 7. Current state of the machine and the data

- **VM `oa-loupe-scanner-1` is `TERMINATED`**, not deleted. No compute charge;
  the boot disk still incurs storage cost.
- **All 132 findings are intact** in the SQLCipher database at
  `~/loupe-data/loupe.sqlite` on that disk, sealed under `~/loupe-data/master.key`.
- To get them out: start the instance, start `loupe-server` with
  `LOUPE_MASTER_KEY=$(cat ~/loupe-data/master.key)`, and use
  `loupectl finding list 1 -n 300` / `loupectl finding show <id>`. The full
  bring-up is in `README.md`.
- The only secret on the box is the provider key at `~/loupe-openai.env`
  (mode `0600`). No GitHub credentials — Omega is public and was cloned
  anonymously.

---

## 8. Next steps

1. **Extract the 132 findings off the disk** before deleting anything. Right
   now they exist in exactly one place.
2. **Report the verify bug upstream.** It is well-characterized, reproducible,
   and a real contribution to a project whose source we have now read closely.
   §6 is most of the report already.
3. **Test the `--die-with-parent` hypothesis** by running verification once
   unsandboxed. One experiment either explains the bug or eliminates the
   explanation.
4. **Get verdicts another way** if upstream is slow: patch the runner to POST
   when the server reports no verdict, which is the conservative fix regardless
   of root cause.
5. **Settle the upstream question on finding 12 before anything is published.**
6. For findings that survive verification: apply each PoC to a scratch worktree
   and confirm it fails on HEAD. That is the step that converts a claim into
   evidence, and no PoC in this run has had it.
7. Decide whether a wider run against the inherited Zed crates is worth the
   cost — the High finding argues it might be, and the cost argues for scoping
   it to the crates our call sites actually reach.

Do not delete `oa-loupe-scanner-1` until step 1 is done.
