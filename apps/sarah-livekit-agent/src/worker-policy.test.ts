import { readFile } from "node:fs/promises";
import { describe, expect, test } from "vite-plus/test";
import {
  SARAH_LIVEKIT_MODEL,
  SARAH_LIVEKIT_TRANSCRIPTION_MODEL,
  SARAH_LIVEKIT_VOICE,
} from "@openagentsinc/audio-contract";

describe("Sarah LiveKit worker policy", () => {
  test("pins one server turn owner and disables provider and worker reconnect", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    expect(source).toContain(`model: SARAH_LIVEKIT_MODEL`);
    expect(source).toContain(`voice: SARAH_LIVEKIT_VOICE`);
    expect(source).toContain(`model: SARAH_LIVEKIT_TRANSCRIPTION_MODEL`);
    expect(source).toContain(`type: "semantic_vad"`);
    expect(source).toContain(`eagerness: "high"`);
    expect(source.match(/maxRetry: 0/g)?.length).toBeGreaterThanOrEqual(2);
    expect(source).toContain("await model.disconnectCurrentProviderSession()");
    expect(SARAH_LIVEKIT_MODEL).toBe("gpt-realtime-2.1");
    expect(SARAH_LIVEKIT_VOICE).toBe("marin");
    expect(SARAH_LIVEKIT_TRANSCRIPTION_MODEL).toBe("gpt-4o-mini-transcribe");
  });

  test("requires the patched per-generation safety identifier header", async () => {
    const patch = await readFile(
      new URL("../../../patches/@livekit__agents-plugin-openai@1.6.0.patch", import.meta.url),
      "utf8",
    );
    expect(patch).toContain(`"OpenAI-Safety-Identifier"`);
    expect(patch).toContain(`safetyIdentifier`);
  });

  test("keeps Kubernetes termination above both worker shutdown bounds", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const manifest = await readFile(
      new URL(
        "../../../infra/livekit/production/resources/sarah-agent-runtime.yaml",
        import.meta.url,
      ),
      "utf8",
    );
    expect(source).toContain("drainTimeout: 30_000");
    expect(source).toContain("shutdownProcessTimeout: 45_000");
    expect(manifest).toContain("terminationGracePeriodSeconds: 120");
  });

  test("drains provider accounting before Sarah-initiated operator shutdown", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const coordinator = source.indexOf("shutdownOperation = closeAfterProviderAccounting(");
    const contextShutdown = source.indexOf("ctx.shutdown(", coordinator);
    const operatorStop = source.indexOf("result.stopReason !== undefined");
    const operatorRequest = source.indexOf("requestShutdown();", operatorStop);
    expect(coordinator).toBeGreaterThan(-1);
    expect(contextShutdown).toBeGreaterThan(coordinator);
    expect(operatorStop).toBeGreaterThan(-1);
    expect(operatorRequest).toBeGreaterThan(operatorStop);
  });

  test("forces participant leave and SDK-close-first accounting uncertain", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const closeListener = source.indexOf("session.on(AgentSessionEventTypes.Close");
    const expectedCloseGuard = source.indexOf("if (sarahCloseInProgress) return;", closeListener);
    const disconnect = source.indexOf("accounting.disconnect();", expectedCloseGuard);
    const participantLeft = source.indexOf('"participant_left"', disconnect);
    const shutdownRequest = source.indexOf("requestShutdown();", participantLeft);
    expect(closeListener).toBeGreaterThan(-1);
    expect(expectedCloseGuard).toBeGreaterThan(closeListener);
    expect(disconnect).toBeGreaterThan(expectedCloseGuard);
    expect(participantLeft).toBeGreaterThan(disconnect);
    expect(shutdownRequest).toBeGreaterThan(participantLeft);
    expect(source).toContain("if (shutdownOperation === undefined) accounting.disconnect();");
  });

  test("fails a provider mismatch before deferring a valid admission until session start", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const rejectMismatch = source.indexOf(
      'observation.state === "mismatch" || observation.state === "drift"',
    );
    const deferAdmission = source.indexOf("pendingProviderAdmission = admitted");
    expect(rejectMismatch).toBeGreaterThan(-1);
    expect(source.indexOf("pendingProviderAdmission = observation.admission")).toBeGreaterThan(
      rejectMismatch,
    );
    expect(deferAdmission).toBe(-1);
  });

  test("attests the server-confirmed prompt and exact serialized tool profile", async () => {
    const agentSource = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const generationSource = await readFile(new URL("./generation.ts", import.meta.url), "utf8");
    expect(agentSource).toContain("providerProfile: realtimeProviderProfile(instructions, tools)");
    expect(agentSource).toContain("llm.toJsonSchema(providerTool.parameters)");
    expect(agentSource).toContain('toolChoice: "auto"');
    expect(agentSource).toContain("expectedProviderProfile");
    expect(agentSource).toContain('"openai_client_event_queued"');
    expect(agentSource).toContain("providerAttestation.observeClientEvent");
    expect(agentSource).toContain("Date.now() + 10_000");
    expect(agentSource).toContain("speed: 1");
    expect(agentSource).toContain("tracing: null");
    expect(generationSource).toContain("session.instructions !== expected.instructions");
    expect(generationSource).toContain("session.tool_choice !== expected.toolChoice");
    expect(generationSource).toContain('session.max_output_tokens === "inf"');
    expect(generationSource).toContain("nullish(session.prompt)");
    expect(generationSource).toContain("nullish(session.tracing)");
    expect(generationSource).toContain("outputAudio.speed === 1");
    expect(generationSource).toContain("instructions: profile.instructions");
    expect(generationSource).toContain("canonicalJson(observed) === canonicalJson(expectedTools)");
    expect(generationSource).toContain('"startup_base"');
    expect(generationSource).toContain('"tool_choice_none"');
  });

  test("keeps participant media unsubscribed until provider admission is durable", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const connectWithoutMedia = source.indexOf("AutoSubscribe.SUBSCRIBE_NONE");
    const awaitProviderAdmission = source.indexOf("() => providerAdmission");
    const subscribeParticipant = source.indexOf("publication.setSubscribed(true)");
    expect(connectWithoutMedia).toBeGreaterThan(-1);
    expect(awaitProviderAdmission).toBeGreaterThan(connectWithoutMedia);
    expect(subscribeParticipant).toBeGreaterThan(awaitProviderAdmission);
    expect(source).toContain("providerAttestation.markDurable(admitted)");
    expect(source).toContain("publication.setSubscribed(false)");
    expect(source).toContain('observation.state === "drift"');
  });

  test("feeds only the authoritative floor holder and interrupts before a floor switch", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const eventInterrupt = source.indexOf("await applyInterruptSequence(result.interruptSequence)");
    const floorUpdate = source.indexOf(
      "floorParticipantRef = result.floorParticipantRef ?? null",
      eventInterrupt,
    );
    expect(eventInterrupt).toBeGreaterThan(-1);
    expect(floorUpdate).toBeGreaterThan(eventInterrupt);
    expect(source).toContain(
      'dispatch.roomContext.kind === "community" ? null : dispatch.participantRef',
    );
    expect(source).toContain("selectSarahFloorParticipant(session, participantRef)");
    expect(source).not.toContain("session?._roomIO?.setParticipant");
    expect(source).toContain(
      "participantRef !== null && remoteParticipant.identity === participantRef",
    );
    expect(source).toContain("publication.setSubscribed(false)");
  });

  test("publishes verified community presence before listening and expires it after projections", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const awaitProviderAdmission = source.indexOf("() => providerAdmission");
    const publishPresence = source.indexOf(
      'sarahPresenceTemplateFromLease(communityProjection.lease, "active")',
      awaitProviderAdmission,
    );
    const subscribeParticipant = source.indexOf("publication.setSubscribed(true)");
    const projectAssistant = source.indexOf("AgentSessionEventTypes.ConversationItemAdded");
    const expirePresence = source.indexOf('"inactive"');
    expect(publishPresence).toBeGreaterThan(awaitProviderAdmission);
    expect(subscribeParticipant).toBeGreaterThan(publishPresence);
    expect(projectAssistant).toBeGreaterThan(-1);
    expect(source).toContain('event.item.role !== "assistant"');
    expect(source).toContain('if (fence.settle("worker_error")) requestShutdown();');
    expect(expirePresence).toBeGreaterThan(-1);
    expect(source).toContain("() => projectionChain");
  });

  test("does not enable recording or log raw provider events", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    expect(source).toContain("record: false");
    expect(source).not.toMatch(/console\\.(log|info|debug|warn|error)/u);
    expect(source).not.toContain("logMetrics");
  });

  test("grants data publish only for ephemeral LiveKit transcription output", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    expect(source).toMatch(
      /new WorkerPermissions\(\s*true,\s*true,\s*true,\s*false,\s*\[TrackSource\.MICROPHONE\],\s*false,?\s*\)/u,
    );
    const patch = await readFile(
      new URL("../../../patches/@livekit__agents@1.6.0.patch", import.meta.url),
      "utf8",
    );
    expect(patch).toContain("canPublishSources: this.#opts.permissions.canPublishSources");
    expect(source).toContain("publishTranscription");
    expect(source).not.toContain(".publishData(");
    expect(source).toContain("stores no transcript");
  });

  test("keeps community tool-free and exposes only the bounded private command set", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    for (const toolName of [
      "editor_context_read",
      "editor_reveal_range",
      "editor_replace_selection",
      "editor_save_document",
      "start_agent_thread",
    ]) {
      expect(source).toContain(`name: "${toolName}"`);
    }
    expect(source).not.toContain(`name: "editor_open_path"`);
    expect(source).toContain(`profile.kind === "private_owner_v1"`);
    expect(source).toContain(`: []`);
    expect(source).toContain("no workspace discovery authority");
    expect(source).toContain("short outcome summary, not the file contents");
    expect(source).not.toContain("Read up to 500 lines");
  });
});
