import { randomUUID } from "expo-crypto"
import { File, Paths } from "expo-file-system"
import { openDatabaseSync } from "expo-sqlite"

import {
  openExpoKhalaSyncStore,
  type ExpoSqliteDatabase,
} from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import { fetchFleetRunClientProjection } from "@openagentsinc/khala-sync-client"
import { loadNativeSessionCredential } from "../auth/native-session-vault"
import {
  fetchMobileExecutionTargetCatalog,
  type MobileExecutionTargetCatalog,
} from "../coding/mobile-execution-targets"
import type { FullAutoRunProjectionResult } from "../full-auto/full-auto-run-projection"
import { fetchFullAutoRunMobileProjection } from "../full-auto/full-auto-run-projection-source"
import {
  makeFullAutoRunControlDispatcher,
  type FullAutoRunControlDispatchOutcome,
} from "../full-auto/full-auto-run-control-intent"
import type { FullAutoRunControlAction } from "@openagentsinc/khala-sync"
import {
  createAuthenticatedMobileRepositoryEnvironment,
  type MobileRepositoryEnvironmentPort,
} from "../coding/mobile-repository-environment-client"
import type { SarahPrincipalProjection } from "@openagentsinc/sarah"
import { fetchSarahPrincipal } from "../sarah/sarah-client"
import { fetchSarahSpeech } from "../sarah/sarah-speech-client"
import {
  fetchMobileManagedSandboxes,
  makeMobileManagedSandboxController,
  type MobileManagedSandboxControlAction,
  type MobileManagedSandboxControlResult,
  type MobileManagedSandboxSnapshot,
} from "../managed-sandbox/mobile-managed-sandbox"
import { openExpoMobileManagedSandboxOutbox } from "../managed-sandbox/expo-mobile-managed-sandbox-outbox"
import type { ManagedSandboxSupervisionProjection } from "@openagentsinc/managed-sandbox-contract"
import { openMobileSyncHostCore, type MobileSyncHost } from "./mobile-sync-host-core"

export type MobileNativeSyncHost = MobileSyncHost & Readonly<{
  connectStoredVerifiedSession: () => Promise<"connected" | "signed_out" | "unavailable">
  /** Public-safe target projection. Credential custody never leaves this host. */
  executionTargets: () => Promise<MobileExecutionTargetCatalog | null>
  fleetRuns: () => ReturnType<typeof fetchFleetRunClientProjection>
  /** Live `FullAutoRun` mobile projection (openagents #8982, consuming #8981
   * once it lands). Best-effort: falls back to `{ state: "unavailable" }`
   * until the real Desktop-published endpoint exists — see
   * `full-auto-run-projection-source.ts`. */
  fullAutoRun: () => Promise<FullAutoRunProjectionResult>
  /** MOB-FA-02 (#8994): dispatches a Pause/Resume/Stop control intent
   * against a Desktop-owned FullAutoRun and resolves once a durable
   * applied/rejected/pending outcome is known. */
  fullAutoControl: (input: Readonly<{
    runRef: string
    action: FullAutoRunControlAction
  }>) => Promise<FullAutoRunControlDispatchOutcome>
  /** Safe, generation-fenced managed-sandbox supervision. The host keeps
   * bearer credentials and the durable exactly-once outbox private. */
  managedSandboxes: () => Promise<MobileManagedSandboxSnapshot>
  managedSandboxControl: (input: Readonly<{
    projection: ManagedSandboxSupervisionProjection
    action: MobileManagedSandboxControlAction
  }>) => Promise<MobileManagedSandboxControlResult>
  repositoryEnvironment: () => Promise<MobileRepositoryEnvironmentPort | null>
  /** Stable owner-private Sarah identity. Token custody remains host-only. */
  sarah: () => Promise<SarahPrincipalProjection | null>
  /** Owner-private TTS transport. Auth stays inside the native sync host and
   * only a short-lived local audio file crosses into the playback host. */
  sarahSpeech: (input: Readonly<{
    threadRef: string
    messageRef: string
    text: string
  }>) => Promise<Readonly<
    | { state: "ready"; fileUri: string }
    | { state: "unauthorized" | "forbidden" | "too_long" | "unavailable"; message: string }
  >>
}>

export const OPENAGENTS_MOBILE_SYNC_DATABASE = "openagents-mobile-sync.sqlite"
export const OPENAGENTS_MOBILE_SYNC_BASE_URL = "https://openagents.com"

const openNativeStore = (databaseName: string) =>
  openExpoKhalaSyncStore(databaseName, name => {
    const database = openDatabaseSync(name)
    const adapter: ExpoSqliteDatabase = {
      execSync: sql => database.execSync(sql),
      runSync: (sql, ...params) => database.runSync(sql, ...params),
      getAllSync: <Row>(sql: string, ...params: ReadonlyArray<string | number>) =>
        database.getAllSync<Row>(sql, ...params),
      withTransactionSync: task => database.withTransactionSync(task),
      closeSync: () => database.closeSync(),
    }
    return adapter
  })

/** Open one host-owned local store and a host-only verified-session connector. */
export const openMobileSyncHost = (): MobileNativeSyncHost => {
  const host = openMobileSyncHostCore({
    databaseName: OPENAGENTS_MOBILE_SYNC_DATABASE,
    randomId: randomUUID,
    openStore: openNativeStore,
  })
  const managedSandboxOutbox = openExpoMobileManagedSandboxOutbox(OPENAGENTS_MOBILE_SYNC_DATABASE)
  return {
    ...host,
    close: () => {
      managedSandboxOutbox.close?.()
      host.close()
    },
    sarah: async () => {
      const credential = await loadNativeSessionCredential();
      if (credential === null || host.conversation() === null) return null;
      return fetchSarahPrincipal({
        baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
        accessToken: credential.accessToken,
      });
    },
    sarahSpeech: async input => {
      const credential = await loadNativeSessionCredential()
      if (credential === null || host.conversation() === null) {
        return { state: "unauthorized", message: "Sign in again to listen to Sarah." }
      }
      const result = await fetchSarahSpeech({
        baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
        accessToken: credential.accessToken,
        ...input,
      })
      if (result.state !== "ready") return result
      try {
        const file = new File(Paths.cache, `openagents-sarah-${randomUUID()}.mp3`)
        file.create()
        file.write(result.audio)
        return { state: "ready", fileUri: file.uri }
      } catch {
        return { state: "unavailable", message: "Sarah voice could not open on this device." }
      }
    },
    fleetRuns: async () => {
      const credential = await loadNativeSessionCredential()
      if (credential === null || host.conversation() === null) {
        return { state: "unauthorized" }
      }
      return fetchFleetRunClientProjection({
        baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
        accessToken: credential.accessToken,
      })
    },
    fullAutoRun: async () => {
      const credential = await loadNativeSessionCredential()
      if (credential === null || host.conversation() === null) {
        return { state: "unauthorized" }
      }
      return fetchFullAutoRunMobileProjection({
        baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
        accessToken: credential.accessToken,
      })
    },
    fullAutoControl: async input => {
      const credential = await loadNativeSessionCredential()
      if (credential === null || host.conversation() === null) {
        return { state: "unauthorized" }
      }
      // Built fresh per call: the credential is loaded just above and
      // stays valid for the duration of one dispatch-then-poll round trip
      // (at most a handful of seconds), so a plain closure over it is safe
      // without a separate refresh path.
      const dispatch = makeFullAutoRunControlDispatcher({
        baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
        accessToken: () => credential.accessToken,
      })
      return dispatch(input)
    },
    managedSandboxes: async () => {
      const credential = await loadNativeSessionCredential()
      if (credential === null || host.conversation() === null) {
        return { state: "unauthorized" }
      }
      const controller = makeMobileManagedSandboxController({
        baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
        accessToken: () => credential.accessToken,
        outbox: managedSandboxOutbox,
      })
      await controller.flush()
      return fetchMobileManagedSandboxes({
        baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
        accessToken: credential.accessToken,
      })
    },
    managedSandboxControl: async input => {
      const credential = await loadNativeSessionCredential()
      if (credential === null || host.conversation() === null) {
        return { state: "rejected", reasonRef: "reason.authentication_required" }
      }
      return makeMobileManagedSandboxController({
        baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
        accessToken: () => credential.accessToken,
        outbox: managedSandboxOutbox,
      }).request(input.projection, input.action)
    },
    executionTargets: async () => {
      try {
        const credential = await loadNativeSessionCredential()
        if (credential === null || host.conversation() === null) return null
        return await fetchMobileExecutionTargetCatalog({
          baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
          token: credential.accessToken,
        })
      } catch {
        return null
      }
    },
    repositoryEnvironment: async () => {
      try {
        const credential = await loadNativeSessionCredential()
        if (credential === null || host.conversation() === null) return null
        return createAuthenticatedMobileRepositoryEnvironment({
          baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
          accessToken: credential.accessToken,
        })
      } catch {
        return null
      }
    },
    connectStoredVerifiedSession: async () => {
      try {
        const credential = await loadNativeSessionCredential()
        if (credential === null) {
          host.disconnectAuthenticated()
          return "signed_out"
        }
        host.connectAuthenticated({
          verification:"server_verified",
          baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
          ownerUserId: credential.ownerUserId,
          authToken: () => credential.accessToken,
        })
        return "connected"
      } catch {
        host.disconnectAuthenticated()
        return "unavailable"
      }
    },
  }
}
