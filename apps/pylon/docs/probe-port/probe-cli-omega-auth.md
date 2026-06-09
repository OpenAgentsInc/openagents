# Probe CLI Omega Account Commands

Date: 2026-06-07

Status: implemented contract slice for Probe issue #161.

## Commands

`probe omega link`

Links a local Probe runner identity to an Omega base URL. This command writes
runner identity state only. It does not write ChatGPT/OAuth credentials.

`probe auth accounts`

Lists Omega-connected ChatGPT/Codex accounts for the configured scope. Output
shows account refs, labels, status/health, and plan type. It does not print
public secret refs or raw credential material.

`probe auth add chatgpt`

Delegates account connection to Omega's ChatGPT/Codex device-login routes. The
CLI prints the OpenAI verification URL and user code, then reads the attempt
status from Omega. The resulting provider account remains in Omega.

## Runtime Package

The CLI lives in `packages/runtime/src/cli.ts` and is exposed as the `probe`
bin in `@openagents/probe-runtime`.

`packages/runtime/src/omega/account-client.ts` implements the account-client
boundary used by the CLI:

- `GET /api/provider-accounts`
- `POST /api/provider-accounts/chatgpt-codex/device-login/start`
- `GET /api/provider-accounts/chatgpt-codex/device-login/:attemptId`

## Tests

`packages/runtime/tests/cli.test.ts` covers:

- local Omega link state creation without credentials
- listing multiple accounts without secret refs
- fake ChatGPT device-login flow through Omega
