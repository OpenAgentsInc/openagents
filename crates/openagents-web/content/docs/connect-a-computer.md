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

The phone and Mac apps aren't published yet; build them from
[source](https://github.com/OpenAgentsInc/openagents). With both built,
connect your phone to your Mac once, and the phone can reach it from
anywhere: chat with it, run Coder in your projects, and open a terminal
on it.

1. Open [OpenAgents for Mac](/docs/mac). It shows a QR code. (Later, open
   **Phones and computers** and click **Connect another phone**.)
2. In [OpenAgents for iPhone](/docs/iphone), open **Account**, then
   **Computers**, then **Connect a computer**, and scan the code.
3. Both screens say the computer is connected.

There is no key to copy, and you don't need Tailscale.

The code changes every minute, works once, for one phone, and shows only
while the window is open on an unlocked screen.

## Can't scan?

Click **Can't scan? Copy a code instead** in
[OpenAgents for Mac](/docs/mac), and paste the code in
[OpenAgents for iPhone](/docs/iphone) (**Account → Computers → Add
another way**).

## Near each other

When your phone and Mac are on the same Wi-Fi, the Mac can also show up
under **Nearby** in [OpenAgents for iPhone](/docs/iphone). Tap it, check
that both screens show the same six-digit number, and click **Connect** on
the Mac.

## A computer without the Mac app

On Linux, or a computer without a screen, use the `openagents` command
that the [Coder installer](/download) puts on your computer:

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
in [OpenAgents for Mac](/docs/mac)'s **Phones and computers**, or run
`openagents connect remove DEVICE`.

Next: [Manage computers](/docs/manage-computers).
