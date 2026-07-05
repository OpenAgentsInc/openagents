# Khala Mobile Maestro Launched-App Smoke Receipt

Date: 2026-07-05, America/Chicago

Issue: [#8456](https://github.com/OpenAgentsInc/openagents/issues/8456)

## Environment

- Repo commit before receipt work: `f223122d7c`
- App id: `com.openagents.khala.mobile`
- App name: Khala Code
- App version: `0.1.0`
- iOS build number: `6`
- Simulator: iPhone 17 Pro, iOS 26.5
- Simulator UDID: `2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA`
- Metro: local `expo start --dev-client --host localhost` serving on `localhost:8081`
- Maestro flow: `clients/khala-mobile/.maestro/flows/LaunchFallback.yaml`
- Local artifact directory: `~/.maestro/tests/2026-07-05_150650`

## Result

PASS.

The flow launched the installed app with clear local state and keychain, verified
the Khala Code shell rendered, verified the Tailnet auto-discovery/manual
fallback state rendered, tapped `Sign in manually instead`, verified the manual
form labels `Owner user id` and `OpenAgents token`, returned to Tailnet
auto-discovery, tapped `Retry`, and verified the shell remained visible.

## Public-Safe Boundary

This receipt intentionally records only public-safe metadata and visible labels.
It does not include tokens, credentials, chat bodies, raw sync rows, screenshots,
or private local machine data.

`SignedInThreadSmoke.yaml` was not run because this worktree did not have a
public-safe seeded owner/token/thread precondition. The broader
`khala_mobile.platform.launched_app_interaction_smoke.v1` contract therefore
remains pending for signed-in thread open/send coverage and Android launched APK
coverage.

## Command

```sh
export PATH="$PATH:$HOME/.maestro/bin:/opt/homebrew/opt/openjdk@17/bin"
export JAVA_HOME="/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home"
export MAESTRO_CLI_NO_ANALYTICS=1
export MAESTRO_APP_ID=com.openagents.khala.mobile

maestro test clients/khala-mobile/.maestro/flows/LaunchFallback.yaml
```

## Output Summary

```text
Running on iPhone 17 Pro - iOS 26.5 - 2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA
Assert that "Khala Code" is visible... COMPLETED
Assert that "Looking for a signed-in Mac on your Tailnet.*|No signed-in Mac found on your Tailnet.|Found Khala Code.*" is visible... COMPLETED
Tap on "Sign in manually instead"... COMPLETED
Assert that "Owner user id" is visible... COMPLETED
Assert that "OpenAgents token" is visible... COMPLETED
Tap on "Back to Tailnet auto-discovery"... COMPLETED
Tap on "Retry"... COMPLETED
Assert that "Khala Code" is visible... COMPLETED
```
