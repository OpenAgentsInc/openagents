import type { TassadarDenseProgramFixture } from "./dense-weight-module-runtime.js"
import fixtureData from "../fixtures/tassadar-dense-weight-module-v1.json" with { type: "json" }

export const tassadarDenseProgramFixture: TassadarDenseProgramFixture =
  fixtureData as unknown as TassadarDenseProgramFixture

export const tassadarDenseWeightModuleDigest = "cfda0fe5dcf42e16db9e18696731427f0f30915fd3100d38da2dcc8411433e2c"
export const tassadarDenseWeightModuleTraceDigest = "2465d2c2af5077b4cf44c6eddbdc5aba2859029e30062f49a30e669acfc8e9d2"
