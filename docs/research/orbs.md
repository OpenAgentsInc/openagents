# Amp Orbs: architecture, supported devices, and how they work

**Amp Orbs are isolated cloud development machines for Amp’s coding agents.** An Orb gives an agent a repository checkout, development tools, a terminal, running applications, and optionally a graphical desktop. You interact with it through Amp’s web interface, native apps, or CLI; the work does not depend on your laptop remaining open. Amp announced Orbs on **June 30, 2026**. [AmpCode](https://ampcode.com/news/agents-in-orbs)

The important architectural distinction is that **the conversation, the AI model, the machine executing tools, and the device you use to supervise the work are separate things**. That separation enables cross-device control, parallel agents, persistent environments, and execution on either Amp’s infrastructure or your own machines. [AmpCode](https://ampcode.com/docs/threads)

This report reflects public documentation available **October 8, 2026**. It is an architectural analysis, not a hands-on benchmark or an audit of Amp’s private implementation.

## 1. The product model

Amp’s terminology becomes clearer when separated into four objects:

| Object | What it represents |
|---|---|
| **Thread** | A unit of work: conversation, tool calls, results, and associated changes. |
| **Project** | Repository selection and shared configuration for work on a codebase. |
| **Orb** | An Amp-managed cloud machine associated with an Orb thread. |
| **Runner** | An Amp process on a machine you provide, available to execute remotely initiated work. |

A thread can be viewed through different clients without changing its execution location. Using Amp from an iPhone does not mean the agent’s compiler, browser, or shell is running on the phone. Similarly, using the CLI does not necessarily mean execution is local: the CLI can launch work in an Orb or on a runner. [AmpCode](https://ampcode.com/docs/threads)

My characterization is: **an Orb is a managed development environment attached to an agent conversation—not merely a chat session with a remote terminal bolted on.** Its distinguishing features are the coordination of environment preparation, persistence, previews, human review, and autonomous follow-up work. [AmpCode](https://ampcode.com/what-are-orbs?utm_source=chatgpt.com)

## 2. Architecture: the major layers

A useful logical reconstruction is:

```text
Human interfaces
  Web UI | macOS app | iPhone/iPad app | CLI
                         |
                Thread coordination
           Authentication, permissions,
           conversation state, usage
                   /            \
          Model inference       Tool execution
          External providers    Orb / local / runner
                                      |
                         Repository and filesystem
                         Shell, builds, tests
                         Browsers and GUI programs
                         Project services
                                      |
                           Human review surfaces
                         Diffs | Terminal | Portal
                                Desktop
```

This is a logical map, not a claim about Amp’s exact internal service boundaries. The separation between clients, threads, inference, and executors is documented; the complete production deployment topology is not. [AmpCode](https://ampcode.com/security)

### The coordination layer

Amp documents a **multi-tenant service on Google Cloud**, with conversations stored in **PostgreSQL**. It handles authentication, workspaces, synchronization, and usage. Thread actors receive authorized, request-scoped inference credentials. Clients use secure WebSockets, including connections to `production.ampworkers.com`. **Model inference occurs at external providers, not on the Orb’s CPUs.** [AmpCode](https://ampcode.com/security)

### The execution layer: E2B microVMs

Amp uses **E2B** for Orb compute. E2B describes its sandbox runtime as built on **Firecracker**, with a separate kernel per sandbox and a specialized snapshot/memory layer. This is a stronger isolation boundary than an ordinary application process sharing your personal development environment. It is not, by itself, proof that every possible action taken inside the environment is safe. [E2B](https://e2b.dev/)

The distinction matters operationally: an agent can install dependencies, change files, or start processes without directly modifying your laptop. Credentials and external systems remain separate security concerns.

The documented default Orb environment is **Debian 12 on E2B in Google Cloud `us-west1`**. Included tools cover Git/tmux, Node/Bun, Python/uv, browser automation, and media utilities. **Docker is supported but not preinstalled**; Amp recommends installing it during setup and running its daemon as a supervised service. Linux FUSE mounts and Tailscale connections are also supported. [AmpCode](https://ampcode.com/docs/orbs/customizing)

### Not every tool necessarily runs in the repository VM

Amp’s current custom-agent API distinguishes between:

- **Executor tools**, dispatched to the attached machine at the repository checkout.
- **Server tools**, executed in a small, detached sandbox without a repository checkout.

That is an important qualification: “the agent has an Orb” does not imply that every component of its reasoning and every possible tool invocation runs inside that one VM. [AmpCode](https://ampcode.com/docs/plugin-api)

## 3. Supported devices and execution targets

There are two separate compatibility questions: **where you can control Amp**, and **where the software you are building can execute**.

### Devices used to control Orbs

| Device or operating system | Documented access |
|---|---|
| **Mac** | Browser, CLI, and native app. Native app requires **macOS 26 or later**. |
| **iPhone** | Native beta app requires **iOS 26 or later**; web access is also available. |
| **iPad** | Native beta app requires **iPadOS 26 or later**. |
| **Windows PC** | Browser; the CLI’s documented Windows route is **WSL**. |
| **Linux PC** | Browser and CLI. |
| **Android device** | Browser is the available route; I did not find a documented native Android app. |

The Apple applications are currently **beta software**. macOS uses a direct download, while iPhone and iPad distribution uses a public **TestFlight** beta; the documentation says an App Store release will follow. The native apps add notifications, dictation, sharing integrations, and iOS screen recording. [AmpCode](https://ampcode.com/docs/macos-and-ios)

Amp does not publish a complete browser-version or device-model certification matrix in these documents. In particular, the native Mac app’s operating-system requirement should not be confused with a minimum requirement for using Amp through a browser or CLI.

### What can actually run inside an Orb?

The managed execution environment is Linux. Amp documents graphical Linux applications, Chrome extensions, and Android emulators as use cases for its Orb desktop. It does **not** document the managed Orb as a macOS or Windows VM. General-purpose computer use is still described as early and unsupported, despite the availability of the desktop feature. [AmpCode](https://ampcode.com/docs/orbs/desktop)

For work requiring **Xcode, Apple signing, a GPU, or attached physical hardware**, Amp’s answer is a runner on an appropriate machine. Its engineering discussion explicitly distinguishes these from Orbs and says GPU Orbs were not yet available. Do not interpret the existing CPU sizes as including GPU acceleration. [AmpCode](https://ampcode.com/podcast/season-02/episode-05?utm_source=chatgpt.com)

For example, from a suitable Mac checkout:

```bash
amp --no-tui --runner-id build-mac --remote-control-terminal
```

Then another client can start work on it:

```bash
amp -x "Build the app and run its tests" \
  --executor runner:build-mac
```

The runner executes under the account that launched it. A shared runner is therefore a grant of access to that account’s machine capabilities—not an automatically isolated replacement for an Orb. [AmpCode](https://ampcode.com/docs/cli/runners)

The native Mac app can also manage a runner without leaving a terminal open. It starts the CLI process, exposes selected checkout directories, and manages its lifecycle; quitting the app stops that managed runner. [AmpCode](https://ampcode.com/docs/macos-and-ios/runner)

**Bringing a runner does not self-host Amp’s coordination service.** Amp currently says it does not offer self-hosted deployments. [AmpCode](https://ampcode.com/security)

## 4. Environment preparation, startup, and persistence

### Preparing a repository

A project can contain an existing repository, including a private repository with appropriate authentication. Amp also supports starting an Orb without a project when a clean general-purpose environment is sufficient. [AmpCode](https://ampcode.com/docs/orbs/getting-started)

Two repository hooks establish repeatable preparation:

| File | Responsibility |
|---|---|
| `.agents/setup` | Install dependencies and prepare an environment reusable by new Orbs. |
| `.agents/resume` | Perform quick authentication, reconnection, or repair after activation and waking. |

Amp encourages having the agent prepare these files, test them, and commit their executable permissions. The practical objective is that a new agent receives a working environment rather than spending each task rediscovering how to boot the application. [AmpCode](https://ampcode.com/docs/orbs/getting-started)

### The startup sequence

Startup checks for a prepared snapshot for the **project and machine size**. A matching snapshot skips setup. Otherwise, Amp restores a base or older snapshot, runs pre-clone configuration, updates repositories, runs pre-setup and `.agents/setup`, and snapshots successful preparation. It then applies current credentials and runs `.agents/resume`. [AmpCode](https://ampcode.com/docs/orbs/customizing)

Matching project snapshots can be reused for **72 hours**. Changing `.agents/setup` alone does not invalidate one. Setup stops after **20 minutes**; failure permits startup but prevents publication of a refreshed snapshot. Resume blocks the agent for at most **10 seconds**, then continues without blocking it. Personal GitHub credentials are unavailable during shared setup, so user login state must not be baked into that snapshot. [AmpCode](https://ampcode.com/docs/orbs/customizing)

The engineering implication is that setup success and agent startup are different conditions. A thread starting successfully does not prove that all your development dependencies were installed correctly.

### Two different forms of persistence

A **project snapshot** is a reusable prepared starting point for new work. An **existing Orb’s paused state** preserves the particular environment in which a thread was already working.

E2B’s persistence documentation explains that pausing preserves filesystem and memory state, including running processes and in-memory variables. Resuming is therefore not equivalent to provisioning a blank machine and checking out the Git branch again. [E2B](https://e2b.dev/docs/sandbox/persistence)

This makes it possible to retain uncommitted work, installed tools, and application state across idle periods. It does not guarantee that external connections or expiring credentials remain valid; that is why a resume hook exists. [E2B](https://e2b.dev/docs/sandbox/persistence)

### How long does an Orb remain awake?

An Orb pauses when **both** conditions hold:

- The agent has performed no work for **five minutes**.
- There has been no human interaction for **20 minutes**.

A running test counts as agent work. An unattended development server alone does not keep the Orb awake. An authorized portal request wakes the Orb and renews the interaction window. Merely opening a thread normally wakes it too, unless **Orb Saver Mode** is enabled. Archiving pauses it immediately. [AmpCode](https://ampcode.com/docs/orbs/sizes-and-costs)

## 5. How the agent actually performs work

The benefit of the environment is the feedback loop it enables: inspect the repository, edit code, run commands, test the application, observe failures, and revise the implementation.

Amp’s own engineering write-up describes preparing its repository with a pinned toolchain, dependency installation, databases and seeded test users, plus instructions for exercising the application. This is a useful illustration of the practical work required: a VM is not enough; the agent needs reproducible setup and usable test fixtures. [AmpCode](https://ampcode.com/notes/putting-an-agent-in-an-orb)

Repository-level **`AGENTS.md`** files provide instructions about architecture, commands, conventions, and testing. Amp supports instructions at different directory scopes, so a monorepo can give the agent guidance specific to the package it is changing. [AmpCode](https://ampcode.com/docs/customize/agents-md?utm_source=chatgpt.com)

### Model choice is separate from machine size

Amp offers **low, medium, high, and ultra** modes, with different model/reasoning configurations. It also uses specialist subagents for functions such as searching, consultation, and delegated tasks. Increasing an Orb’s CPU or RAM allocation does not itself select a more capable language model; those are separate resource decisions. [AmpCode](https://ampcode.com/docs/models-and-subagents)

For example, a difficult architectural problem might require a stronger reasoning mode but little local compute. A straightforward migration across a large repository might require substantial test/build capacity without the same reasoning requirements.

### Persistent conversation does not mean unlimited model context

Amp’s May rebuild documentation describes automatic compaction at roughly **90% of the model’s context window**: accumulated context is summarized before continuing in a fresh window. Thus, a long-lived thread can retain its history while the model works from a compressed representation of older material. Files, tests, and explicit written decisions remain important durable records. [AmpCode](https://ampcode.com/news/neo)

### Human and agent share the working environment

The Orb terminal uses a shared **tmux** session. The agent and the person supervising it see the same filesystem and working copy; manual edits and agent edits are immediately visible to each other. Amp also exposes file browsing and diffs without requiring you to clone the repository locally. [AmpCode](https://ampcode.com/docs/orbs)

## 6. Portals and graphical desktops

These are distinct capabilities: **a portal exposes a web application**, while **the desktop exposes a graphical Linux session**.

### Portals: live applications rather than screenshots

A portal gives you an authenticated HTTPS entry point to an application running inside the Orb. You can interact with the real application and annotate its interface to send feedback to the agent. There is no separate preview deployment required for this workflow. [AmpCode](https://ampcode.com/news/portals?utm_source=chatgpt.com)

Long-running application processes are declared in `.amp/services.yaml`. For a Vite-style project whose `dev` script accepts these arguments, an illustrative configuration is:

```yaml
services:
  web:
    command: npm run dev -- --host 0.0.0.0 --port "$PORT"
    health: /
    portal: true
```

Then:

```bash
amp orb services ensure
```

Amp supplies the port and public origin, starts missing services, checks readiness, and returns the actual portal URL. A configured health endpoint must return HTTP 2xx or 3xx. Without a health endpoint, listening on the port is sufficient; a URL existing is not proof that the application is healthy. [AmpCode](https://ampcode.com/docs/orbs/portals)

Two limitations are especially relevant. Browser calls between separate portal origins encounter an authentication/CORS restriction; Amp recommends using a same-origin frontend proxy. Also, portal access control is separate from authentication inside your application. Public sharing must be explicitly enabled and can expire. [AmpCode](https://ampcode.com/docs/orbs/portals)

The architectural lesson is that environment preparation and service supervision are separate jobs. Do not treat a background process launched during dependency installation as a reliable service lifecycle.

### Desktop: browser-visible Linux GUI

An Orb can expose a graphical desktop with Chrome and other Linux applications. You can watch the agent, take over interactions such as login, and inspect screenshots or recordings. A separate `.agents/desktop/resume` hook can prepare the desktop when it starts. [AmpCode](https://ampcode.com/docs/orbs/desktop)

Amp documents its Linux desktop stack as **labwc plus Waymote**. Linux runners use a headless virtual desktop rather than capturing the user’s existing desktop; macOS runners instead share the Mac’s actual display and require Screen Recording and Accessibility permissions. Existing GNOME/KDE/X11 session sharing from Linux runners is not currently supported. [AmpCode](https://ampcode.com/docs/cli/runners)

Waymote’s public implementation provides unusually concrete transport details:

| Component | Function |
|---|---|
| `waymote-streamd` | Captures the Wayland display, handles virtual input, and supervises media encoding. |
| `waymote-gateway` | Provides the browser-facing HTTP/WebSocket interface. |
| Video/audio transport | H.264 video and Opus audio, consumed through browser WebCodecs over WebSockets. |
| Interaction | Keyboard, pointer, and clipboard traffic travel back toward the desktop. |

The underlying project uses local RTP between its components. These are details of the documented dependency, not a claim that Amp’s production build is an unmodified copy. In particular, it would be inaccurate to casually describe this as “just VNC” or assume the desktop transport is WebRTC. [GitHub](https://github.com/rockorager/waymote)

## 7. Parallel agents and extensibility

### Separate agents can have separate machines

Amp supports delegating work into additional threads, each with its own context and environment. Agents can send messages and results to one another. This enables parallel investigation or implementation without requiring every worker to modify one shared checkout. [AmpCode](https://ampcode.com/docs/orbs/agent-to-agent)

However, **sending a message is not the same as transferring files or merging commits**. Uncommitted changes in one Orb do not automatically appear in another. Coordination must deliberately move artifacts or integrate Git changes. Also, invoking a specialist subagent is not necessarily the same operation as allocating another Orb. [AmpCode](https://ampcode.com/docs/orbs/agent-to-agent)

Amp’s **Puck** interface serves as a coordinator: it can create work, monitor other threads, and send messages to agents. It is a supervisory interface, not another category of virtual machine. [AmpCode](https://ampcode.com/docs/puck)

### Plugins extend the runtime

Plugins are JavaScript/TypeScript modules that can register tools, react to lifecycle events and tool activity, add commands, and integrate other behavior. They can be associated with repositories or broader user/workspace configuration. Treat them as executable software you are trusting, not as harmless prompt text. [AmpCode](https://ampcode.com/docs/customize/plugins)

The API supports creating independent agent threads and targeting local, Orb, or runner execution. It also exposes a keep-alive lease through `amp.system.executor.keepAlive()`. Such a lease is **best effort**: it does not override manual pausing, exhausted credits, or infrastructure limits. [AmpCode](https://ampcode.com/docs/plugin-api)

### MCP integrations

Amp supports local and remote **Model Context Protocol** servers, including definitions shared through Amp’s settings. It recommends packaging tools into skills when they need not remain permanently loaded into model context.

For Orb-to-service authentication, remote HTTPS MCP definitions can use an Amp workload-identity token. Amp scopes the token to the server origin, handles refresh, and rejects redirects for that mechanism. The receiving service still needs to validate the token correctly. [AmpCode](https://ampcode.com/docs/customize/mcp)

## 8. Scheduled and event-driven work

### Scheduled automations

A thread can have a one-time or repeating schedule. When it becomes due, Amp wakes the agent with a trigger, and the agent retrieves the saved schedule instructions. The Orb resumes when execution is needed.

The documentation currently specifies **one schedule per thread**. A one-time task completes after execution; a failed automation pauses and requires correction and resumption. [AmpCode](https://ampcode.com/docs/orbs/automations)

Architecturally, this is different from relying on cron inside a VM that may be asleep. The scheduling mechanism can initiate work from outside the paused execution environment.

### Webhooks: durable intake outside the Orb

Plugin webhooks are especially revealing. They are **not portals exposing an HTTP server inside the VM**. Amp accepts and stores an event, returns HTTP 200, then wakes the owning Orb and invokes its handler. HTTP 200 means **queued**, not successfully processed. This mechanism is available for plugins in Amp-managed Orbs. [AmpCode](https://ampcode.com/docs/orbs/event-driven)

The documented guarantees and limits include:

| Property | Behavior |
|---|---|
| Delivery | At least once; handlers must deduplicate. |
| Ordering | Not guaranteed. |
| Handler deadline | 30 seconds. |
| Retry | Exponential backoff, starting at five seconds and capped at five minutes. |
| Persistent failure | Event dropped after one hour of handler failures. |
| Queue/body limits | 100 queued events; 1 MB request body. |

Generic webhook handlers must verify the sender’s signature themselves. Archiving the owning thread disables the endpoint and pauses delivery; an idle pause does not. Longer tasks should be handed to a durable queue or another thread rather than performed entirely inside the webhook deadline. [AmpCode](https://ampcode.com/docs/orbs/event-driven)

Amp separately documents an **experimental built-in GitHub automation integration**, for which Amp performs signature verification. That should not be confused with the responsibilities of a generic plugin webhook. [AmpCode](https://ampcode.com/docs/github)

## 9. Security, credentials, and retention

### Isolation does not eliminate authority risk

An isolated machine protects your laptop’s environment, but an agent can still act on whatever repositories, APIs, networks, or accounts you make accessible. My recommendation is to treat an Orb like a temporary developer workstation with explicitly limited credentials—not as a security boundary around the consequences of every command.

Amp supports secrets at personal, project, and workspace scope, with the more specific values taking precedence. It masks secret values in shell output and supports refreshing the executor and managed services after changes. Masking is an exposure-reduction measure, not a reason to grant unnecessarily broad access. [AmpCode](https://ampcode.com/docs/orbs/handling-secrets)

### Workload identity instead of permanent credentials

Orbs can mint signed **OIDC identity tokens** describing the relevant user, thread, project, or workspace. Tokens default to a **10-minute lifetime**, with documented configurable lifetimes from one minute to one hour. A service can exchange this identity for narrowly scoped access instead of requiring a permanent credential in the Orb. [AmpCode](https://ampcode.com/docs/orbs/handling-secrets)

The receiving service must validate the issuer, audience, expiry, and intended identity claims. Accepting any token issued by Amp would not establish that the request came from your project. Amp’s published examples use scoped identity to access services with per-user or per-thread attribution. [AmpCode](https://ampcode.com/news/secrets-of-the-orb?utm_source=chatgpt.com)

### Multiplayer materially changes the access boundary

**Enabling multiplayer gives workspace members access to the thread and Orb, including its terminal, files, and secrets.** It is not merely permission to comment on a diff. The thread owner remains responsible for billed usage. Shared-runner access also needs deliberate consideration because it can expose a real machine rather than an isolated Orb. [AmpCode](https://ampcode.com/docs/collaborate/multiplayer)

For review-only collaboration, granting access to a portal may be more appropriate than granting access to the whole development environment.

### Retention is not the same as pausing

The Orb contains the cloned repository; files read by tools can also enter conversation history. Paused memory/files remain encrypted until thread deletion. Orb deletion usually takes seconds; thread-data deletion can take up to 30 days. Superseded project snapshots may remain until dependent Orbs are deleted, so **72-hour snapshot reuse is not a 72-hour deletion guarantee**. Amp states it has SOC 2 Type II certification; I did not inspect its audit report. [AmpCode](https://ampcode.com/security)

Amp’s **Minimal Data Retention** policy concerns model-provider inference data, not deletion of your Orb or conversation. It includes safety, legal, and feature-specific exceptions. Customer-managed provider connections use that provider account’s retention terms. [AmpCode](https://ampcode.com/docs/enterprise/minimal-data-retention)

## 10. Git integration, review, and shipping

Amp’s GitHub integration combines your authorization with installation of its GitHub App on the relevant repositories. It supports cloning private repositories and performing repository operations with your access. **GitHub Enterprise Server is not currently supported** by the documented integration. [AmpCode](https://ampcode.com/docs/github)

The most consequential workflow default is **Ship**.

Amp documents the default shipping workflow as an instruction to the agent to commit, fetch and rebase onto the latest base branch, run tests, push to that base branch, and archive the thread. **Do not assume Ship means “open a pull request.”** It is an agent-driven workflow, not an atomic deployment primitive. [AmpCode](https://ampcode.com/docs/orbs/shipping)

You can instead configure **Push to Branch** or a custom shipping instruction. For a team adopting Orbs, my recommendation is branch-based shipping with repository-enforced review and CI requirements. That preserves the speed of the agent workflow without relying on a prompt as the only policy boundary. [AmpCode](https://ampcode.com/docs/orbs/shipping)

### A practical CLI workflow

For an existing project:

```bash
amp -ox \
  "Investigate the failing tests, fix the cause, and show test evidence. Do not push." \
  --project my-org/my-repo \
  --orb-size a1.medium
```

The command starts a cloud thread, prints its URL, and normally returns immediately. The agent continues independently of the terminal. `--stream-json` is available when an integration needs a stream of execution events instead. [AmpCode](https://ampcode.com/docs/cli/spawning-orbs)

After reviewing the result, you can mirror changes locally:

```bash
amp sync <thread>
```

This is an **Orb-to-local mirroring workflow**, not a promise of automatic two-way synchronization between every local edit and the remote checkout. [AmpCode](https://ampcode.com/docs/orbs)

## 11. Hardware sizes, pricing, and scaling limits

### Current managed Orb sizes

Every currently listed size includes **60 GB of disk**.

| Size | CPUs | RAM | Standard hourly price |
|---|---:|---:|---:|
| `a1.tiny` | 1 | 2 GB | $0.08 |
| `a1.small` | 2 | 4 GB | $0.17 |
| `a1.medium` | 4 | 8 GB | $0.33 |
| `a1.large` | 8 | 16 GB | $0.66 |
| `a1.xxlarge` | 16 | 32 GB | $1.32 |
| `a1.3xlarge` | 16 | 44 GB | $2.13 |

Usage is metered by the minute. The largest size is restricted to Gigawatt subscribers and Enterprise workspace members. These are the current specifications, rather than the smaller storage figures found in some launch-era material. [AmpCode](https://ampcode.com/docs/orbs/sizes-and-costs)

### Compute and inference are separate costs

Amp offers pay-as-you-go usage and subscription allowances. Its displayed **Megawatt plan is $20/month** and advertises **45,000 Orb minutes**, but larger machines consume the allowance faster. That headline is not 45,000 wall-clock minutes on every size.

Model and tool usage is accounted for separately. Supported bring-your-own-key or subscription routing can avoid Amp token fees, but the underlying provider’s charges or limits still apply. Using your own runner avoids managed Orb compute charges; your hardware remains your responsibility. [AmpCode](https://ampcode.com/docs/pricing)

For scale intuition, ten medium Orbs each running for one hour would total **$3.30 in standard compute charges**, before model/tool usage. Actual billed runtime also includes the period before automatic pausing. This is arithmetic from the published rate, not a measured workload estimate. [AmpCode](https://ampcode.com/docs/orbs/sizes-and-costs)

### Parallelism is paced

The documented default permits a user to start a burst of **20 metered Orbs**, followed by **one additional Orb every five minutes**. Further launches queue rather than immediately failing; higher limits require contacting Amp.

This is a launch-pacing policy, not a statement that only 20 Orbs can ever exist. It also means broad marketing language about spawning arbitrary numbers of agents should not be read as an unlimited instantaneous-capacity guarantee. [AmpCode](https://ampcode.com/docs/orbs)

## 12. What is not publicly established—and the overall assessment

The sources reviewed do not establish the exact production scheduler, host CPU models, storage IOPS, network throughput, snapshot internals, cold-start distribution, or a complete service-by-service deployment blueprint. E2B’s implementation descriptions are useful, but they are not end-to-end Amp performance benchmarks. Likewise, the presence of a long-lived portal should not be treated as a production-hosting reliability guarantee without appropriate terms. [E2B](https://e2b.dev/)

My assessment is that the strongest architectural choices are the **separation of thread from executor**, **prepared environments that can pause without being discarded**, and **review surfaces attached to the same place where the agent worked**. Those choices turn the result from “some generated code” into a package containing the conversation, changes, executable application, and evidence. [AmpCode](https://ampcode.com/docs/threads)

The principal engineering responsibilities do not disappear: preparing the repository, supplying representative test data, managing credentials, integrating parallel changes, and enforcing shipping policy remain essential. Amp’s own environment-preparation example makes that particularly clear. [AmpCode](https://ampcode.com/notes/putting-an-agent-in-an-orb)

**Bottom line:** Orbs are best understood as **persistent, remotely supervised workstations for coding agents**, built on E2B sandbox infrastructure and integrated with Amp’s conversation, automation, and review systems. The compelling feature is not the VM alone; it is that the agent’s workspace and the human’s ability to inspect, redirect, and verify its work remain connected across devices and across time. [AmpCode](https://ampcode.com/what-are-orbs?utm_source=chatgpt.com)

