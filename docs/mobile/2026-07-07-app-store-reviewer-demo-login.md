# Khala Mobile — App Store reviewer demo login

Date: 2026-07-07
Surface: `clients/khala-mobile` (Expo React Native, bundle
`com.openagents.khala.mobile`)
Behavior contract: `khala_mobile.auth.demo_login_example_data.v1`

## What it is

App Store reviewers cannot complete real GitHub OAuth against a personal
account, so the sign-in screen has a hidden, offline demo mode that shows
realistic hardcoded example data on every screen.

- **Gesture:** a deliberate ~1 second **long-press** on the "Sign in with
  GitHub" button enters demo mode. A normal tap still starts real GitHub OAuth
  (unchanged). The long-press is discoverable only to a reviewer who was told
  about it (via the App Review notes below) — an accidental tap never triggers
  it.
- **Session:** demo mode establishes a synthetic in-app "reviewer" session with
  a fake sentinel token that no server accepts. It never hits GitHub OAuth or a
  live backend, so it always works offline and a reviewer never sees a loading,
  error, or unauthorized state.
- **Data:** every product data source (thread list, thread messages, credits
  balance + history, repo picker, settings, model config) serves hardcoded,
  public-safe example fixtures. The data is clearly generic — `demo-user`,
  `example-web-app`, an example $10.00 balance — with no real user accounts,
  repositories, tokens, or balances.
- A subtle "Demo mode — example data" banner shows on the thread list so it is
  honest that it is demo data. The reviewer can freely navigate every screen.

## App Store Connect — App Review Information → Notes

Paste this into the "Notes" field for the build under review:

> To sign in without a GitHub account: on the sign-in screen, press and hold
> ("long-press") the "Sign in with GitHub" button for about 1 second. This
> enters a demo mode signed in as a sample account with example data, so you can
> review every screen (chats, credits, repositories, settings) without
> completing GitHub sign-in. All data shown in demo mode is fictional example
> data. A normal short tap on the button performs real GitHub sign-in.

## Where it lives (implementation)

- Gesture: `clients/khala-mobile/src/components/sign-in-screen.tsx`
  (`onLongPress={enterDemoMode}`, `delayLongPress={1000}`).
- Session + `demoMode`/`enterDemoMode`:
  `clients/khala-mobile/src/auth/khala-auth-context.tsx`,
  `clients/khala-mobile/src/auth/khala-auth-context-value.ts`,
  `clients/khala-mobile/src/auth/khala-auth-state-machine.ts`.
- Fixtures: `clients/khala-mobile/src/demo/demo-fixtures.ts`,
  `clients/khala-mobile/src/demo/demo-sync-runtime.ts`.
- Data-source gates (screens unchanged):
  `clients/khala-mobile/src/sync/use-khala-sync-scope-entities.ts` (threads +
  messages), `clients/khala-mobile/src/sync/khala-mobile-sync-runtime-context.tsx`
  (offline runtime), and the mobile credits / repos / model-preference API
  clients in `clients/khala-mobile/src/sync/`.
- Oracle test: `clients/khala-mobile/tests/demo-login-mode.test.ts`.
