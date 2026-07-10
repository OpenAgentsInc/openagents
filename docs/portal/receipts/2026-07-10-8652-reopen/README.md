# #8652 reopen — /portal browser-proof receipts (2026-07-10)

Owner-reported P0: authenticated /portal rendered only "Your setup is being
prepared" with no account context. Root cause: demo engagement bound to
`arcadecd@gmail.com` while the owner's GitHub session (`github:14167547`,
AtlantisPleb) carries `chris@openagents.com`. Second bug found by the browser
gate: the store's SAFE_REF guard rejected `email:<address>` session user ids,
so email-login clients could never read even their own engagement.

All three states captured by `apps/openagents.com/workers/api/scripts/portal-browser-smoke.ts`
driving REAL headless-Chromium sessions against live production
(`https://openagents.com`, monolith revisions 00076-xvx/00077-jp7,
commits 41df5387e3 + 6d70f32ecf + 3874961a8c):

| Receipt | State | Proof |
| --- | --- | --- |
| `portal-logged-out.png` | logged out | login gate + "Log in with GitHub", no engagement/empty body |
| `portal-logged-in-empty.png` | real email-OTP login (`email:chris@openagents.com`), no engagement | "Signed in as chris@openagents.com", different-email guidance, "Sign out / switch account" |
| `portal-logged-in-engagement.png` | real email-OTP login, engagement bound | engagement header, honest KPI placeholders, 2 A/B pairs with Approve/Reject |

The engagement screenshot used a TEMPORARY prod engagement bound to the smoke
identity, deleted immediately after capture. The owner's demo engagement
`portal_engagement_bb838850…` is bound to `clientUserId=github:14167547`
(+ `clientEmail=chris@openagents.com`) and untouched by the smoke.
