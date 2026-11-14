# OpenAgents mobile account surface

This is application code delivered to Coax Native's WebKit/Wasm profile,
starting with the starter's shared components and browser ABI. `host.js`
only transports service requests; `app.coax` owns the UI and transitions.
The reusable starter itself remains in `coax/native/starter/`.

Email login calls the same `/api/auth/email/start` and `/verify` services as
openagents.com/login through the installed `AppAuth` adapter. The server
sends the email; the person enters its six digits. Native storage retains
access/refresh tokens and the device key. Only an opaque pending challenge
and the entered code pass through this screen; they are cleared after login,
logout or changing email. Account expiry and every new login erase the
previous chat list and its title bytes before a new account can load. Wrong
codes retain the challenge for retry; resend requests a new one. Requests
cannot overlap, and a network failure leaves a
manual retry. Restarting during login requires a new code.

After login, `GET /api/chat/history/v1` returns at most 60 saved conversations,
newest first, across the account's linked Nostr keys. It is a read-only
account route available to email L1 sessions. Its Coax policy checks the
principal and each owning account. Existing admin and conversation-writing
gates are unchanged. `chat-history-wire.coax` is the one shared JSON codec.
A new email account with no saved chats correctly shows an empty list.
Local development Coder/Claude sessions served at `/local/*` are separate;
those routes are not deployed on openagents.com or exposed to this app.
This change lists saved chats; opening a transcript and sending messages are
subsequent application work.

Build the Wasm with `coax/remote/remote.sh run -- coax/web/wasm.sh
apps/openagents-ios/home/app.coax apps/openagents-ios/home/face.c
apps/openagents-ios/Starter/build/app.wasm`, then follow the update publisher
in `coax/native/README.md`. For the installed build 1069, retain its runtime
`0495897de3936831430611873a4ded8d9b4bdec8a0ecb8c511b680fea3ba9450` and released
extras module. Native changes on main already produce a different fingerprint;
do not regenerate the old runtime from today's native sources.

The API's auth suite verifies delivered-code login, account isolation and
unchanged admin/write refusal. `EmailLoginUITests` uses an independent fake
mail/API server (`Tests/email_fixture.py`) with a signed package and the retained
1069 simulator executable: real update verification, native device key,
Keychain persistence, wrong code, resend, chat list, relaunch and logout.
Fixtures do not send mail or access a real account or wallet.
