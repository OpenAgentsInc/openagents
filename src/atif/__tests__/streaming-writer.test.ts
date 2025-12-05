import { describe, expect, test } from "bun:test";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { StreamingWriter } from "../streaming-writer.js";
import { generateSessionId, type Agent, type Step } from "../schema.js";

const baseAgent: Agent = {
  name: "test-agent",
  version: "1.0.0",
  model_name: "test-model",
};

const makeStep = (id: number): Step => ({
  step_id: id,
  timestamp: `2024-01-01T00:00:0${id}.000Z`,
  source: "system",
  message: `step-${id}`,
});

const withWriter = async (
  fn: (writer: StreamingWriter, baseDir: string) => Promise<void>,
) => {
  const baseDir = await mkdtemp(path.join(tmpdir(), "streaming-writer-"));
  const writer = new StreamingWriter({
    sessionId: generateSessionId(),
    agent: baseAgent,
    baseDir,
  });
  await writer.initialize();

  try {
    await fn(writer, baseDir);
  } finally {
    await rm(baseDir, { recursive: true, force: true });
  }
};

describe("StreamingWriter", () => {
  test("recreates index directory when it disappears mid-run", async () => {
    await withWriter(async (writer) => {
      const { dateDir } = writer.getPaths();
      await rm(dateDir, { recursive: true, force: true });

      // @ts-expect-error Accessing private method for targeted regression coverage
      await writer.updateIndex({
        status: "in_progress",
        final_metrics: null,
      });

      const indexData = JSON.parse(
        await readFile(writer.getPaths().index, "utf-8"),
      );

      expect(indexData.status).toBe("in_progress");
      expect(indexData.checkpoint.completed_step_count).toBe(0);
    });
  });

  test("avoids tmp rename collisions during parallel step writes", async () => {
    await withWriter(async (writer) => {
      await Promise.all([
        writer.writeStep(makeStep(1)),
        writer.writeStep(makeStep(2)),
        writer.writeStep(makeStep(3)),
      ]);

      const paths = writer.getPaths();
      const indexData = JSON.parse(await readFile(paths.index, "utf-8"));

      expect(indexData.checkpoint.completed_step_count).toBe(3);
      expect(indexData.checkpoint.step_id).toBe(3);
      expect(indexData.status).toBe("in_progress");

      const files = await readdir(paths.dateDir);
      expect(files.every((name) => !name.endsWith(".tmp"))).toBe(true);
    });
  });
});
