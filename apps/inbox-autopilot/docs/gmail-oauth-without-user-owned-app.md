# Gmail OAuth Without User-Owned OAuth App

**Short answer: Yes.** You can provide Gmail (and email automation) without asking users to create their own OAuth “Desktop app” in Google Cloud Console. Your app can use **one OAuth client that you own**; users only sign in with Google and grant permissions.

---

## How it works

1. **You** (e.g. Inbox Autopilot / OpenAgentsInc) create **one** OAuth 2.0 client in [Google Cloud Console](https://console.cloud.google.com/apis/credentials):
   - Application type: **Desktop app** (for a native/desktop app) or **Web application** (for a backend or web app).
   - Enable the APIs you need (e.g. **Gmail API**).
   - Configure the [OAuth consent screen](https://console.cloud.google.com/apis/credentials/consent) (app name, logo, scopes).

2. **Users** see a normal “Sign in with Google” flow (browser or in-app webview). They choose their Google account and grant the requested scopes (e.g. read/send Gmail). They **do not** create or manage any OAuth client or project.

3. Your app receives an authorization code, exchanges it for access + refresh tokens, and stores the refresh token (securely). From then on, your app uses the refresh token to get access tokens and call Gmail API (watch, read, send, etc.).

So the “OAuth app” is **yours**; users only authenticate and consent.

---

## Options for your app

| Approach | Who has the client secret? | Best for |
|--------|-----------------------------|----------|
| **Backend-held secret** | Your server holds the client ID + secret; user is redirected to your backend or a hosted auth page that does the code exchange. | Web or mobile app; most secure. |
| **Embedded client ID + secret (desktop)** | Client ID (and often secret) ship inside the desktop app. Google treats desktop app secrets as “not highly sensitive” (redirect is to localhost), but the secret can still be extracted. | Desktop CLI or native app where you accept that risk. |
| **Embedded client ID only (PKCE)** | You ship only the client ID; no secret. Use [PKCE](https://developers.google.com/identity/protocols/oauth2/native-app#creating-a-client-id) in the OAuth flow. Google supports this for native/installed apps. | Desktop or mobile when you don’t want to ship a secret. |

Recommendation: if you have a backend, do the token exchange there and never ship the client secret. If you’re desktop-only, use PKCE with a single client ID so users never need to create their own OAuth app.

---

## What you need in GCP (one-time, your project)

- A **Google Cloud project** (yours).
- **Gmail API** (and any other APIs) enabled.
- **OAuth consent screen** configured (app name, support email, scopes such as `https://www.googleapis.com/auth/gmail.modify`, etc.).
- **OAuth client** (Desktop or Web) created; use the client ID (and optionally secret) in your app or backend.

If the app is in **Testing** mode, only test users (up to 100 emails you add in the console) can sign in. For **Production** (any Google user), you must submit for [app verification](https://support.google.com/cloud/answer/9110914) when using sensitive/scoped Gmail access—Google will review your consent screen and use case.

---

## Why gog doesn’t do this

[gogcli](https://gogcli.sh) deliberately requires users to supply their own OAuth client JSON (`gog auth credentials <path>`). Their README says they do **not** ship a pre-configured client ID/secret so that:

- The consent screen shows the **user’s** (or their org’s) app name, not a third-party name.
- Quota and abuse are tied to the user’s project.
- They avoid Google’s verification process and test-user limits.

So “user-owned OAuth app” is a **design choice** by gog, not a technical requirement. For a product like Inbox Autopilot where you want a smooth “Sign in with Google” experience, using **your** single OAuth client is standard and supported.

---

## Summary

| Question | Answer |
|----------|--------|
| Can we avoid users creating their own OAuth desktop app? | **Yes.** |
| Who creates the OAuth client? | **You** (one client for your app). |
| What do users do? | Sign in with Google and grant scopes; no GCP or OAuth setup. |
| Safe to ship a client secret in a desktop app? | Possible but not ideal; prefer backend exchange or PKCE. |
| Gmail API (watch, read, send) still work? | **Yes;** same scopes and tokens, regardless of who owns the client. |

So you can get the Gmail functionality you need with a single, app-owned OAuth client and no per-user OAuth app setup.
