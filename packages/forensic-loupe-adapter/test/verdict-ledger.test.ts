import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vite-plus/test";

import { forensicCanonicalJson } from "@openagentsinc/forensic-contract";

import { durableFirstVerdictLedger } from "../src/verifier.ts";

/**
 * The durable first-verdict authority.
 *
 * The previous shape took a `commitInitialVerdict` function, so exactly-once
 * was a property of whatever the caller handed in — the residual the OFR-007
 * acceptance audit left named. The verifier now owns the compare-and-set, and
 * the primitive is `O_EXCL`, so the tests below have to beat the kernel rather
 * than a helper written a few lines up.
 */

const ledgerDirectory = () => mkdtempSync(join(tmpdir(), "loupe-ledger-test-"));

const verdict = (verdictRef: string) => ({
  schema: "openagents.loupe_initial_verdict.v1",
  verdictRef,
  verificationRef: "verification.ledger.v1",
  outcome: "confirmed",
});

describe("durable first-verdict ledger", () => {
  it("returns the stored verdict, not the candidate, on the losing path", async () => {
    const ledger = durableFirstVerdictLedger(ledgerDirectory());
    const first = await ledger.commit("verification.ledger.v1", verdict("verdict.first.v1"));
    const second = await ledger.commit("verification.ledger.v1", verdict("verdict.second.v1"));
    expect(first).toEqual(verdict("verdict.first.v1"));
    expect(second).toEqual(verdict("verdict.first.v1"));
  });

  it("returns what survived to disk, not the object it was handed", async () => {
    const ledger = durableFirstVerdictLedger(ledgerDirectory());
    // Deliberately out of canonical order, and carrying a member the canonical
    // encoder drops. What comes back must be the durable record, so it is
    // key-sorted and the dropped member is gone. Returning the candidate would
    // hand back an in-memory object that no reader could ever have observed.
    const stored = (await ledger.commit("verification.shape.v1", {
      verdictRef: "verdict.shape.v1",
      schema: "openagents.loupe_initial_verdict.v1",
      absent: undefined,
      outcome: "confirmed",
    })) as Record<string, unknown>;
    expect(Object.keys(stored)).toEqual(["outcome", "schema", "verdictRef"]);
    expect("absent" in stored).toBe(false);
  });

  it("keeps distinct verifications distinct", async () => {
    const ledger = durableFirstVerdictLedger(ledgerDirectory());
    await ledger.commit("verification.a.v1", verdict("verdict.a.v1"));
    await ledger.commit("verification.b.v1", verdict("verdict.b.v1"));
    expect(await ledger.read("verification.a.v1")).toEqual(verdict("verdict.a.v1"));
    expect(await ledger.read("verification.b.v1")).toEqual(verdict("verdict.b.v1"));
  });

  it("admits exactly one winner when many commits race in one process", async () => {
    const directory = ledgerDirectory();
    const ledger = durableFirstVerdictLedger(directory);
    const candidates = Array.from({ length: 64 }, (_, index) => verdict(`verdict.racer.${index}.v1`));
    const stored = await Promise.all(
      candidates.map(async (candidate) => await ledger.commit("verification.race.v1", candidate)),
    );
    const distinct = new Set(stored.map((entry) => forensicCanonicalJson(entry)));
    expect(distinct.size).toBe(1);
    // Exactly one racer got back its own candidate; the other 63 got the winner.
    expect(
      stored.filter(
        (entry, index) =>
          forensicCanonicalJson(entry) === forensicCanonicalJson(candidates[index]),
      ),
    ).toHaveLength(1);
    // And one record on disk, not sixty-four.
    expect(readdirSync(directory)).toHaveLength(1);
  });

  it("admits exactly one winner when separate processes race", async () => {
    const directory = ledgerDirectory();
    const module = fileURLToPath(new URL("../src/verdict-ledger.ts", import.meta.url));
    const script = (index: number) => `
      import { durableFirstVerdictLedger } from ${JSON.stringify(module)};
      const stored = await durableFirstVerdictLedger(${JSON.stringify(directory)}).commit(
        "verification.cross-process.v1",
        { schema: "openagents.loupe_initial_verdict.v1", verdictRef: "verdict.process.${index}.v1" },
      );
      process.stdout.write(JSON.stringify(stored));
    `;
    const results = await Promise.all(
      Array.from(
        { length: 8 },
        async (_, index) =>
          execFileSync(
            process.execPath,
            ["--experimental-strip-types", "--input-type=module", "-e", script(index)],
            { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
          ),
      ),
    );
    expect(new Set(results).size).toBe(1);
    expect(readdirSync(directory)).toHaveLength(1);
  });

  it("stores a schema-tagged record and refuses an unknown one", async () => {
    const directory = ledgerDirectory();
    const ledger = durableFirstVerdictLedger(directory);
    await ledger.commit("verification.ledger.v1", verdict("verdict.first.v1"));
    const file = join(directory, readdirSync(directory)[0] ?? "");
    const record = JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>;
    expect(record.schema).toBe("openagents.loupe_first_verdict_ledger.v1");
    expect(record.verificationRef).toBe("verification.ledger.v1");

    const foreign = durableFirstVerdictLedger(directory);
    const { writeFileSync } = await import("node:fs");
    writeFileSync(file, JSON.stringify({ schema: "something.else.v1", verdict: {} }));
    await expect(foreign.read("verification.ledger.v1")).rejects.toThrow("unknown schema");
  });

  it("reports no verdict for a verification it has never seen", async () => {
    const ledger = durableFirstVerdictLedger(ledgerDirectory());
    expect(await ledger.read("verification.absent.v1")).toBeUndefined();
  });
});
