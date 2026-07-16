import { isAbsolute } from "node:path";

import { extractLeadingSemver } from "@openagentsinc/agent-client-protocol/profiles";

import { createCursorAcpPeerRuntime, probeCursorAcpExecutable } from "../src/index.ts";

if (process.env.CURSOR_ACP_LIVE_RUNTIME !== "1") {
  console.log(JSON.stringify({ result: "skipped", arm: "CURSOR_ACP_LIVE_RUNTIME=1" }, null, 2));
  process.exit(0);
}

const workspace = process.env.CURSOR_ACP_LIVE_WORKSPACE;
if (workspace === undefined || !isAbsolute(workspace))
  throw new Error("CURSOR_ACP_LIVE_WORKSPACE must be an explicit absolute disposable workspace");
const probe = await probeCursorAcpExecutable();
const peerVersion = extractLeadingSemver(probe.reportedVersion);
if (peerVersion === undefined) throw new Error("Cursor version is not classifiable");
const updateKinds = new Set<string>();
let assistantText = "";
const peer = await createCursorAcpPeerRuntime({
  cwd: workspace,
  probe,
  authorizeLogin: async () => "continue",
  onUpdate: (record) => {
    const update = record.update as { sessionUpdate?: unknown; content?: { text?: unknown } };
    if (typeof update.sessionUpdate === "string") updateKinds.add(update.sessionUpdate);
    if (update.sessionUpdate === "agent_message_chunk" && typeof update.content?.text === "string")
      assistantText += update.content.text;
  },
  requestTimeoutMs: 30_000,
});

try {
  const started = await peer.start();
  if (!started.ok) throw new Error(`start failed: ${started.reason}`);
  const attached = await peer.newSession({
    cwd: workspace,
    canonicalThreadSeed: "cursor-live-smoke",
  });
  if (!attached.ok) throw new Error(`new session failed: ${attached.reason}`);
  const prompted = await peer.prompt(attached.value.peerSessionId, [
    { type: "text", text: "Reply with exactly ACP_CURSOR_PONG. Do not use tools." },
  ]);
  if (!prompted.ok) throw new Error(`prompt failed: ${prompted.reason}`);
  if (prompted.value.terminal !== "completed" || assistantText.trim() !== "ACP_CURSOR_PONG")
    throw new Error("Cursor prompt did not produce the exact diagnostic sentinel");
  console.log(
    JSON.stringify(
      {
        proofClass: "diagnostic-live",
        claimAuthority: "none-do-not-use-as-compatibility-evidence",
        result: "pass",
        platform: probe.platform,
        command: ["agent", "acp"],
        reportedVersion: probe.reportedVersion,
        classifiedVersion: peerVersion,
        executableBasename: probe.realPath.split("/").at(-1),
        executableSha256: probe.sha256,
        installationClosureSha256: probe.closureSha256,
        schemaRelease: started.value.schemaRelease,
        protocolVersion: started.value.wireVersion,
        authMethodIds: started.value.authMethodIds,
        capabilities: started.value.capabilities,
        extensionMethods: started.value.extensionMethods,
        session: {
          modeIds: attached.value.modes?.availableModes.map((mode) => mode.id) ?? [],
          configOptionIds: attached.value.configOptions.map((option) => option.id),
          stopReason: prompted.value.stopReason,
          terminal: prompted.value.terminal,
          updateKinds: [...updateKinds].toSorted(),
          assistantTextRetained: false,
          assistantTextLength: assistantText.length,
        },
      },
      null,
      2,
    ),
  );
} finally {
  await peer.shutdown();
}
