import {
  TARGET_ADAPTER_SCHEMA_VERSION,
  type TargetAdapterContract,
} from "./target-adapter";

export const QS7_EXECUTOR_RUN_REF = "qa-run.executor.qs7-public-home" as const;

export type Qs7Verdict = "CONFIRMED" | "REFUTED" | "INCONCLUSIVE";

export interface Qs7EvalRow {
  readonly variant: string;
  readonly axis: string;
  readonly verdict: Qs7Verdict;
  readonly runtime: string;
  readonly notes: string;
}

export interface Qs7ExternalPrPacket {
  readonly runRef: string;
  readonly verdict: Qs7Verdict;
  readonly shareUrl: string;
  readonly browserRecording: string;
  readonly terminalRecording: string;
  readonly distilledTest: string;
  readonly trace: string;
  readonly chillEvalComparison: string;
  readonly verificationCommand: string;
  readonly evalRows: ReadonlyArray<Qs7EvalRow>;
}

const PRIVATE_OR_PLACEHOLDER_PATTERN =
  /(<[^>\n]+>|TODO|TBD|\/Users\/|\/home\/|\/tmp\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|oauth|provider[_-]?(credential|payload|secret|token)|raw[_-]?(artifact|log|prompt|trace)|secret|sk-[a-z0-9])/i;

const assertSafeField = (name: string, value: string): void => {
  if (PRIVATE_OR_PLACEHOLDER_PATTERN.test(value)) {
    throw new Error(`QS7 packet field "${name}" is not public-safe`);
  }
};

const row = (cells: ReadonlyArray<string>): string => `| ${cells.join(" | ")} |`;

export function rhysExecutorTargetAdapter(): TargetAdapterContract {
  return {
    schemaVersion: TARGET_ADAPTER_SCHEMA_VERSION,
    id: "executor-public-prod",
    displayName: "Executor public production",
    target: {
      name: "executor-public-prod",
      baseUrl: "https://executor.sh",
      environment: "prod",
      owner: "external",
      capabilities: ["browser", "terminal"],
      restrictions: ["read-only"],
    },
    auth: {
      kind: "none",
      freshIdentity: {
        required: false,
        strategy: "public landing page smoke only",
      },
    },
    restart: {
      kind: "none",
    },
    prodReadOnly: {
      policy: "read-only",
      allowedStepKinds: ["navigate", "wait-for", "screenshot", "assert"],
      blockedStepKinds: ["click", "type"],
      notes: "External production target; QS7 only observes public landing-page state.",
    },
    scenarioSeeds: [
      {
        id: "executor-public-home",
        title: "Executor public landing page renders",
        startPath: "/",
        commitment:
          'Confirm the public page renders "Connect any agent to" and describes Executor as an MCP gateway.',
      },
    ],
    checklist: [
      "Run only read-only browser steps against external production.",
      "Record browser video, terminal evidence, result.json, and trace refs.",
      "Include the executor-public-home chill-evals comparison.",
      "Hold the external PR until owner sign-off approves media and wording.",
    ],
  };
}

export function assertQs7ExternalPrPacketPublicSafe(packet: Qs7ExternalPrPacket): void {
  assertSafeField("runRef", packet.runRef);
  assertSafeField("shareUrl", packet.shareUrl);
  assertSafeField("browserRecording", packet.browserRecording);
  assertSafeField("terminalRecording", packet.terminalRecording);
  assertSafeField("distilledTest", packet.distilledTest);
  assertSafeField("trace", packet.trace);
  assertSafeField("chillEvalComparison", packet.chillEvalComparison);
  assertSafeField("verificationCommand", packet.verificationCommand);
  for (const [index, evalRow] of packet.evalRows.entries()) {
    assertSafeField(`evalRows.${index}.variant`, evalRow.variant);
    assertSafeField(`evalRows.${index}.axis`, evalRow.axis);
    assertSafeField(`evalRows.${index}.runtime`, evalRow.runtime);
    assertSafeField(`evalRows.${index}.notes`, evalRow.notes);
  }
}

export function renderQs7ExternalPrBody(packet: Qs7ExternalPrPacket): string {
  assertQs7ExternalPrPacketPublicSafe(packet);
  const lines: string[] = [
    "## QA Swarm audit",
    "",
    "This PR is a demo run of OpenAgents QA Swarm against `executor`: a real browser and terminal drove the public read-only scenario, the session was distilled into a reviewable e2e test, and the artifacts below are the review surface.",
    "",
    `Verdict: \`${packet.verdict}\``,
    `Share URL: ${packet.shareUrl}`,
    "",
    "### Artifacts",
    "",
    `- Browser recording: \`${packet.browserRecording}\``,
    `- Terminal recording: \`${packet.terminalRecording}\``,
    `- Distilled test: \`${packet.distilledTest}\``,
    `- Trace: ${packet.trace}`,
    `- Chill-evals comparison: ${packet.chillEvalComparison}`,
    "",
    "### Chill-evals variants",
    "",
    row(["Variant", "MCP/config axis", "Verdict", "Runtime", "Notes"]),
    row(["---", "---", "---", "---", "---"]),
    ...packet.evalRows.map((evalRow) =>
      row([
        evalRow.variant,
        evalRow.axis,
        evalRow.verdict,
        evalRow.runtime,
        evalRow.notes,
      ]),
    ),
    "",
    "### Verification",
    "",
    "```sh",
    packet.verificationCommand,
    "```",
    "",
    "The test source and recordings are the proof surface; reviewers should not need to run the project locally to understand the result.",
  ];

  return lines.join("\n");
}
