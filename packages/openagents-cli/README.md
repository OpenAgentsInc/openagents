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
```

Your OpenAgents namespace is your GitHub user or organization namespace. You
sign in with GitHub, and organization creation requires an active GitHub
membership that can create repositories.

`repo create --source <directory>` verifies the Git worktree and adds the
server-provided clone URL as a remote. It refuses to overwrite an unrelated
remote and prints the next `git push` command. The first release never pushes
automatically. While the server creates repository storage, the CLI writes the
current state and a five-second heartbeat to standard error.

`repo view` and `repo clone` accept `-R, --repo <owner>/<name>`. When you omit a
repository, the CLI infers it only from an exact `/<owner>/<repo>.git` URL
on the selected OpenAgents API origin.

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
