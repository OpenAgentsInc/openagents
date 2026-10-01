# Phone chat is text only

Issue: [#10093](https://github.com/OpenAgentsInc/openagents/issues/10093).

`coder_tab::ATTACHMENTS_ENABLED` is `false`. On a fresh iPhone 17 Pro
simulator (iOS 26.5, created for this check and deleted afterwards),
`SharedContractsUITests` passed (3 tests). The launch handed the draft a PNG
through the `--coder-tap attach:` hook. No `coder-attach` control, `image:`
card, or notice appeared, and with only an image on the pasteboard the
composer offered no Paste. [The composer](composer-text-only.png) shows no
attach button above the message box.
