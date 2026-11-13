# Security

[openagents.com/settings/security](https://openagents.com/settings/security) shows how you log in to
OpenAgents, and lets you change it. Log in first.

## Ways in

The page lists your emails, Nostr keys and passkeys, and says whether you have a recovery code.

- **ADD A PASSKEY.** Your device makes a passkey for openagents.com and unlocks it with your
  face, fingerprint or screen lock. It's named after its password manager ("iCloud Keychain",
  "Google Password Manager"); RENAME changes the name. SYNCED means your password manager keeps
  it on your other devices. An account holds up to 20 passkeys.
- **MAY BE COPIED.** A passkey's counter went backwards at a login, which can mean someone
  copied it. We refused that login and told you. Remove the passkey if you don't know why.
- **ADD A KEY.** Your browser's Nostr extension (NIP-07) signs a one-time challenge, and the key
  is added. A key that already has its own account can't be added: log in to that account
  instead.
- **REMOVE.** Removes an email, a key or a passkey, and ends every login made with it. You can't remove
  the last way in.
- **NEW RECOVERY CODE.** Makes a 24-word recovery code and shows it once. Write it down. Making a
  new code ends the old one. You can use the code to log in, or to confirm it's you, once; then
  you get a new one.

We send a notice to every email and key on your account for every change. A notice has a link
that freezes your account if the change wasn't you.

## Confirm it's you

Some changes need a passkey or a key, or your recovery code:

- If you logged in with email and your account has a passkey or a key, you need it (or the
  code) to change anything.
- Removing a way in, making a recovery code and admin changes need a passkey or a signature
  from the last 5 minutes.

The page asks you when it needs to: choose USE A PASSKEY or SIGN WITH YOUR KEY, or type your recovery code. Then it
makes the change you asked for.

## Recovery

If you lost your keys and your recovery code, log in with your email and choose ASK FOR
RECOVERY.

- Recovery waits 72 hours. The page shows when it finishes.
- We tell every email and key on the account at once. Any of them can cancel it: the link in
  the notice, a login with a key, or CANCEL RECOVERY on this page.
- After 72 hours, choose FINISH WITH A NEW KEY. The new key replaces the account's keys and
  passkeys, and every other login ends.

You can ask for recovery 3 times a week. Every limit is listed at [/docs/limits](/docs/limits).

## Freeze

If a notice tells you about a change you didn't make, choose FREEZE. It opens
[/settings/data](/settings/data/index.md), where FREEZE MY ACCOUNT ends every login and cancels
any recovery.
