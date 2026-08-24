# OpenAgents CLI

`@openagentsinc/cli` is the Effect TypeScript command-line client for OpenAgents
repositories.

## Install

The package targets Node.js 20 or later.

```sh
npm install --global @openagentsinc/cli
```

Run one command without a global installation:

```sh
npx --yes @openagentsinc/cli@latest --version
npx --yes @openagentsinc/cli@latest repo list
```

Pin the version for a reproducible run:

```sh
npx --yes @openagentsinc/cli@0.1.5 --version
```

Do not run `auth setup-git` through `npx`. That command saves a persistent Git
helper that calls `openagents`, but the temporary executable is unavailable
after `npx` exits. Install the CLI globally before you configure a local or
global Git helper.

## Select an API

The CLI uses `https://openagents.com` by default. Select a named profile or an
explicit API origin for staging and local development.

```sh
openagents --profile staging repo list
openagents --profile local repo list
openagents --api-url http://localhost:4000 repo list
```

Named profiles resolve to these origins:

- `production`: `https://openagents.com`
- `staging`: `https://staging.openagents.com`
- `local`: `http://localhost:4000`

You can also set `OPENAGENTS_PROFILE` or `OPENAGENTS_API_URL`. Command-line
flags take precedence over environment variables, and environment variables
take precedence over `~/.config/openagents/config.json`. The configuration file
accepts a `profile` or `api_url` field and never stores tokens. Explicit API
URLs must use HTTPS, except for loopback development origins.

## Sign in

Start the browser-assisted device flow:

```sh
openagents auth login
```

In an interactive terminal, the command prints a verification URL and user
code, opens your browser when the operating system supports it, and waits for
approval. If the browser does not open, use the printed URL. The CLI stores the
resulting `oa_pat_` token in your operating-system credential store. The CLI
uses macOS Keychain through `security` and Linux Secret Service through
`secret-tool`. It never writes a production credential to a plaintext file.

In a headless or noninteractive process, the command returns immediately with
the complete authorization URL, user code, and a resume command. An agent can
surface the URL and code without needing streaming shell output. After you
approve the request in any browser, the agent runs the resume command:

```sh
openagents auth login
# Show the printed URL and code to the user. After approval:
openagents auth login --resume
```

Use `--headless` to force the resumable flow in an interactive terminal. Use
the global `--json` flag when an agent needs structured output:

```sh
openagents --json auth login
openagents --json auth login --resume
```

The CLI stores the pending device request in a private, mode-`0600` local file.
It removes that request after successful authorization or when it detects that
the request expired. The agent sees the user code, but it never receives your
GitHub credential or the resulting OpenAgents token.

The same two-step flow works through `npx`:

```sh
npx --yes @openagentsinc/cli@latest --json auth login
npx --yes @openagentsinc/cli@latest --json auth login --resume
```

You can also read a token from standard input:

```sh
openagents auth login --token-stdin
```

Set `OPENAGENTS_TOKEN` to use a token without storing it:

```sh
export OPENAGENTS_TOKEN="..."
openagents auth status
```

`OPENAGENTS_TOKEN` must contain an OpenAgents user token that starts with
`oa_pat_`. `OPENAGENTS_AGENT_TOKEN` is an internal agent-runtime credential.
Repository endpoints do not accept it, so do not use it with this CLI.

Run `openagents auth status` to inspect the selected endpoint and credential
source. Run `openagents auth logout` to remove the stored credential for that
exact API origin.

## Pair a Computer

Pair a local Computer with the selected OpenAgents API:

```sh
openagents computer pair
```

The command prints a browser approval URL and a short code, then waits for the
owner to approve the pairing. The machine token is stored in the operating
system credential store under the selected endpoint. It is not written to the
configuration file, output, or local Computer journal. The poll secret stays
in memory for the exchange.

Use `openagents computer status` to inspect local policy and pairing state. When
a Computer credential exists, status verifies it against the server and reports
when the machine is no longer active. It preserves the local credential and
directs you to run `openagents computer logout` for cleanup. A temporary
network failure is reported separately from revocation. Production and staging
use separate credential entries.

## Call any endpoint

`openagents api` sends an authenticated request to any OpenAgents API route and
writes the response body to standard output as JSON. Use it for the routes that
have no dedicated command yet, and for scripting.

```sh
openagents api repos/OWNER/REPO/issues
openagents api -X POST -f title="It fails on Tuesdays" -f body="Steps to reproduce" repos/OWNER/REPO/issues
openagents api -X PATCH -f state=closed repos/OWNER/REPO/issues/41
openagents api repos/OWNER/REPO/issues | jq '.[].title'
```

A path without a leading slash resolves under the API base `/api/v3/`, so
`repos/OWNER/REPO/issues` and `/api/v3/repos/OWNER/REPO/issues` name the same
route. An absolute path must start with `/api/`, and a complete URL must match
the API origin you selected. The CLI refuses a path that would leave that
origin.

`-X, --method` accepts `GET`, `POST`, `PATCH`, `PUT`, and `DELETE`. Without it,
a request that carries a body is a `POST` and a request without one is a `GET`.

`-f, --field key=value` is repeatable and builds a JSON object. Every value is
sent as a JSON string, and the CLI never guesses the type a route wants. For
numbers, booleans, arrays, and nested objects, pass the whole body with
`--input`:

```sh
openagents api --input body.json repos/OWNER/REPO/issues
echo '{"labels":["bug"],"milestone":3}' | openagents api -X PATCH --input - repos/OWNER/REPO/issues/41
```

`--input` reads a file, or standard input when you pass `-`. `--field` and
`--input` are mutually exclusive, and the CLI refuses a command that uses both.

`-H, --header 'Name: value'` is repeatable. The CLI sets the authorization
header from your OpenAgents session, so a `--header authorization` is refused.

Standard output carries only the body of a successful response. A non-2xx
status is a failed command: the CLI writes the response body and the request id
to standard error and exits non-zero, with the exit code every other command
uses for that status. A network failure exits with the transport status
instead, so a script can tell a refused request from an unreachable server.

`--profile`, `--api-url`, and `--json` work the same way they do for every other
command. The body is JSON in both output modes; `--json` writes it on one line.

## Manage repositories

```sh
openagents repo create --private my-project
openagents repo create --private acme/my-project
openagents repo create --source . --remote openagents my-project
openagents repo import --private acme/existing-project
openagents repo list
openagents repo list --namespace acme --limit 50
openagents repo view acme/my-project
openagents repo clone acme/my-project
openagents repo delete acme/my-project --yes
```

Without `--public` or `--private`, an import keeps the source repository's GitHub
visibility.

Your OpenAgents namespace is your GitHub user or organization namespace. You
sign in with GitHub, and organization creation requires an active GitHub
membership that can create repositories.

`repo create --source <directory>` verifies the Git worktree and adds the
server-provided clone URL as a remote. It refuses to overwrite an unrelated
remote and prints the next `git push` command. The first release never pushes
automatically. While the server creates repository storage, the CLI writes the
current state and a five-second heartbeat to standard error.

`repo view` and `repo clone` accept `-R, --repo <owner>/<name>`. When you omit a
repository, the CLI infers it from this checkout's Git remotes.

A remote's name does not decide this; its URL does. The CLI reads every remote
and takes the first whose URL is an exact `/<owner>/<repo>.git` path on the
OpenAgents API origin in use, so a forge remote named `openagents` and one named
`origin` both work. A GitHub mirror is never inferred, whatever it is called,
because the forge is the authority for issues and projects and a mirror is not.
When more than one remote points at the forge, `origin` wins, then `openagents`,
then the rest in the order `git remote -v` lists them.

When no remote qualifies, the CLI names each remote it examined and why it was
rejected, and asks for `OWNER/REPO` instead.

`repo delete` permanently deletes a repository you own, including its Git
history, issues, projects, and import records. It accepts an explicit
`owner/name` or the same `-R, --repo` and remote inference as `repo view`.
You must pass `--yes` so an agent or script cannot delete a repository by
accident.

`repo import` takes a GitHub `owner/name`, imports into that same GitHub user or
organization namespace, records the accepted branch and tag snapshot, and
copies the tip of every accepted branch and tag with depth 1. The shallow
snapshot is the default so large repositories become usable without first
copying years of history. OpenAgents becomes the destination source of truth.
Later GitHub changes do not sync in either direction.

While the command waits, it writes import state transitions, attempt counts,
elapsed time, and a five-second heartbeat to standard error. The server streams
Git bundles through durable storage without retaining the complete bundle in
application memory. Use `--wait-timeout 0` to return after acceptance; the
server import continues.

## Delegate to child coding agents

One prompt, many child coding agents, each in its own process. A child runs
under a harness — `opencode` today — and the CLI reports what every one of them
did.

Inside `openagents coder`, delegation is a tool the model calls, so you ask for
it in the conversation and nothing else is required:

```text
> split this three ways: each agent surveys one package for dead exports
```

The model calls `delegate`, the interface lists each child with the tool it is
running, its tool and token counts, and its result, and `ctrl+x` stops every
running child. `/delegate [<n>x] <prompt>` still works when you want to start
children yourself without spending a turn.

Children run on the session's own thread grant, so there is no child model to
choose and no provider credential to install. The CLI holds the grant in the
parent process, opens a loopback gateway on `127.0.0.1`, and points the child
harness at it; the child never receives the grant token, and its spending is
the thread's spending, under the same server-side budget.

The children need the harness itself on `PATH`:

```sh
npm i -g opencode-ai
```

Headless, the same fleet runs without a session:

```sh
openagents delegate "Add a regression test for the retry path, then say done" \
  --agents 3 --concurrency 2
```

`--agents` is how many children run the prompt and `--concurrency` is how many
run at once; the rest queue, so a fan-out of thirty does not become thirty
processes. Add `--json` for the task records and outcomes as data. The exit code
is non-zero when any child did not finish. Each child's raw harness transcript
is kept as JSONL under `$TMPDIR/openagents-coder-delegations`.

When a child fails, the CLI reports what the child reported — the harness error
and its reference, the provider or proxy refusal with its status, a model the
harness does not have, or a missing executable — rather than an exit code.

To run children on a provider of your own instead of the thread grant, name it:
`--child-model`, `--child-command`, and `--child-config` fall back to
`OPENAGENTS_DELEGATE_MODEL`, `OPENAGENTS_DELEGATE_COMMAND`, and
`OPENAGENTS_DELEGATE_CONFIG`. The CLI never reads or stores a provider
credential: `--child-config` names a harness configuration file, which the CLI
passes to the child as `OPENCODE_CONFIG` and nothing else. A child approves its
own tool use, because a delegated child has nobody to ask; `--child-ask` stops
it at its first edit for a dry run.

## Manage issues

```sh
openagents issue list
openagents issue list --state all --label area:cli --limit 100
openagents issue list --blocked false --limit 50
openagents issue view 129 --comments
openagents issue create --title "It fails on Tuesdays" --body-file -
openagents issue comment 129 --body "Reproduced on staging."
openagents issue close 129 --comment "Shipped in 0.4.0."
openagents issue reopen 129
openagents issue label 129 --add agent-ready --remove needs-design
openagents issue assign 129 octavia
openagents issue unassign 129 octavia
openagents issue deps 129
openagents issue deps 129 --add 80 --remove 81
```

The list route holds 25 issues to a page and takes no page size, so
`issue list --limit 100` pages until it has the issues you asked for. The
human-facing table reports the API's own total, not the number of rows it
printed.

`issue create` takes `--body`, or `--body-file` with a path or `-` for standard
input. `--label` and `--assignee` are repeatable, and a label has to exist in
the repository already.

`issue close` and `issue reopen` send the state change and nothing else. With
`--comment`, the comment is posted first as its own request, so the issue text
is never rewritten by a state change.

`issue deps` reads, adds, and removes the prerequisite edges an orchestrator
polls to find unblocked work. With no flags it reports what blocks the issue and
what the issue blocks.

## Manage projects

```sh
openagents project list
openagents project list --archived
openagents project view 2
openagents project create --title "Issues and Projects delivery"
openagents project fields 2
openagents project items 2
openagents project item-add 2 --issue 129
openagents project item-set 2 175 --set Status=Done
openagents project item-move 2 175 --set Status="In Progress" --position 1
openagents project item-remove 2 175
```

Projects are repository-scoped, so every project command takes the same
`-R, --repo` and remote inference the issue commands take.

## Deploy the fleet (operators)

The `deploy` commands drive the operator-only fleet promotion API
(`/api/v3/admin/forge/targets`). They require an API token holding the
privileged `deployments:promote` scope, and the server additionally checks
that the account is a current operator on every request. `forge:write` cannot
promote, and neither can a Git credential or a browser session.

Sign in with the privileged scope through the same device flow. The server
mints the scope only for an operator account, shows it on the approval page,
and gives the credential a shorter lifetime than an ordinary token:

```sh
openagents --profile production auth login --scope deployments:promote
```

Promote an exact pushed commit. Every production input is explicit: the CLI
never resolves a branch name, never promotes the working tree, and never
assumes an environment. Print the value you reviewed with `git rev-parse HEAD`
and pass it whole:

```sh
openagents deploy promote \
  --repo openagents.com \
  --sha "$(git rev-parse HEAD)" \
  --environment production \
  --wait
```

A `202 Accepted` without `--wait` means the promotion was recorded, not that
production is live. `--wait` polls the status resource with bounded backoff
until the target reaches `live`, `failed`, `reverted`, or
`needs_rolling_replace`, and `--wait-timeout` bounds the watching (the target
keeps running when the CLI stops watching). Observe and resume at any time:

```sh
openagents deploy view <target-id> --wait
openagents deploy list --limit 10
```

For release automation, pass `--idempotency-key` with a key your pipeline
owns: retrying the same key with the same inputs returns the original target
instead of deploying twice, and different inputs under the same key are
refused with a conflict. Left out, the CLI generates a key once and reuses it
across its own automatic transport retries. The key is never printed. Guard
against racing operators with compare-and-set:

```sh
openagents deploy promote --repo openagents.com --sha <full-sha> \
  --environment production \
  --expected-current-target <current-target-id>
```

Exit codes keep the outcomes apart for scripts: authentication or operator
refusals exit `3`, an invalid or unknown commit exits `2`, a stale expected
target or idempotency conflict exits `5`, a target that reached `failed` or
`reverted` exits `17`, a poll that outlived `--wait-timeout` while the target
was still running exits `18`, and `needs_rolling_replace` exits `19`. A
transport failure exits `6` and never claims the deployment failed. `SIGINT`
stops local polling without touching the server-side target and prints the
`deploy view` command that resumes.

If a privileged credential leaks or an operator leaves, revoke the token in
the account's API token settings; the server also rechecks operator standing
on every request, so removing the account from the operator allowlist takes
effect immediately, before the token expires. During an incident, promote the
last known-good SHA from `deploy list` the same way — an exact SHA, an
explicit environment, and a fresh idempotency key.

Every issue and project command accepts `-R, --repo <owner>/<name>` and
otherwise infers the repository from the forge remote the way `repo view` does,
whatever that remote is named. Issue and project numbers are bare integers; a
leading `#` is accepted and never required.

Add `--json` before a subcommand to return machine-readable output. Add
`--no-color`, or set `NO_COLOR`, to disable ANSI output. The clone command
invokes `git` with an argument array and never puts a token in a URL or process
argument. `SIGINT` and `SIGTERM` cancel in-flight HTTP and Git child-process
work and exit with status `130`.

For standard Git commands, configure the origin-scoped credential helper in
the current repository. Install the CLI globally before you save this helper:

```sh
openagents auth setup-git --local
git push -u origin main
```

Global setup requires an interactive terminal and explicit confirmation:

```sh
openagents auth setup-git --global --yes
```
