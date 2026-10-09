# Environments, locally against real Boat

`/environments` on the web app: pick a GitHub repository and branch, watch a
setup agent work out how to install it on a Boat machine, answer its
questions, and save the checked result as an environment version. A saved
version runs Claude Code tasks on a fresh machine made from it.

## Run it

```sh
codex login                         # once; the setup agent uses this login
scripts/dev/environments-local.sh   # builds and starts the web app
open http://127.0.0.1:4300/environments
```

The script reads `~/work/.secrets/boat.env` (`BOAT_API_KEY`, optional
`BOAT_API_BASE`) without printing it, writes a private config to
`~/.openagents/environments/environments.json` (names only, no secret), and
starts `openagents-web --environments CONFIG` on `127.0.0.1:4300`.

Optional:

- `GH_TOKEN` (or a `gh` login): lists your repositories on the New
  environment page and lets setup machines fetch private repositories.
  Without it, paste a public `owner/name` or `https://github.com/...` URL.
- `ANTHROPIC_API_KEY`: turns on **Run Claude Code here** for saved
  environments (your own key; it is applied to each run's machine and never
  saved in an image).
- `OA_ENVIRONMENTS_TEMPLATE`: the Boat template setup and builder machines
  start from. By default the newest ready `oa-coder-runtime-*` template,
  which carries the Coder runtime and Claude Code (see
  [the Boat template runbook](../deployment/boat-template.md)). Claude Code
  runs need a runtime template, because a saved image is built on top of it.

## What happens

1. **New environment** resolves the branch to its latest commit through
   GitHub and records the environment.
2. The **setup agent** (`coder-environment-operator::agent`) opens a setup
   session on a dedicated Boat machine, checks out the exact commit, and
   runs a model turn loop on the Codex login (model `gpt-6.1-sol`). Its tools
   are the setup owner's: run a command, write the install recipe, run the
   install, declare checks, ask you a question, finish. Each step shows in
   the chat; a failed install shows its output, and the next recipe shows as
   a diff.
3. Messages you type reach the agent before its next step. When it asks a
   question, its machine stops until you answer.
4. **Finish** ends the setup machine, builds a clean image from the recipe
   on a fresh builder, then checks that image on another fresh machine (the
   declared checks, then the recipe again to show it changes nothing).
5. The **Save** card names the exact checked result; saving makes it the
   environment's selected version.
6. **Run Claude Code here** starts a Cloud job with the Claude Code engine
   pinned to that version: Boat boots exactly the saved image, and the task
   is told the repository is at `/home/user/repo`.

If a step stops, the chat says why and offers **Try again**, which starts a
new setup machine and keeps the conversation and the recipe. A message after
a result or a save starts a revision the same way.

## Where things live

Everything is under `~/.openagents/environments` (private): the owners'
records (`environments/`, `environment-setup/`, `environment-build/`,
`environment-verify/`, `environment-computers/`), the conversation and agent
state (`environment-studio/<env>/`), and Claude Code runs
(`environment-claude/<env>/`). Command output that names a configured
credential is redacted before it is kept. Restarting the web app picks up
setups that were moving.

The web app runs the environment owners itself. Do not also point
`openagents host serve --environment-owners` at the same state directory.

## Cost and cleanup

Each setup, build, and check machine is deleted when its step ends, a
setup that stops cancels its setup machine, and each Claude Code run deletes
its machine when it finishes or is stopped. Built images stay in Boat as
named snapshots (`oaenv-*`), including images whose fresh-machine check
failed; Boat allows 10 named snapshots per account, so delete the ones no
saved version uses. To see what is running: `openagents boat` or the Boat
console.

## Recipes and images

- The install recipe runs under `sh`; a `#!` line is ignored. The setup
  agent is told so.
- The fresh-machine check runs the declared checks, then runs the recipe a
  second time on the image; with `offline` checks that rerun has no network,
  so a recipe must skip work already done.
- The Coder runtime template carries a `~/.boxignore` that keeps `~/.cargo`,
  `~/.rustup`, and `~/.cache` out of Boat snapshots. The builder removes it
  before saving the image, so toolchains a recipe installs under home are
  kept.
- A failed check's output shows in the conversation and is handed to the
  agent on **Try again**.
