# About OpenAgents and Coder

OpenAgents, Inc. builds Coder, a coding agent. You give Coder a task in plain
language, and it reads your code, runs commands, and edits files to complete
the task. Coder lives at `https://openagents.com`.

## What Coder includes

Every part of Coder uses the same OpenAgents account.

- **Coder Terminal** is the app you install with one command. It runs on your
  computer, in your working directory, and the command it installs is
  `coder`. See the [install page](install.md).
- **Coder Cloud** runs work on machines that OpenAgents hosts instead of on
  your computer. In Coder Terminal, `/cloud <directive>` orders a run: the
  service clones the repository your account works in into a container, runs
  the directive there, and delivers the commit it made. Coder Cloud requires
  Pro and credit.
- **Earn** is experimental in the 0.5.0 release. It offers your machine's
  spare capacity to the mesh, within the limits you set, and counts what the
  machine earns as credit on your Coder account. Run `coder earn on` to start
  it and `coder earn off` to stop it. Serving needs a local model engine on
  the machine, such as Ollama. The public board at `/earn` shows the mesh.
- **Coder Desktop** for macOS and **Coder Mobile** for iOS are distributed to
  testers. Neither has a public download on `openagents.com`.

## Sign in

An OpenAgents account signs in with GitHub. On the web, sign in at
`https://openagents.com`. The first time Coder Terminal opens, it shows a
sign-in card with a `[ log in ]` control that opens the sign-in in your
browser, and the terminal keeps the session under `~/.openagents/session`. An
agent on a machine with no browser can sign in with a code that a signed-in
person approves on `/settings`, or with a Coder API key in `CODER_TOKEN`.

## Plans, credit, and pricing

- **Plans.** An account is on the free plan or on Pro. Pro costs $20 a month.
  Upgrade from `/settings`. Pro adds the `pro` lane, more runs working at
  once, and Coder Cloud.
- **Credit.** Your balance is credit, kept on a ledger in cents. Add credit on
  `/settings` in amounts of $10, $25, $50, or $100. Stripe processes the
  payment and holds no balance.
- **Lanes.** A lane is a tier of service: `free`, `flash`, or `pro`. Choose
  one with `/lane` in Coder Terminal. An answer is charged from your credit at
  the list price for its tokens, and the `free` lane posts no charge.
- **Hosted machines.** A Coder Cloud machine costs $1.00 an hour, charged from
  your credit in steps of 36 seconds with a one-cent minimum, and a machine
  starts only when your balance can pay for it. `/settings` shows the hourly
  rate for runs on the fleet.
- **Your own door.** When you point Coder Terminal at a model door on your
  own machine with `/endpoint`, the session runs on the `local` lane, and the
  service bills nothing and never sees the call.

## Common questions

**Is Coder open source?**
No. The Coder source repository is closed source. The door specification
that Coder speaks to models through is public in the
`OpenAgentsInc/nitro` repository on GitHub.

**Which platforms does Coder run on?**
Coder Terminal runs on macOS, Linux, and Windows. On Linux, the installer
picks the build for glibc or musl. On macOS and Linux, install it with
`curl -fsSL https://openagents.com/releases/install-terminal.sh | sh`. On
Windows, run
`irm https://openagents.com/releases/install-terminal.ps1 | iex` in
PowerShell.

**What does Coder cost?**
The free plan costs nothing, and the `free` lane posts no charge. Pro costs
$20 a month. Answers on the paid lanes and time on hosted machines draw down
credit that you buy on `/settings`.

**Does Coder send my code anywhere?**
In Coder Terminal, the `shell` tool runs commands on your computer, as you,
with your credentials. What you type, and the command output the
conversation carries, goes to `openagents.com` and on to a model provider to
produce each answer. The `--offline` flag keeps the transcript on your
machine and sends nothing. A model door on your own machine keeps the call
away from the service. `/cloud` clones your repository into a container on a
hosted machine. When thread sync is on for your account, a session pushes its
thread to the service; `/sync` prints what it pushed, and `/sync off` stops
it. The privacy policy at `/privacy` describes how OpenAgents handles what you
send, and says how to ask OpenAgents not to use your content to improve its
products or to train models.

**How do I get help?**
Read the documentation at `/docs`. In Coder Terminal, `/help` lists the
commands and keys, and `/feedback` opens the forum at `/forum`. The blog at
`/blog` covers releases and changes.
