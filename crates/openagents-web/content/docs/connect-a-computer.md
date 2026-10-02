# Connect a computer

Connect your phone to your Mac once, and the phone can reach it from
anywhere: chat with it, run Coder in your projects, and open a terminal on
it.

## Scan the QR code

1. Open **OpenAgents** on your Mac. It shows a QR code. (Later, open
   **Phones and computers** and click **Connect another phone**.)
2. On your iPhone, point the **Camera** at the code and tap the OpenAgents
   banner. Or, in the app, open **Account**, then **Computers**, then
   **Connect a computer**, and scan the code.
3. Both screens say the computer is connected.

There is no account to sign in to, no key to copy, and you don't need
Tailscale.

The code changes every minute, works once, for one phone, and shows only
while the window is open on an unlocked screen.

## Can't scan?

Click **Can't scan? Copy a code instead** on the Mac, and paste the code
in the app (**Account → Computers → Add another way**).

## Near each other

When your phone and Mac are on the same Wi-Fi, the Mac can also show up
under **Nearby** in the app. Tap it, check that both screens show the same
six-digit number, and click **Connect** on the Mac.

## A computer without the Mac app

On Linux, or a computer without a screen, use OpenAgents Terminal:

- In OpenAgents Terminal, type `/connect` to draw the QR code as text.
- Or run `openagents connect invite` in a shell.
- For a computer you reach over SSH, run `openagents connect --ssh
  me@box` from a computer that has `openagents`. It installs and starts
  everything there and pairs the two.

See [The openagents command](/docs/cli).

## How the phone reaches the computer

The phone connects straight to the computer when it can, and through the
OpenAgents relay when it can't, so it works on the same Wi-Fi and on
mobile data. If you already use Tailscale, it can be another route; it's
never required. See [Manage computers](/docs/manage-computers).

## What a connected phone can do

A connected phone has full access to that computer: it can chat through
it, run Coder in your projects, open a terminal, and see the phones
connected to it. To take a phone's access away, click **Remove** next to it
in the Mac app's **Phones and computers**.

Next: [Manage computers](/docs/manage-computers).
