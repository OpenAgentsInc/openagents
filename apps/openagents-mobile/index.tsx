// Must precede every other import: `src/app` pulls in the signing path, and a
// module that signs before Hermes has `crypto.getRandomValues` fails at the
// first relay challenge rather than at startup.
import "./src/crypto-random-values-polyfill"

import { registerRootComponent } from "expo"

import { App } from "./src/app"

registerRootComponent(App)
