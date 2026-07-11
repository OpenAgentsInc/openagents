# OpenAgents mobile keyboard avoidance receipt

Date: 2026-07-10 CDT  
Surface: `apps/openagents-mobile` home composer

## Fixed behavior

- The React Native host now wraps the home surface in a
  `KeyboardAvoidingView`: iOS uses padding and Android uses height.
- A background press dismisses the keyboard.
- The Android fallback input uses `blurAndSubmit`; both its return-key and send
  button paths dismiss before dispatching the existing typed turn intent.
- The iOS SwiftUI glass composer uses the same React Native submit callback, so
  submit and New chat dismiss the process keyboard without creating another
  composer state model.

No native configuration or runtime fingerprint changed, so the fix remains
eligible for the existing owned OTA channel.

## Android release proof

The locally built release APK was installed and cold-launched on the Pixel 7
Android 15 arm64 AVD at 1080 × 2400.

- Before focus, composer bounds: `[170,2167][946,2279]`.
- With `mInputShown=true`, composer bounds: `[170,1284][946,1396]`; the full
  input remained visible above the keyboard.
- Return-key submit produced `mInputShown=false`, blurred the input, restored
  the composer to `[170,2167][938,2279]`, and dispatched the turn.
- Refocus followed by a background tap also produced `mInputShown=false`.

Pixel evidence:
[`receipts/2026-07-10-mobile-keyboard-avoidance-android.png`](./receipts/2026-07-10-mobile-keyboard-avoidance-android.png)

SHA-256:
`da538f266179101c0e9efa1867bb9a5846078406989f9848f45bf1cbc8708a37`

The screenshot contains only fixture text and public app chrome.
