# OpenAgents CLI

`@openagentsinc/cli` is the Effect TypeScript command-line client for OpenAgents
repositories.

## Install

The package targets Node.js 24 or later.

```sh
npm install --global @openagentsinc/cli
```

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
flags take precedence over environment variables. Explicit API URLs must use
HTTPS, except for loopback development origins.

## Authenticate during the first release

Set `OPENAGENTS_TOKEN` to use a token without storing it:

```sh
export OPENAGENTS_TOKEN="..."
openagents auth status
```

`auth token-stdin` and `auth logout` define the persistent credential workflow,
but the distributed CLI refuses persistent storage until an approved OS-backed
credential adapter is available. The CLI does not fall back to a plaintext
production credential file.

## Manage repositories

```sh
openagents repo create --private my-project
openagents repo create --private acme/my-project
openagents repo import --private acme/existing-project
openagents repo list
openagents repo view acme/my-project
openagents repo clone acme/my-project
```

Add `--json` before a subcommand to return machine-readable output. The clone
command invokes `git` with an argument array and never puts a token in a URL or
process argument.
