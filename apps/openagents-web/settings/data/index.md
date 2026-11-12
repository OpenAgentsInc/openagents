# Your data

[openagents.com/settings/data](https://openagents.com/settings/data) lets you download your data,
freeze your account, and merge two accounts. Log in first.

## Freeze

If you think someone else is in your account, choose FREEZE MY ACCOUNT. You can do this from any
login, even an email-only one, and it happens at once:

- every login on the account ends;
- every agent pauses, and its keys stop working;
- nothing can be spent.

The link in any notice we send you freezes the account too.

To unfreeze, log in and choose UNFREEZE. This needs your key, passkey or recovery code from the
last 5 minutes. On an account with only email, it needs a new email code from the last 5
minutes. Your agents stay paused until you resume each one on Settings, Agents.

## Download your data

DOWNLOAD MY DATA gives you everything we hold about you as one JSON lines file: your account,
emails, keys, agents, realms, saves, chats, payments and your security log. The format is
described at [/docs/auth/data-and-backups](/docs/auth/data-and-backups). A frozen account can't
download; an account waiting to be deleted can.

## Merge two accounts

If you have two accounts, MERGE ANOTHER ACCOUNT makes them one. Log in to the account you keep,
then prove the other one within 10 minutes:

- sign with the other account's key; or
- if the other account has no key, passkey or recovery code, type a code we send to its email.

The other account's emails, keys, agents, realms and saves move to this one, and its security
log is kept with this account. If the other account has service credit, you choose whether it
moves too. You can't merge a frozen account, or one waiting to be deleted.

To delete your account, see [/settings/delete](/settings/delete/index.md).
