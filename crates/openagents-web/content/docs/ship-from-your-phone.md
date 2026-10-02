# Ship an iPhone build from your phone

You can ask Coder, from your phone, to build an iPhone app on your Mac and
upload it to TestFlight. Then you install the new build on the same phone.
This is how we ship OpenAgents for iPhone: build 44 was made and uploaded
from the phone. It works for your own app too, if its repository has a
release script and says how to use it.

## What you need on the Mac

- OpenAgents for Mac, with your phone [connected](/docs/connect-a-computer),
  the app's repository picked as the project, and **Let my phone start
  Coder here** on.
- A coding agent signed in. See [Coding agents](/docs/coding-agents).
- Xcode, with your Apple Distribution signing identity in the login
  keychain and the App Store profile installed.
- An App Store Connect API key in a file on the Mac. The OpenAgents
  repository reads it from `~/work/.secrets/appstoreconnect.env` by default
  (`ASC_API_KEY_ID`, `ASC_API_ISSUER_ID`, `ASC_API_PRIVATE_KEY_PATH`).
- Coder's default full access (`coder.access` set to `full`), so the build
  can use the keychain and Xcode.

## Tell Coder how to ship

Coder's agents read the repository's `AGENTS.md`. Say there how a release
works. The OpenAgents repository says, in short:

1. Raise the build number in `bins/openagents-ios/host/project.yml`.
2. Add that build's entry at the top of the app's changelog.
3. Commit and push to `main`.
4. Run `scripts/release/testflight.sh start`, then run
   `scripts/release/testflight.sh wait` again and again until it says it's
   done or failed. Each `wait` returns within a few minutes, so no single
   command runs too long.
5. Report the build number and the script's last line.

The script refuses a checkout with uncommitted changes and a build number
App Store Connect already has. `--validate-only` does a dry run: it builds
and validates the app without uploading.

## Ship

1. On your phone, open a chat and say what to ship: "Ship build 45 to
   TestFlight with the new chat menu in the changelog."
2. Coder starts on the Mac. Follow its steps in the chat; a release takes
   15 to 25 minutes.
3. When Coder reports the build is in TestFlight, open TestFlight on the
   phone and install it.
4. In the app, **Account → Changelog** shows the new build first.

If something fails, Coder reports the script's last line. Ask it to fix
the cause and try again in the same chat.

Next: [Connect a computer](/docs/connect-a-computer).
