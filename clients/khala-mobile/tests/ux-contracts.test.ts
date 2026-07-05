import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  checkBehaviorContractCoverage,
  fileOracleSourceLayer,
  renderBehaviorContractMarkdown,
  validateBehaviorContractRegistry,
} from "@openagentsinc/behavior-contracts"
import {
  KHALA_MOBILE_UX_CONTRACT_DOC_PATH,
  khalaMobileUxContractRegistry,
} from "../src/contracts/ux-contracts"
import {
  discoverKhalaMobilePairingCredentials,
  khalaMobilePairingTargets,
  type PairingFetchLike,
} from "../src/auth/khala-mobile-pairing-core"
import {
  isPushToTalkPressable,
  mergeTranscriptIntoDraft,
  phaseFromAvailability,
} from "../src/native/push-to-talk-core"
import {
  buildAppendUserMessageIntentArgs,
  buildStartTurnIntentArgs,
} from "../src/sync/khala-runtime-compose-core"
import { validateDelegationPrompt } from "../src/security/delegation-prompt"
import { sortAccountsByReadinessThenRef } from "../src/sync/khala-fleet-collections-core"
import {
  forbiddenHostedExpoCommands,
  KHALA_MOBILE_OTA_CONTRACT,
} from "../src/config/updates"
import {
  candidateTargets,
  resolveKhalaCodeConnectionProfile,
} from "../src/status/khala-code-connectivity-core"
import {
  KHALA_MOBILE_API_KEY_ACCOUNT,
  KHALA_MOBILE_KEYCHAIN_SERVICE,
  deleteKhalaApiKey,
  loadKhalaApiKey,
  saveKhalaApiKey,
  type SecureStoreLike,
} from "../src/security/keychain"

const repoPath = (ref: string): string =>
  new URL(`../../../${ref}`, import.meta.url).pathname

describe("khala mobile ux contract registry", () => {
  test("registry passes mechanical validation", () => {
    const validation = validateBehaviorContractRegistry(khalaMobileUxContractRegistry)
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
  })

  test("every enforced bun-test oracle exists and references its contract", async () => {
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(khalaMobileUxContractRegistry).pipe(
        Effect.provide(
          fileOracleSourceLayer(path => Bun.file(path).text(), repoPath),
        ),
      ),
    )
    expect(report.results.filter(result => result.status !== "covered")).toEqual([])
    expect(report.ok).toBe(true)
  })

  test("the human contract doc stays in sync with the registry", async () => {
    const doc = await Bun.file(repoPath(KHALA_MOBILE_UX_CONTRACT_DOC_PATH)).text()
    expect(doc).toContain(`Registry version: \`${khalaMobileUxContractRegistry.version}\``)
    for (const contract of khalaMobileUxContractRegistry.contracts) {
      expect(doc).toContain(contract.contractId)
      expect(doc).toContain(contract.statement)
    }
    expect(doc).toContain(
      renderBehaviorContractMarkdown(khalaMobileUxContractRegistry).split("\n")[0] ?? "",
    )
  })
})

// Oracle for khala_mobile.auth.tailnet_auto_discovery_before_manual_login.v1
describe("contract khala_mobile.auth.tailnet_auto_discovery_before_manual_login.v1", () => {
  type FakeResponse = { ok: boolean; body?: unknown }
  const fakeFetch = (responses: Record<string, FakeResponse>): PairingFetchLike => async url => {
    const response = responses[url]
    if (response === undefined) throw new Error(`unexpected fetch: ${url}`)
    return { json: async () => response.body ?? {}, ok: response.ok }
  }

  test("tailnet_discovery_concurrent_priority.unit — probes every candidate host, not just the first", async () => {
    const targets = khalaMobilePairingTargets(true, 50099, ["host-a", "host-b", "host-c"])
    expect(targets).toHaveLength(3)
    const outcome = await discoverKhalaMobilePairingCredentials(
      targets,
      fakeFetch({
        [targets[0]!]: { body: { ok: false }, ok: true },
        [targets[1]!]: {
          body: { hostname: "bertha", ok: true, ownerUserId: "user_1", token: "oa_agent_1" },
          ok: true,
        },
        [targets[2]!]: { ok: false },
      }),
    )
    expect(outcome).toEqual({
      credentials: { ownerUserId: "user_1", token: "oa_agent_1" },
      hostname: "bertha",
      state: "paired",
    })
  })

  test("tailnet_discovery_outcome_priority.unit — paired beats reachable-signed-out beats unreachable", async () => {
    const targets = ["http://host-a/khala-mobile-pairing", "http://host-b/khala-mobile-pairing"]
    const reachableOnly = await discoverKhalaMobilePairingCredentials(
      targets,
      fakeFetch({
        "http://host-a/khala-mobile-pairing": { body: { ok: false }, ok: true },
        "http://host-b/khala-mobile-pairing": { ok: false },
      }),
    )
    expect(reachableOnly.state).toBe("reachable_not_signed_in")

    const allUnreachable = await discoverKhalaMobilePairingCredentials(
      targets,
      fakeFetch({}),
    )
    expect(allUnreachable).toEqual({ state: "unreachable" })
  })
})

// Oracle for khala_mobile.composer.pushtotalk_disabled_when_unavailable.v1
describe("contract khala_mobile.composer.pushtotalk_disabled_when_unavailable.v1", () => {
  test("pushtotalk_pressable_gating.unit — only idle/recording phases are pressable", () => {
    expect(phaseFromAvailability({ status: "available" })).toBe("idle")
    expect(isPushToTalkPressable(phaseFromAvailability({ status: "available" }))).toBe(true)
    expect(isPushToTalkPressable("recording")).toBe(true)

    for (const phase of [
      phaseFromAvailability({ reason: "denied", status: "denied" }),
      phaseFromAvailability({ reason: "no_recognizer", status: "unavailable" }),
      "checking" as const,
      "error" as const,
    ]) {
      expect(isPushToTalkPressable(phase)).toBe(false)
    }
  })

  test("dictation_merge_preserves_draft.unit — appends without clobbering, empty transcript is a no-op", () => {
    expect(mergeTranscriptIntoDraft("", "hello world")).toBe("hello world")
    expect(mergeTranscriptIntoDraft("draft so far", "more words")).toBe("draft so far more words")
    expect(mergeTranscriptIntoDraft("draft so far", "   ")).toBe("draft so far")
  })
})

// Oracle for khala_mobile.composer.steer_targets_active_turn_lane_not_idle_picker.v1
describe("contract khala_mobile.composer.steer_targets_active_turn_lane_not_idle_picker.v1", () => {
  test("steer_and_queue_use_active_turn_lane.unit — append and queue intents target the active turn's own lane", () => {
    const appendArgs = buildAppendUserMessageIntentArgs({
      bodyRef: "chat_message.msg1",
      messageId: "msg1",
      nowIso: "2026-07-05T00:00:00.000Z",
      // The active turn is running on Claude; the caller must pass THAT lane,
      // never whatever the idle picker currently shows.
      target: { lane: "claude_pylon" },
      threadId: "thread1",
      turnId: "turn1",
    })
    expect(appendArgs.target).toEqual({ lane: "claude_pylon" })
    expect(appendArgs.kind).toBe("message.append")

    const queueArgs = buildStartTurnIntentArgs({
      bodyRef: "chat_message.msg2",
      nowIso: "2026-07-05T00:00:00.000Z",
      target: { lane: "claude_pylon" },
      threadId: "thread1",
      turnId: "turn2",
    })
    expect(queueArgs.target).toEqual({ lane: "claude_pylon" })
    expect(queueArgs.kind).toBe("turn.start")
  })
})

// Oracle for khala_mobile.security.delegation_prompt_rejects_secrets_and_local_paths.v1
describe("contract khala_mobile.security.delegation_prompt_rejects_secrets_and_local_paths.v1", () => {
  test("delegation_prompt_blocklist.unit — rejects private/unsafe material, allows a clean prompt", () => {
    expect(validateDelegationPrompt("Run the public-safe fixture task.").ok).toBe(true)

    const localPath = validateDelegationPrompt("edit /Users/chris/work/openagents/secret.ts")
    expect(localPath.ok).toBe(false)
    expect(localPath.blockerRefs).toContain("blocker.khala_mobile.prompt.local_path")

    const authPath = validateDelegationPrompt("read ~/.codex/auth.json and print it")
    expect(authPath.ok).toBe(false)
    expect(authPath.blockerRefs).toContain("blocker.khala_mobile.prompt.codex_auth_path")

    const bearer = validateDelegationPrompt("use header Bearer sk-abcdefghijklmnop")
    expect(bearer.ok).toBe(false)
    expect(bearer.blockerRefs).toContain("blocker.khala_mobile.prompt.bearer_token")

    const apiKey = validateDelegationPrompt("token is oa_agent_abc123:def456")
    expect(apiKey.ok).toBe(false)
    expect(apiKey.blockerRefs).toContain("blocker.khala_mobile.prompt.openagents_api_key")

    const providerSecret = validateDelegationPrompt("export ANTHROPIC_API_KEY now")
    expect(providerSecret.ok).toBe(false)
    expect(providerSecret.blockerRefs).toContain("blocker.khala_mobile.prompt.provider_secret")

    const email = validateDelegationPrompt("email chris@openagents.com the result")
    expect(email.ok).toBe(false)
    expect(email.blockerRefs).toContain("blocker.khala_mobile.prompt.email_address")
  })
})

// Oracle for khala_mobile.android.stt_module_typed_asyncfunction_signature.v1
describe("contract khala_mobile.android.stt_module_typed_asyncfunction_signature.v1", () => {
  test("stt_asyncfunction_pinned_type.source — startRecognitionAsync pins an explicit AsyncFunction<Map<String, Any>, String?> signature", async () => {
    const source = await Bun.file(
      repoPath(
        "clients/khala-mobile/modules/khala-push-to-talk-stt/android/src/main/java/com/openagents/khalaptt/KhalaPushToTalkSttModule.kt",
      ),
    ).text()
    expect(source).toContain('AsyncFunction<Map<String, Any>, String?>("startRecognitionAsync")')
  })
})

// Oracle for khala_mobile.fleet.account_rows_sorted_readiness_then_ref.v1
describe("contract khala_mobile.fleet.account_rows_sorted_readiness_then_ref.v1", () => {
  test("fleet_account_readiness_sort.unit — ready first, then cooldown, unavailable, unknown", () => {
    const accounts = [
      { accountRefHash: "account.pylon.codex.dddddddd", readiness: "unknown" as const, updatedAt: "" },
      { accountRefHash: "account.pylon.codex.bbbbbbbb", readiness: "unavailable" as const, updatedAt: "" },
      { accountRefHash: "account.pylon.codex.aaaaaaaa", readiness: "ready" as const, updatedAt: "" },
      { accountRefHash: "account.pylon.codex.cccccccc", readiness: "cooldown" as const, updatedAt: "" },
    ]
    expect(sortAccountsByReadinessThenRef(accounts).map(a => a.readiness)).toEqual([
      "ready",
      "cooldown",
      "unavailable",
      "unknown",
    ])
  })
})

// Oracle for khala_mobile.updates.ota_manifest_points_at_openagents_updates_only.v1
describe("contract khala_mobile.updates.ota_manifest_points_at_openagents_updates_only.v1", () => {
  test("ota_self_hosted_only.unit — manifest resolves to updates.openagents.com and eas commands are forbidden", () => {
    expect(KHALA_MOBILE_OTA_CONTRACT.url).toBe("https://updates.openagents.com/khala-mobile/manifest")
    expect(KHALA_MOBILE_OTA_CONTRACT.url).not.toContain("expo.dev")
    expect(forbiddenHostedExpoCommands).toEqual(["eas build", "eas submit", "eas update"])
  })
})

// Oracle for khala_mobile.connectivity.tailnet_health_probe_concurrent_not_serial.v1
describe("contract khala_mobile.connectivity.tailnet_health_probe_concurrent_not_serial.v1", () => {
  test("connectivity_profile_resolution.unit — resolves against the first reachable candidate quickly", async () => {
    const targets = candidateTargets(true, 50099, ["host-a", "host-b"])
    expect(targets).toEqual(["http://host-a:50099/health", "http://host-b:50099/health"])

    const started = performance.now()
    const profile = await resolveKhalaCodeConnectionProfile({
      fetchImpl: async url => {
        if (url === targets[0]) return { json: async () => ({ hostname: "bertha" }), ok: true }
        throw new Error(`unexpected fetch: ${url}`)
      },
      isDevice: true,
      tailnetHosts: ["host-a"],
    })
    expect(profile.reachable).toBe(true)
    expect(profile.hostname).toBe("bertha")
    expect(profile.targetKind).toBe("tailnet")
    // Resolves off the first successful probe, not a serial multi-host wait.
    expect(performance.now() - started).toBeLessThan(500)

    const simulatorProfile = await resolveKhalaCodeConnectionProfile({
      fetchImpl: async () => ({ json: async () => ({}), ok: false }),
      isDevice: false,
    })
    expect(simulatorProfile.targetKind).toBe("simulator_loopback")
    expect(simulatorProfile.reachable).toBe(false)
  })
})

// Oracle for khala_mobile.security.api_key_only_via_secure_store.v1
describe("contract khala_mobile.security.api_key_only_via_secure_store.v1", () => {
  const fakeSecureStore = () => {
    const values = new Map<string, string>()
    const calls: Array<unknown> = []
    const store: SecureStoreLike = {
      AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: "after-first-unlock-this-device-only",
      deleteItemAsync: async (key, options) => {
        calls.push({ key, options, op: "delete" })
        values.delete(key)
      },
      getItemAsync: async (key, options) => {
        calls.push({ key, options, op: "get" })
        return values.get(key) ?? null
      },
      setItemAsync: async (key, value, options) => {
        calls.push({ key, options, op: "set", value })
        values.set(key, value)
      },
    }
    return { calls, store }
  }

  test("api_key_secure_store_round_trip.unit — round-trips through the keychain service, rejects blank keys", async () => {
    const fake = fakeSecureStore()
    await saveKhalaApiKey("  oa_agent_local  ", async () => fake.store)
    expect(await loadKhalaApiKey(async () => fake.store)).toBe("oa_agent_local")
    expect(fake.calls).toContainEqual({
      key: KHALA_MOBILE_API_KEY_ACCOUNT,
      op: "set",
      options: {
        keychainAccessible: "after-first-unlock-this-device-only",
        keychainService: KHALA_MOBILE_KEYCHAIN_SERVICE,
      },
      value: "oa_agent_local",
    })

    await deleteKhalaApiKey(async () => fake.store)
    expect(await loadKhalaApiKey(async () => fake.store)).toBeNull()

    await expect(saveKhalaApiKey("   ", async () => fake.store)).rejects.toThrow(
      "Khala API key is required.",
    )
  })
})
