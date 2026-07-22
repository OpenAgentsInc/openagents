import { OwnerLocalPortableCapabilityInstallationPort } from "@openagentsinc/khala-sync-server/portable-capability-installation-ports"
import { Effect } from "effect"

import type { PylonPortablePhaseContextAdmissionStore } from "./portable-phase-context-admission.js"
import type { PylonPortableSessionOperationLedger } from "./portable-session-operation-ledger.js"
import {
  makePylonOwnerLocalCapabilityTransportHandler,
  type PylonOwnerLocalCapabilityAuthority,
} from "./portable-owner-local-capability-transport.js"

export const PYLON_OWNER_LOCAL_CAPABILITY_INGRESS_ENV =
  "PYLON_OWNER_LOCAL_CAPABILITY_INGRESS" as const

export const isPylonOwnerLocalCapabilityIngressEnabled = (
  env: Readonly<Record<string, string | undefined>>,
): boolean => env[PYLON_OWNER_LOCAL_CAPABILITY_INGRESS_ENV] === "1"

export type PylonOwnerLocalCapabilityIngressConfig = Readonly<{
  bearerToken: string
  pylonHome: string
  pylonRef: string
  targetRef: string
  sessionRef: string
  ledger: PylonPortableSessionOperationLedger
  authorityStore: PylonPortablePhaseContextAdmissionStore
  targetBindingIsCurrent: () => boolean
}>

const exactConfiguredScope = (
  authority: PylonOwnerLocalCapabilityAuthority,
  config: PylonOwnerLocalCapabilityIngressConfig,
): boolean =>
  authority.pylonRef === config.pylonRef &&
  authority.targetRef === config.targetRef &&
  authority.sessionRef === config.sessionRef

/**
 * Makes the private capability route for one recovered owner-local target.
 * The installation port is constructed only after all durable authority and
 * current-binding checks pass, so refused requests cannot enter custody.
 */
export const makePylonOwnerLocalCapabilityIngress = (
  config: PylonOwnerLocalCapabilityIngressConfig,
): ((request: Request) => Promise<Response>) =>
  makePylonOwnerLocalCapabilityTransportHandler({
    bearerToken: config.bearerToken,
    authorize: async authority => {
      if (
        !exactConfiguredScope(authority, config) ||
        !config.targetBindingIsCurrent() ||
        !config.authorityStore.authorizesCapability(authority)
      ) return false
      try {
        const [binding, fence] = await Promise.all([
          Effect.runPromise(config.ledger.readControlBinding(authority.sessionRef)),
          Effect.runPromise(config.ledger.readSession(authority.sessionRef)),
        ])
        return binding.sessionRef === authority.sessionRef &&
          binding.attachmentRef === authority.attachmentRef &&
          binding.generation === authority.attachmentGeneration &&
          binding.state !== "cleaned" &&
          fence.sessionRef === authority.sessionRef &&
          fence.attachmentRef === authority.attachmentRef &&
          fence.generation === authority.attachmentGeneration &&
          (binding.state !== "accepting" || fence.acceptingWork)
      } catch {
        return false
      }
    },
    portForAuthority: authority => new OwnerLocalPortableCapabilityInstallationPort({
      pylonHome: config.pylonHome,
      ownerRef: authority.ownerRef,
      targetRef: authority.targetRef,
    }),
  })
