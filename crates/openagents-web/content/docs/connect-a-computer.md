# Connect a computer

When a computer is connected to your openagents.com account, its Coder
chats show on the website, in the left panel, where you can reply to them.

## Connect it to your account

1. Install Coder on the computer. On macOS and Linux:

   ```sh
   curl -fsSL https://openagents.com/cli/install.sh | bash
   ```

   On Windows, in PowerShell: `irm https://openagents.com/cli/install.ps1 | iex`
2. Run `coder login`. It shows a code.
3. Approve that code at [openagents.com/device](https://openagents.com/device).
4. In Coder, type `/sync on`.

Coder's chats on that computer then show in the website's left panel. A
reply you send from the site reaches Coder while it's open on that
computer. [Settings](https://openagents.com/settings) lists your
signed-in computers, each with **Remove**.

## Connect your phone

Get [OpenAgents for iPhone](/docs/iphone) on TestFlight at
[testflight.apple.com/join/dvQdns5B](https://testflight.apple.com/join/dvQdns5B).
Connect your phone to a computer once, and the phone can reach it from
anywhere: chat with it, run Coder in your projects, and open a terminal
on it.

1. Install Coder on the computer (above). It adds the `openagents`
   command.
2. Start the computer's host and leave it running:
   `openagents host serve --iroh --control`
3. In a second terminal, run `openagents connect invite`. It draws a QR
   code.
4. On your phone, open **Account**, then **Computers**, then **Connect a
   computer**, and scan the code.
5. Both the terminal and the phone say the computer is connected.

There is no key to copy, and you don't need Tailscale. The code works
once, for one phone, and stops working when the command ends.

## Can't scan?

Run `openagents connect invite --text` instead. It prints a code under
the QR code; paste it in the phone's **Paste a code**.

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

A connected phone has full access to that computer: it can chat through
it, run Coder in your projects, open a terminal, and see the phones
connected to it. To take a phone's access away, run
`openagents connect remove DEVICE` on the computer
(`openagents connect devices` lists them).

Next: [Manage computers](/docs/manage-computers).
