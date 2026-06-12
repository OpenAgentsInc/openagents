# Probe CLI OpenAgents product surface Account Commands

Date: 2026-06-07

Status: implemented contract slice for Probe issue #161.

## Commands

`probe openagents link`

Links a local Probe runner identity to an OpenAgents product surface base URL. This command writes
runner identity state only. It does not write ChatGPT/OAuth credentials.

`probe auth accounts`

Lists OpenAgents product surface-connected ChatGPT/Codex accounts for the configured scope. Output
shows account refs, labels, status/health, and plan type. It does not print
public secret refs or raw credential material.

`probe auth add chatgpt`

Delegates account connection to OpenAgents product surface's ChatGPT/Codex device-login routes. The
CLI prints the OpenAI verification URL and user code, then reads the attempt
status from OpenAgents product surface. The resulting provider account remains in OpenAgents product surface.

## Runtime Package

The CLI lives in `packages/runtime/src/cli.ts` and is exposed as the `probe`
bin in `@openagentsinc/probe-runtime`.

`packages/runtime/src/openagents/account-client.ts` implements the account-client
boundary used by the CLI:

- `GET /api/provider-accounts`
- `POST /api/provider-accounts/chatgpt-codex/device-login/start`
- `GET /api/provider-accounts/chatgpt-codex/device-login/:attemptId`

## Tests

`packages/runtime/tests/cli.test.ts` covers:

- local OpenAgents product surface link state creation without credentials
- listing multiple accounts without secret refs
- fake ChatGPT device-login flow through OpenAgents product surface
