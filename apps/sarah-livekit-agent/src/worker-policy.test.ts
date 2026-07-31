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
    expect(source).toContain("shutdownProcessTimeout: 35_000");
    expect(manifest).toContain("terminationGracePeriodSeconds: 90");
  });

  test("fails a provider mismatch before deferring a valid admission until session start", async () => {
    const source = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const rejectMismatch = source.indexOf("if (admitted === false)");
    const deferAdmission = source.indexOf("pendingProviderAdmission = admitted");
    expect(rejectMismatch).toBeGreaterThan(-1);
    expect(deferAdmission).toBeGreaterThan(rejectMismatch);
  });

  test("attests the server-confirmed prompt and exact serialized tool profile", async () => {
    const agentSource = await readFile(new URL("./agent.ts", import.meta.url), "utf8");
    const generationSource = await readFile(new URL("./generation.ts", import.meta.url), "utf8");
    expect(agentSource).toContain("providerProfile: realtimeProviderProfile(instructions, tools)");
    expect(agentSource).toContain("llm.toJsonSchema(providerTool.parameters)");
    expect(agentSource).toContain('toolChoice: "auto"');
    expect(agentSource).toContain("expectedProviderProfile");
    expect(agentSource).toContain("Date.now() + 10_000");
    expect(generationSource).toContain("session.instructions !== expected.instructions");
    expect(generationSource).toContain("session.tool_choice !== expected.toolChoice");
    expect(generationSource).toContain("instructions: profile.instructions");
    expect(generationSource).toContain("canonicalJson(observed) === canonicalJson(expectedTools)");
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
