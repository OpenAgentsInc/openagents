# Your wallet

[openagents.com/settings/wallet](https://openagents.com/settings/wallet) shows your wallet's daily
limit and its backup. Log in first.

Your Bitcoin wallet lives in the OpenAgents app on your phone. Its keys never reach us.

## The daily limit

The limit is how many sats the app pays in any 24 hours before it asks you to confirm it's you
with your passkey or key. It starts at 20,000 sats. The page shows the limit and how much the last
24 hours counted.

A payment of more than 100,000 sats to somewhere you haven't paid before always asks first,
whatever the limit.

### Change the limit

Type a number of sats and press SET LIMIT.

- A lower limit counts at once.
- A higher limit asks you to confirm it's you, then waits 24 hours. We tell every email and key
  on your account. While it waits, the page shows the time left and CANCEL. Any login of your
  account can cancel it, and so can the link in the notice we send, even when you are logged out.

### Where the limit is enforced

The OpenAgents app enforces the limit. It asks us before each payment, and it doesn't pay past the
limit until you confirm it's you. When it can't reach us, it uses its own count of the last 24
hours, and it doesn't pay past the limit until it can ask.

We can't stop a payment. Your wallet's keys are on your phone, not with us. If someone has your
wallet's words, they can load the wallet in other software and spend from it, and the limit
doesn't apply there. Changed software on your phone could skip the check too, and so could we:
the app trusts our answer, so the limit doesn't protect you from us. We still can't move your
money, because we never have your wallet's keys. The limit protects
you from someone who has your unlocked phone or your login. It doesn't protect you from someone
who has your wallet's words.

If you think someone else is in your account, freeze it on
[/settings/data](/settings/data/index.md). Freezing ends every login. The app on a phone that
has had a daily limit doesn't pay while it is logged out, so it stops paying until someone logs
in again, and a frozen account can't.

## The backup

The app can back up your wallet so that you can bring it back on a new phone. To make one, open
the OpenAgents app on your phone and back up your wallet there.

The app encrypts the wallet on your phone and sends us only the encrypted copy. We store that copy
and can't open it. Two things open it:

- A passkey on your account. On a new phone, the passkey prompt that logs you in also opens the
  backup, so your wallet is back after one prompt. Your password manager syncs the passkey.
- Your wallet recovery code: 24 words. The app makes this code on your phone and shows it once.
  Write it down. The code is never sent to us, and this website never asks for it. If a page
  asks for your wallet recovery code, it isn't ours. This code isn't the recovery code for your
  account on [/settings/security](/settings/security/index.md): that one logs you in, and it
  can't open your wallet.

If you lose the passkey and the code, nobody can open the backup, and we can't help.

The page lists each way to open the backup, with its name. To read the encrypted copy, change it
or delete it, you confirm it's you first, and we tell every email and key on your account.

### When the backup's passkey was removed

A passkey row that says REMOVED FROM YOUR ACCOUNT is a passkey you removed on
[/settings/security](/settings/security/index.md). When every passkey of the backup is gone, the
page says so: a new phone can't open the backup with a passkey prompt any more, and only your
recovery code opens it.

Your wallet on your phone still works. Open the app and back up again. The new backup replaces
the old one and comes with a new recovery code.

### Delete the backup

Press DELETE BACKUP, then PRESS AGAIN TO DELETE, and confirm it's you. We delete the encrypted
copy. Your wallet on your phone isn't touched. Without a backup, a lost phone means a lost
wallet.

To see how you log in, go to [/settings/security](/settings/security/index.md).
