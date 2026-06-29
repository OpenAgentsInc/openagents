import type { TassadarLinkedDenseProgramFixture } from "./linked-dense-module-runtime.js"
import fixtureData from "../fixtures/tassadar-linked-dense-module-v1.json" with { type: "json" }

export const tassadarLinkedDenseProgramFixture: TassadarLinkedDenseProgramFixture =
  fixtureData as unknown as TassadarLinkedDenseProgramFixture

export const tassadarLinkedDenseModuleDigest = "cc1403674fc0d38892610d9e9c6c9230075494061f720c45bfa4f7b5a961756a"
export const tassadarLinkedDenseComposedTraceDigest = "0caa43ace27a5b86da14cfe037e65c30f250f0c0a0ac1c01f1fe3a3a45a230b2"
