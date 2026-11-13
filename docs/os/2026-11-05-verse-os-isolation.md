# Metaverse OS isolation (proposal, draft)

Written 2026-11-05. Status: proposal. Nothing here is built. It extends
[2026-11-02-verse-os.md](2026-11-02-verse-os.md) and the bets in
[2026-10-30-coderos-and-coax-audit.md](2026-10-30-coderos-and-coax-audit.md).

This doc proposes how Metaverse OS keeps your identity, your wallet and your
machine safe when agents and other people's jobs run on it. It ends with
suggested issues, in order.

## Why now

On 2026-11-04, Greg Brockman posted that OpenAI's vulnerability pipeline had
found, validated and prepared patches for almost 1,000 Linux local privilege
escalations (LPEs). Hunter Leath (Archil) replied that he has always treated
the VM as the minimum boundary for untrusted code, with the VM's own
permissions cut to the minimum. Others argued that a provably safe Linux is
impossible.

Read together, the thread makes three points:

- Finding kernel LPEs is now cheap and automated. Anyone can buy the same
  capability, so the time between a bug and its exploit shrinks.
- Most of those bugs are memory-safety bugs in kernel code that an
  unprivileged process can reach (syscalls, ioctls, io_uring, netfilter,
  filesystems, BPF). A better permission model on the same kernel doesn't
  remove them. The problem is a large shared kernel in C, not DAC or UID 0.
- A shared kernel is not a boundary for untrusted code. A small VMM with a
  least-privilege identity is, if you count the VMM and KVM in what you
  trust.

Code an agent writes and runs is untrusted. An agent that can be
prompt-injected is an adversary inside the machine. A Pylon job is someone
else's work running on your hardware.

## Where Metaverse OS stands today

Metaverse OS, as specified, puts everything of value in one trust domain:

- One kernel, one user. The `pilot` account runs the session, the panels
  (any Linux program on the desk), the agents (V4's T key starts Claude
  Code, Codex or Coder in a project directory), and the Pylon.
- The Nostr key is `~/.openagents/cli/keys/PROFILE`, mode 0600. Mode 0600
  stops other users, not other processes of the same user. Every agent and
  panel can read it.
- The Spark wallet (V2) is planned for the same session.
- The audit's "engine and inference in one process" bet would put Psionic,
  which parses requests from outside when it serves as a Pylon, in the
  session's address space.
- V7's read-only A/B image protects the system. It does not protect
  `/home`, which is where the identity, the wallet and the agents' work
  live.

So one LPE, or one prompt-injected agent with plain user rights, reaches the
identity and the money. Isolation has to come before V2's wallet ships.

## Principles

These add to the six in the Metaverse OS spec.

1. **One machine, many trust domains.** The session is where trust is
   granted, not one big domain everything shares.
2. **Each presence is a domain.** An agent, a Pylon job or an untrusted
   program runs in its own microVM. The world already has a word for each
   one (presence, panel); each word is also a boundary.
3. **Keys never sit where code runs.** The identity and the wallet live in a
   small signer. Everything else asks it to sign or pay.
4. **Grants are checked code.** Which files, network hosts, devices,
   credentials and GPU share a domain gets is `#authority` Coax in the host
   description, the same as power limits and device grants today.
5. **The host is small.** The host runs the session, the VMM, the GPU driver
   and the signer. Nothing else.
6. **You can see the boundaries.** Isolation shows in the world and in the
   HUD, and a request that crosses a boundary is an event you answer.

## The design

### 1. Each presence is a microVM

Each agent run, each Pylon job and each program that isn't yours runs in its
own microVM (Firecracker-style: KVM, a minimal VMM, a jailer, seccomp,
cgroups; `projects/repos/firecracker` is the reference).

- **Agents.** A domain gets the project directory (shared read-write
  through virtiofs), a scoped token for its model account, and a network
  allowlist. It never sees `$HOME`, the key file or other projects.
- **Panels.** A guest's window reaches the session as a buffer over
  virtio-gpu (cross-domain contexts, as ChromeOS Crostini and Spectrum OS
  do), and the session draws it as a texture. The spec already describes a
  compositor as an engine that draws other programs' buffers as textures;
  only the buffer's source changes. Your own trusted programs can stay on
  the host compositor.
- **Pylon jobs.** A job runs in a domain with its inputs, a GPU share and no
  network beyond the job's endpoints. It never shares the session's
  process.

### 2. Keys live in a signer

A small Coax service, `verse-signer`, holds the Nostr key and the wallet.

- The key is sealed to the TPM at rest (V7's image already uses the TPM for
  `/home`) and decrypted only into the signer's memory.
- Other domains send requests over a socket: sign this event, pay this
  invoice. Every request passes an `#authority` policy: which kinds of
  event, which recipients, amounts per request and per day, rate limits.
  `pay/wallet.coax` is already `#authority`; the signer adds the process
  boundary around it.
- A request outside policy becomes an approval in the world. Nothing waits
  silently.
- The first-boot flow (V2) makes the key inside the signer. The seed words
  are shown once, as today, and the key file in `$HOME` goes away.

### 3. The host description is the security policy

Today the host description maps devices to programs and sets power limits.
It gains the grants for every domain:

```
domain claude-code/openagents
  files    ~/work/openagents (rw)
  network  api.anthropic.com, github.com
  secrets  anthropic token (scoped), github token (repo:openagents)
  gpu      none
  signer   sign kind 1 only, pay never
```

It stays Coax with `#test`, `#forall` and `#authority` checks (for example:
no domain but the signer reads the key; no Pylon domain has a secret; every
network grant names a host). `coax run` prints JSON. Nix turns that JSON
into VM configs, seccomp filters and Landlock rules for host processes.

### 4. A smaller host

Because everything else runs in guests, the host can drop most of what makes
Linux hard to defend:

- No setuid or setgid binaries in the image.
- No sshd and no listening services. Remote access goes into a guest.
- Unprivileged user namespaces off (`kernel.unprivileged_userns_clone=0`
  or the equivalent sysctl).
- `NoNewPrivileges=yes`, seccomp and a private `/tmp` for every host
  service.
- Kernel lockdown mode on; module loading limited to the image's own
  modules.

A test in `os/nix/tests` checks each item against the built image.

### 5. Coax where attackers reach

Coax doesn't claim general memory safety, and it compiles to C. It helps
here because the code that faces attackers is small, bounded parsing, which
Coax's checks fit:

- **The VMM's device models.** Virtio queue and descriptor handling reads
  memory a hostile guest controls, which is where VMM bugs live. Write it in
  a restricted Coax profile (no allocator, bounded loops, every
  guest-controlled index checked by an always-on `#assert`), use bounded
  codecs that check sizes before trusting a value, gate it with
  `#authority` and mutation testing, and use `coax deps --effects` to show
  it makes no foreign calls. Start with one device (virtio-vsock or
  virtio-console) beside an existing VMM, not a whole VMM.
- **The kernel's policy, without kernel modules.** The audit proposes a Coax
  BPF profile for `sched_ext`. The same profile can emit BPF-LSM programs
  that enforce per-domain policy in the kernel, generated from the host
  description and checked by the kernel's verifier before they load.
- **The signer** is Coax end to end, with no foreign calls but the socket,
  the TPM and the clock.

### 6. Boundaries you can see

- An agent stands inside a visible enclosure in the Grid, colored by its
  domain, the way Qubes colors window borders.
- The boot sequence (V1) reports isolation as real lines:
  `CLAUDE CODE · VM · NO KEYS · NET 2 HOSTS`, `WALLET · SEALED`,
  `HOST · NO SETUID · USERNS OFF`. A line that fails stays red and says why.
- A request that crosses a boundary (an agent asking to pay, a job asking
  for the camera) is an event in the world, answered by voice, key or pinch.

### 7. Later: Linux as a driver

The long-term version puts a capability microkernel (seL4) under the
session. Linux runs as a guest for the GPU driver and old programs. The
signer and the policy run as native components. This removes Linux from the
trusted core instead of trying to verify it.

Two things block it today:

- Coax has no freestanding profile (no libc). The audit lists this gap.
- NVIDIA's userspace driver is closed and runs only on Linux, so the display
  stays in a Linux domain. That's acceptable if keys and agents live
  elsewhere; Genode's Sculpt OS and Qubes' GUI domain work this way. An AMD
  or Intel GPU with Mesa is the more open path.

## What this doesn't solve

- **KVM and the GPU driver are still trusted.** A KVM escape or a GPU driver
  bug still reaches the host.
- **Coax is not memory-safe.** The claim for Coax code is small, bounded,
  mutation-tested code, not proven code.
- **Attackers can point AI at our code too.** Fuzz the device models, the
  signer and the policy with an automated bug-finding pipeline before
  shipping each one.
- **It costs speed.** Copying buffers across domains costs time, so in-world
  panels from guests will be slower than host panels. Psionic in the
  session's process stays fine for your own inference; Pylon serving moves
  to its own domain.
- **It changes V5.** The audit's "engine and inference in one process" bet
  applies only to inference you run for yourself.

## Suggested issues

In order. Each one ends on a check run on our own machines, as the Metaverse OS
milestones do.

| # | Title | Scope | Done when |
| --- | --- | --- | --- |
| I0 | Audit where agents and Pylon jobs run today | Write down, for the GCE pool, Archil, `coderos-4080` and a contributor's Pylon: the boundary each run gets (user, container, VM), its credentials, and what it can read on the host | A table in this doc, with each row checked on the machine, and an issue filed for each gap |
| I1 | Move the Nostr key into `verse-signer` | A Coax signer on a Unix socket; `openagents nostr` and the session sign through it; the key sealed to the TPM; `#authority` policy for event kinds | No process but the signer can read the key (a test reads it as `pilot` and fails); existing signing paths pass |
| I2 | Wallet behind the signer before V2 ships | The Spark wallet runs inside the signer, with `#authority` limits per payment and per day and in-world approval past them | A payment over the limit waits for approval; an agent domain can't reach the wallet directly |
| I3 | Agent domains as microVMs | `verse-session`'s T key starts an agent in a microVM with the project directory (virtiofs), scoped tokens and a network allowlist | An agent can't read `$HOME` or the key; its transcript still appears on the desk and in the Grid (V4) |
| I4 | Pylon jobs in their own domain | `openagents pylon` runs each job in a microVM with a GPU share and only the job's endpoints; Psionic in the session's process serves only you | A job can't read the session's memory, files or keys; the V5 idle policy still starts and stops it |
| I5 | Grants in the host description | Add domain grants (files, network, secrets, GPU, signer policy) to the host description with `#forall` and `#authority` checks; Nix emits VM configs, seccomp and Landlock from the JSON | Mutation gate at 0 survivors; the image builds from the JSON; removing a grant in Coax removes it on the machine |
| I6 | Shrink the host image | Remove setuid and setgid binaries, sshd and listening services; turn off unprivileged user namespaces; turn on lockdown; set `NoNewPrivileges` everywhere | A test in `os/nix/tests` checks each item against the built image |
| I7 | Guest windows as panels | A guest program's window reaches the session over virtio-gpu cross-domain and draws as a panel (V3), with keys and the pointer | A terminal in an agent's VM shows on the desk and in the Grid at the desk's frame rate |
| I8 | Isolation in the boot sequence and the world | Boot lines for every domain, the signer and the host checks; enclosures around agents; boundary requests as in-world approvals | A failing check shows red with its reason; an agent's pay request shows as an approval |
| I9 | A Coax virtio device model | One device (virtio-vsock or virtio-console) in a restricted Coax profile beside an existing VMM; bounded parsing; `#authority`, mutation gate and `coax deps --effects` showing no foreign calls | It passes the VMM's device tests and a fuzz run with no crash |
| I10 | A Coax BPF profile for BPF-LSM | Extend the audit's BPF profile idea to LSM hooks generated from the host description | A denied file open from an agent domain's host process is refused by the hook; the verifier accepts the program |
| I11 | Study: seL4 under the session | Write down what a freestanding Coax profile needs, how the GPU stays in a Linux guest, and which components move to native first | A doc in `docs/os/` with a go or no-go and its reasons |

I0 comes first because its answers decide how urgent I3 and I4 are. I1 and
I2 come before V2's wallet ships.
