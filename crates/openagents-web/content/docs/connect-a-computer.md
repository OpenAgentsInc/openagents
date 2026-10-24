# Connect a computer

When a computer is connected to your openagents.com account, its Coder
chats show on the website, in the left panel, where you can reply to them.

## Connect it to your account

1. These steps apply to an existing installation of the earlier Coder app.
   For the current browser experience, [start on the web](/docs/download).
2. Run `coder login`. It shows a code.
3. Approve that code at [openagents.com/device](https://openagents.com/device).
4. In Coder, type `/sync on`.

Coder's chats on that computer then show in the website's left panel. A
reply you send from the site reaches Coder while it's open on that
computer. [Settings](https://openagents.com/settings) lists your
logged-in computers, each with **Remove**.

## Let an agent log in as you

An agent, a script, or a sandbox can log in without a code to approve. Make
a key at [Settings, Login keys for agents](https://openagents.com/settings/sign-in-keys),
copy it (it's shown once), and give it to the agent in one line:

```sh
OPENAGENTS_TOKEN=oa_sk_... coder
```

To save it on a computer instead, run this and paste the key when asked:

```sh
coder login --key
```

Anyone with the key can act as you, so keep it secret. Revoke it in the same
Settings page, or from a terminal with `openagents keys revoke NAME`
(`openagents keys list` shows your keys).

## Connect your phone

Connect [OpenAgents for iPhone](/docs/iphone) to a computer once, and the
phone can reach it from anywhere: chat with it, run coding work in your
projects there, and see its sessions.

1. Install Coder on the computer (above). It adds the `openagents`
   command.
2. Start the computer's host and leave it running:
   `openagents host serve --iroh --control`
3. In a second terminal, run `openagents connect invite`. It draws a QR
   code.
4. On your phone, open **Settings**, then **Connect a computer**, and
   scan the code.
5. The computer shows in the phone's **Devices**, and it joins your
   account and signs in as you.

There is no key to copy, and you don't need Tailscale. The code works
once, for one phone, and stops working when the command ends.

## Can't scan?

Run `openagents connect invite --text` instead. It prints a code under
the QR code. Copy it to your phone, then tap **Paste** at the top of the
phone's scanner.

## A computer you reach over SSH

From a computer that has `openagents`, run `openagents connect --ssh
me@box`. It installs and starts everything there and pairs the two. See
[The openagents command](/docs/cli).

## How the phone reaches the computer

The phone connects straight to the computer when it can, and through the
OpenAgents relay when it can't, so it works on the same Wi-Fi and on
mobile data. If you already use Tailscale, it can be another route; it's
never required. See [Manage computers](/docs/manage-computers).

## What a connected phone can do

A connected phone has full access to that computer. In the app, it chats
through it, runs coding work in your projects there, and shows its
sessions. To take a phone's access away, run
`openagents connect remove DEVICE` on the computer
(`openagents connect devices` lists them).

Next: [Manage computers](/docs/manage-computers).
