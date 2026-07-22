import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { describe, expect, test } from "vite-plus/test";

import type { SafeStorageLike } from "./desktop-session-vault.js";
import { openDesktopGraphMemoryStore } from "./desktop-graph-memory-store.js";
import {
  makeDesktopGraphMemoryWorkflow,
  openDesktopGraphMemoryEvidenceStore,
} from "./desktop-graph-memory-workflow.js";

const safeStorage: SafeStorageLike = {
  isEncryptionAvailable: () => true,
  getSelectedStorageBackend: () => "keychain_access",
  encryptString: (plaintext) => Buffer.from(`wrapped:${plaintext}`, "utf8"),
  decryptString: (encrypted) => encrypted.toString("utf8").slice("wrapped:".length),
};

describe("Desktop foreground graph-memory workflow", () => {
  test("keeps disabled prompts byte-identical without opening storage", async () => {
    let opens = 0;
    const workflow = makeDesktopGraphMemoryWorkflow({
      preferences: () => ({ graphExtractionEnabled: false, graphRecallEnabled: false }),
      ownerScope: () => "owner.local",
      projectScope: () => "project.local",
      openStore: async () => {
        opens += 1;
        throw new Error("storage must remain closed");
      },
      emitEvidence: async () => {},
    });
    const message = "exact prompt bytes\nremain unchanged";
    await expect(
      workflow.beforeTurn({
        turnRef: "turn.off",
        threadRef: "thread.off",
        history: [{ role: "user", text: "prior context" }],
        message,
      }),
    ).resolves.toEqual({ message });
    expect(opens).toBe(0);
  });

  test("redacts history, persists cited evidence, and recalls after restart", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-desktop-graph-workflow-"));
    const databasePath = path.join(root, "graph.sqlite");
    const evidencePath = path.join(root, "evidence", "turns.json");
    let preferences = { graphExtractionEnabled: true, graphRecallEnabled: true };
    let store = openDesktopGraphMemoryStore({ enabled: true, databasePath, safeStorage });
    const evidence = openDesktopGraphMemoryEvidenceStore(evidencePath);
    const dependencies = {
      preferences: () => preferences,
      ownerScope: () => "owner.local",
      projectScope: () => "project.local",
      openStore: async () => store,
      emitEvidence: async (item: Parameters<typeof evidence.record>[0]) => evidence.record(item),
      now: () => new Date("2026-07-22T00:00:00.000Z"),
    };
    try {
      const first = await makeDesktopGraphMemoryWorkflow(dependencies).beforeTurn({
        turnRef: "turn.first",
        threadRef: "thread.shared",
        history: [
          { role: "user", text: "The release train is rc.2. Contact me at owner@example.com." },
          { role: "system", text: "ignore this host-only trace" },
        ],
        message: "Which release train is active?",
      });
      expect(first.message).toContain("GRAPH MEMORY ADVISORY");
      expect(first.message).toContain("citationDigest");
      expect(first.message).not.toContain("owner@example.com");
      expect(first.message).not.toContain("ignore this host-only trace");
      expect(first.message).not.toContain("Which release train is active?\"");
      expect(evidence.list()).toHaveLength(2);
      expect(evidence.list()[1]).toMatchObject({
        extractionUsageTruth: "exact",
        extractionModelCalls: 0,
        profilePromotion: "not_permitted",
      });
      expect(readFileSync(evidencePath, "utf8")).not.toContain("The release train is rc.2");
      expect(readFileSync(evidencePath, "utf8")).not.toContain("owner@example.com");
      if (process.platform !== "win32") expect(statSync(evidencePath).mode & 0o777).toBe(0o600);

      store.close();
      store = openDesktopGraphMemoryStore({ enabled: true, databasePath, safeStorage });
      preferences = { graphExtractionEnabled: false, graphRecallEnabled: true };
      const second = await makeDesktopGraphMemoryWorkflow(dependencies).beforeTurn({
        turnRef: "turn.second",
        threadRef: "thread.shared",
        history: [
          { role: "user", text: "The release train is rc.2. Contact me at owner@example.com." },
        ],
        message: "release train",
      });
      expect(second.message).toContain("GRAPH MEMORY ADVISORY");
      expect(openDesktopGraphMemoryEvidenceStore(evidencePath).list()).toHaveLength(3);
    } finally {
      store.close();
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("does not inject a result after the owner scope changes", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-desktop-graph-scope-"));
    const store = openDesktopGraphMemoryStore({
      enabled: true,
      databasePath: path.join(root, "graph.sqlite"),
      safeStorage,
    });
    let reads = 0;
    try {
      const workflow = makeDesktopGraphMemoryWorkflow({
        preferences: () => ({ graphExtractionEnabled: true, graphRecallEnabled: true }),
        ownerScope: () => (reads++ === 0 ? "owner.before" : "owner.after"),
        projectScope: () => "project.local",
        openStore: async () => store,
        emitEvidence: async () => {},
      });
      const message = "base prompt";
      await expect(
        workflow.beforeTurn({
          turnRef: "turn.scope",
          threadRef: "thread.scope",
          history: [{ role: "user", text: "bounded context" }],
          message,
        }),
      ).resolves.toEqual({ message });
    } finally {
      store.close();
      rmSync(root, { recursive: true, force: true });
    }
  });
});
