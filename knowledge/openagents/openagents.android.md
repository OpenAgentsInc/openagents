---
id: openagents.android
version: 1
kind: product
title: "OpenAgents on Android"
summary: >-
  The Android app runs the same Rust library with the same tabs; at launch
  it's a signed APK checked on the emulator.
tags: [android, apk, platform]
applies_when: >-
  The user asks whether OpenAgents works on Android, what the Android app has,
  or how it differs from iPhone.
answer: >-
  The Android app runs the same Rust library as the iPhone app, with the same
  four tabs: chats with Coder and Computers, the Tailnet screen, the Grid, the
  Wallet with recovery words, trainer levels, and Report a problem. At launch
  it's a signed APK, checked on the Android emulator; checks on physical
  phones are still to come.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-android/README.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

The Android app runs the same Rust library as the iPhone app, with the same four tabs: chats with Coder and Computers, the Tailnet screen, the Grid, the Wallet with recovery words, trainer levels, and Report a problem. At launch it's a signed APK, checked on the Android emulator; checks on physical phones are still to come.

## Details

- The Android host is thin Kotlin over the same Rust views.
- The release APK is built for arm64-v8a.

## Sources

- `bins/openagents-android/README.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
