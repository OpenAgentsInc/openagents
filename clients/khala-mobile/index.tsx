// Side-effect import FIRST: installs a Web Crypto shim (crypto.getRandomValues
// + crypto.randomUUID) backed by expo-crypto before any app code — Hermes has
// no globalThis.crypto, which crashed @tanstack/db's optimistic-mutation UUIDs.
// See src/native/install-web-crypto.ts.
import "./src/native/install-web-crypto"

import { registerRootComponent } from "expo"

import { App } from "./src/app"

registerRootComponent(App)
