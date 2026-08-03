import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { createInterface } from "node:readline";

import {
  decodeProtocolInitializeResult,
  decodePlanningGraphReadResult,
  decodeWorkIndexReadResult,
  decodeWorkSnapshotReadResult,
} from "@openagentsinc/all-work-contract";
import { expect, test } from "vite-plus/test";

import { openFullAutoRunRegistry } from "../engine/full-auto-run-registry.ts";
import { resolveFullAutoRunsPath } from "../paths.ts";
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA } from "./framed.ts";

test("the omega-effectd process serves typed All Work v2 index and snapshot reads", async () => {
  const dataRoot = mkdtempSync(resolve(tmpdir(), "omega-effectd-all-work-process-"));
  const run = openFullAutoRunRegistry(
    resolveFullAutoRunsPath({ dataRoot }),
    () => new Date("2026-08-02T12:00:00Z"),
  ).createDraft({
    title: "Process-level Work row",
    objective: "Private process objective",
    doneCondition: "The actual stdio process returns typed Work.",
    objectiveSource: "user",
    threadRef: "thread:process:1",
  });
  const child = spawn(
    process.execPath,
    ["--import", "tsx", resolve(import.meta.dirname, "../bin/omega-effectd.ts")],
    {
      env: { ...process.env, OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT: dataRoot },
      stdio: ["pipe", "pipe", "pipe"],
    },
  );
  const output = createInterface({ input: child.stdout });
  const errors: Array<string> = [];
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk: string) => errors.push(chunk));
  const readResponse = (): Promise<Record<string, unknown>> =>
    new Promise((resolveResponse, rejectResponse) => {
      output.once("line", (line) => {
        try {
          resolveResponse(JSON.parse(line) as Record<string, unknown>);
        } catch (error) {
          rejectResponse(error);
        }
      });
      child.once("exit", (code) =>
        rejectResponse(new Error(`omega-effectd exited ${code}: ${errors.join("")}`)),
      );
    });
  const send = (frame: Record<string, unknown>): void => {
    child.stdin.write(`${JSON.stringify(frame)}\n`);
  };

  try {
    send({
      schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
      kind: "request",
      id: "init",
      generation: 0,
      method: "initialize",
      params: {
        generation: 1,
        allWork: {
          supportedVersions: ["omega-effectd.v2", "omega-effectd.v1"],
          requestedCapabilities: ["work.index.read", "work.snapshot.read", "planning.graph.read"],
        },
      },
    });
    const initialized = await readResponse();
    expect(initialized.ok).toBe(true);
    expect(
      decodeProtocolInitializeResult((initialized.result as { allWork: unknown }).allWork)
        .selectedVersion,
    ).toBe("omega-effectd.v2");

    send({
      schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
      kind: "request",
      id: "index",
      generation: 1,
      method: "work.index.read",
      params: {},
    });
    const indexed = await readResponse();
    expect(indexed.ok).toBe(true);
    const indexResult = decodeWorkIndexReadResult(indexed.result);
    expect(indexResult.items[0]?.workRef).toBe(`work:${run.runRef}`);
    expect(JSON.stringify(indexResult)).not.toContain("Private process objective");

    send({
      schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
      kind: "request",
      id: "snapshot",
      generation: 1,
      method: "work.snapshot.read",
      params: { workRef: `work:${run.runRef}` },
    });
    const snapshot = await readResponse();
    expect(snapshot.ok).toBe(true);
    expect(decodeWorkSnapshotReadResult(snapshot.result).snapshot.threadRefs).toEqual([
      "thread:process:1",
    ]);

    send({
      schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
      kind: "request",
      id: "planning",
      generation: 1,
      method: "planning.graph.read",
      params: {},
    });
    const planning = await readResponse();
    expect(planning.ok).toBe(true);
    const graph = decodePlanningGraphReadResult(planning.result).graph;
    expect(graph.work).toHaveLength(28);
    expect(graph.sourceCoordinates).toHaveLength(28);
    expect(graph.reconciliationDigest).toMatch(/^[a-f0-9]{64}$/u);

    child.stdin.end();
    await new Promise<void>((resolveExit, rejectExit) => {
      child.once("exit", (code) =>
        code === 0
          ? resolveExit()
          : rejectExit(new Error(`omega-effectd exited ${code}: ${errors.join("")}`)),
      );
    });
  } finally {
    output.close();
    if (child.exitCode === null) child.kill("SIGTERM");
    rmSync(dataRoot, { force: true, recursive: true });
  }
});
