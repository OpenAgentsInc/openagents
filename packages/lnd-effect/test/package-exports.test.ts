import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import * as Root from "../src/index.js"
import * as Contracts from "../src/contracts/index.js"
import * as Errors from "../src/errors/index.js"
import * as Services from "../src/services/index.js"
import * as Layers from "../src/layers/index.js"
import * as Adapters from "../src/adapters/index.js"

describe("lnd-effect exports", () => {
  it.effect("exposes root and subpath entrypoints", () =>
    Effect.gen(function* () {
      expect(Root.decodeLndNodeInfo).toBeTypeOf("function")
      expect(Root.LndNodeService).toBe(Services.LndNodeService)
      expect(Root.LndContractDecodeError).toBe(Errors.LndContractDecodeError)
      expect(Root.LndNodeDefaultLayer).toBe(Layers.LndNodeDefaultLayer)
      expect(Root.makeLndDeterministicLayer).toBe(Adapters.makeLndDeterministicLayer)

      expect(Contracts.LndNodeInfo).toBeDefined()
      expect(Contracts.LndRpcRequest).toBeDefined()
      expect(Errors.LndServiceUnavailableError).toBeDefined()
      expect(Services.LndTransportService).toBeDefined()
      expect(Layers.LndNodeDefaultLayer).toBeDefined()
      expect(Adapters.makeLndNodeDeterministicLayer).toBeDefined()
    }),
  )
})
