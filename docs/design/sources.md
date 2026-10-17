# Design source notes

Reviewed 2026-10-17. These notes separate observed reference details from
OpenAgents recommendations. No upstream code or artwork is copied here.

## WalletUI

Source: [bdgwallet/WalletUI](https://github.com/bdgwallet/WalletUI), revision
[`8b3aea4df7be648161ad013812bf62b1fb340c52`](https://github.com/bdgwallet/WalletUI/tree/8b3aea4df7be648161ad013812bf62b1fb340c52).
The repository is a fork of `reez/BitcoinUI`; the package and source directory
still use the BitcoinUI name. Its README calls it a work in progress. This
review reads source; it does not build or test the Swift package.

The table records implementation details, not recommended OpenAgents tokens.
All file links are pinned to the reviewed revision.

| Reference | Observed specifics | OpenAgents adaptation |
| --- | --- | --- |
| [TextStyles.swift](https://github.com/bdgwallet/WalletUI/blob/8b3aea4df7be648161ad013812bf62b1fb340c52/Sources/BitcoinUI/TextStyles.swift) | Five semibold title sizes: 36, 28, 24, 21, 18. Five regular body sizes: 24, 21, 18, 15, 13. Text switches between black and white by theme. | Use a small semantic type scale. These fixed sizes do not establish large-text support; verify scaling and wrapping in our adapters. |
| [ButtonStyles.swift](https://github.com/bdgwallet/WalletUI/blob/8b3aea4df7be648161ad013812bf62b1fb340c52/Sources/BitcoinUI/ButtonStyles.swift) | Filled, outlined, and plain styles. Defaults: width 315, height 48, corner radius 5. Enabled, disabled, and pressed treatments; pressed scale 0.95 with a 0.1-second animation. | Preserve action hierarchy and visible states. Adapt width to the container, verify contrast, and respect reduced motion rather than adopting these values as product defaults. |
| [AddressFormattedView.swift](https://github.com/bdgwallet/WalletUI/blob/8b3aea4df7be648161ad013812bf62b1fb340c52/Sources/BitcoinUI/AddressFormattedView.swift) | Four-character chunks, a three-column default, and monospaced text with alternating emphasis. | Group for visual checking, but copy the exact unformatted value. Allow the full destination to wrap and remain accessible. |
| [SeedPhraseView.swift](https://github.com/bdgwallet/WalletUI/blob/8b3aea4df7be648161ad013812bf62b1fb340c52/Sources/BitcoinUI/SeedPhraseView.swift) | Numbered word capsules; configurable columns and optional paging, with six words per page by default. | Keep numbering and reading order clear. Secret reveal, authentication, storage, and backup completion belong to our wallet flow, not this display component. |
| [QRCodeView.swift](https://github.com/bdgwallet/WalletUI/blob/8b3aea4df7be648161ad013812bf62b1fb340c52/Sources/BitcoinUI/QRCodeView.swift) | Encodes Bitcoin and Lightning URI prefixes or a supplied BIP21 URI. Uses no image interpolation and adds an interactive tilt effect. | Render the Rust wallet's exact request. Keep the payment QR steady, provide copy and share, and verify scanning at realistic sizes. A rendered QR does not establish protocol support. |
| [Utilities.swift](https://github.com/bdgwallet/WalletUI/blob/8b3aea4df7be648161ad013812bf62b1fb340c52/Sources/BitcoinUI/Utilities.swift) | Helpers return horizontal spacing 10 and vertical spacing 20. | Use consistent spacing roles; verify content density and touch targets in each shell. |

The [package manifest](https://github.com/bdgwallet/WalletUI/blob/8b3aea4df7be648161ad013812bf62b1fb340c52/Package.swift)
sets iOS 16, while the README says iOS 15. The
[test file](https://github.com/bdgwallet/WalletUI/blob/8b3aea4df7be648161ad013812bf62b1fb340c52/Tests/BitcoinUITests/BitcoinUITests.swift)
has an example test with no active assertion. Treat the project as design and
implementation reference, not evidence of wallet correctness or accessibility.

The [MIT license](https://github.com/bdgwallet/WalletUI/blob/8b3aea4df7be648161ad013812bf62b1fb340c52/LICENSE)
names Matthew Ramsden, 2021. If a later change copies code or substantial
portions, preserve that notice and license. Check artwork licenses separately.
Our product uses shared Rust behavior and Rust Native views; adding this Swift
package is not the integration plan.

## Bitcoin Design

Read the original pages for their illustrations and full rationale:

| Source | What this collection uses |
| --- | --- |
| [First use](https://bitcoin.design/guide/daily-spending-wallet/first-use/) | Flexible onboarding, clear responsibilities, funding guidance, and reminders |
| [Sending bitcoin](https://bitcoin.design/guide/daily-spending-wallet/sending/) | Payment review, understandable status, and useful failure handling |
| [Receiving bitcoin](https://bitcoin.design/guide/daily-spending-wallet/requesting/receiving/) | Incoming payment visibility, fees, and actions when receipt needs attention |
| [Accessibility](https://bitcoin.design/guide/designing-products/accessibility/) | Labels, reading order, input alternatives, and user preferences |

The first-use reference models a Lightning mobile wallet with automated
services. Its channel management, backups, and swaps are architectural choices,
not universal requirements. Our [wallet guidance](wallet.md) adapts the
experience to Spark and identifies recommendations separately.

## Figma reference

The owner's [Bitcoin Wallet UI Kit and Design System file](https://www.figma.com/design/SRuOBNle62nKXhGn5yy0nf/Bitcoin-Wallet-UI-Kit---Design-System--Community-?node-id=4349-64998)
was visually inspected on its **First use** page on 2026-10-17. The overview
shows onboarding screens, cover-screen states, and product introductions.
This was a canvas overview, not a measurement or export of every component.
The numeric details above come from WalletUI source, not inferred Figma tokens.

Use the file to compare screen composition. Record a page and frame when a
future design decision depends on a specific example. Check permissions and
asset licensing before copying or distributing exports.

## OpenAgents evidence

The [2026-10-17 Spark wallet audit](../audits/2026-10-17-spark-wallet.md)
compares our pinned SDK with upstream and distinguishes current app features,
retired features, and proposed work. It also lists owner decisions. Consult it
before using an SDK feature in a screen or promising recovery behavior.

The older [wallet design](../breez/wallet-design.md) provides background and
prior interaction decisions. Its implementation statements include the retired
phone app. In particular, do not reuse its seed-separation assumption or
“shipped” annotations as current evidence without checking the October audit
and the implementation being changed.
