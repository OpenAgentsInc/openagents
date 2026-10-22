import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";
const base = process.env.HAUNT_URL;
if (!base) throw new Error("Set HAUNT_URL to the local creator server.");
const out = resolve(
  process.env.HAUNT_EVIDENCE || "dist/verification",
  "natural-defaults",
);
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const page = await browser.contexts()[0].newPage();
await page.setViewportSize({ width: 1440, height: 960 });
const errors = [],
  report = { cases: [], refusals: [], checks: [] };
page.on("pageerror", (e) => errors.push(String(e)));
const snap = () => page.evaluate(() => window.hauntPreview.snapshot());
const command = (op) =>
  page.evaluate((o) => window.hauntPreview.command(o), op);
const ready = () =>
  page.waitForFunction(
    () =>
      window.hauntPreview?.snapshot().accepted > 0 &&
      !document.querySelector("#ask").disabled,
    null,
    { timeout: 30000 },
  );
let lastAnswer;
page.on("response", async (response) => {
  if (/\/api\/(haunt|medieval)\/decision$/.test(response.url()))
    lastAnswer = await response.json().catch(() => undefined);
});
const content = (p) => ({ ...p, revision: 0 });
const rooms = (p) =>
  p.town
    ? Object.values(p.town.buildings).reduce(
        (n, b) => n + b.floors.reduce((n, f) => n + f.rooms.length, 0),
        0,
      )
    : (p.layout?.rooms.length ?? 3);
const matrix = {
  medieval: [
    [
      "add 2 buildings your choice anywhere u want",
      2,
      ["cottage", "townhouse"],
    ],
    [
      "Can you put a couple of houses wherever you think looks good?",
      2,
      ["cottage"],
    ],
    ["put two pieces of furniture wherever they fit", 2, ["chair"]],
    ["add some barrels somewhere, size is up to you", 3, ["barrel"]],
    ["add an object of your choice, pick a spot", 1],
    ["Surprise me with a decoration wherever it fits", 1],
    [
      "add two buildings, style and location are up to you",
      2,
      ["cottage", "townhouse"],
    ],
    ["place a bench anywhere", 1, ["bench"]],
    ["add 2 cool decorations of your choice wherever there's space", 2],
    ["add a room wherever it will fit", "add_room"],
    ["add a small wing wherever it fits", "add_wing"],
    [
      "put a barrel at (0, 17), your choice of size",
      1,
      ["barrel"],
      { x: 0, z: 17 },
    ],
    [
      "add two small chairs to my right, you choose the rest",
      2,
      ["chair"],
      "right",
    ],
    ["move the selected barrel somewhere sensible", "move_asset"],
  ],
  haunt: [
    ["add a couple of ghosts wherever you like", 2, ["ghost"]],
    ["put 3 scary decorations anywhere u want", 3],
    ["add something spooky, your choice", 1],
    ["place a candle wherever it fits", 1, ["candle"]],
    ["can you add a few pumpkins, put them wherever", 3, ["pumpkin"]],
    ["give me two props, you pick what and where", 2],
    ["spawn two of your choice anywhere", 2],
    [
      "put some cauldrons somewhere, whatever size you think works",
      3,
      ["cauldron"],
    ],
    ["add an object, surprise me, pick the size and location", 1],
    ["add a room wherever there's space", "add_room"],
    ["build a small wing, you choose where", "add_wing"],
    ["add a ghost at (1, 2), pick the size", 1, ["ghost"], { x: 1, z: 2 }],
    [
      "add 2 tiny pumpkins on my left, your choice of anything else",
      2,
      ["pumpkin"],
      "left",
    ],
    ["move that pumpkin anywhere you like", "move_asset"],
    ["put it somewhere else, your choice", "move_asset"],
    ["make the ghosts move around, your choice of behavior", "set_behavior"],
    ["change the lantern color, your choice", "set_tint"],
  ],
};
try {
  for (const [lab, cases] of Object.entries(matrix)) {
    await page.goto(
      new URL(lab === "medieval" ? "/medieval/" : "/", base).href,
    );
    await ready();
    const baseline = (await snap()).project;
    for (const [text, expected, kinds, explicit] of cases) {
      await command({
        kind: "replace",
        project: { ...baseline, revision: (await snap()).project.revision + 1 },
      });
      if (expected === "set_behavior")
        await command({
          kind: "add_asset",
          entity: { id: "ghost-test", kind: "ghost", x: 1, z: 2, scale: 1 },
        });
      if (expected === "move_asset") {
        if (lab === "medieval")
          await page.getByRole("button", { name: "2 Assets" }).click();
        await page
          .locator("#selected")
          .selectOption(lab === "medieval" ? "barrel-starter" : "pumpkin-1");
      }
      if (expected === "move_asset" && lab === "medieval")
        await page.keyboard.press("Escape");
      const before = await snap(),
        n = await page.evaluate(
          () => window.hauntPreview.editor().decisions.length,
        );
      lastAnswer = undefined;
      const record = { lab, text };
      try {
        await page.locator("#request").fill(text);
        await page.locator("#ask").click();
        await page.waitForFunction(
          (n) =>
            window.hauntPreview.editor().decisions.length > n ||
            (!document.querySelector("#ask").disabled &&
              !/Thinking|Choosing|Making/.test(
                document.querySelector("#scene-status").textContent,
              )),
          n,
          { timeout: 6000 },
        );
        const result = await page.evaluate(() =>
          window.hauntPreview.editor().decisions.at(-1),
        );
        const after = await snap();
        record.status = await page.locator("#scene-status").textContent();
        assert.equal(result?.text, text, record.status);
        record.decision = result.decision;
        record.totalMs = result.totalMs;
        const added = after.project.entities.slice(
          before.project.entities.length,
        );
        if (typeof expected === "number") {
          assert.ok(["add_asset", "compound"].includes(result.decision.action));
          assert.equal(added.length, expected);
          if (kinds)
            assert.ok(
              added.every((e) => kinds.includes(e.kind)),
              JSON.stringify(added),
            );
          if (explicit && typeof explicit === "object")
            assert.deepEqual({ x: added[0].x, z: added[0].z }, explicit);
          if (typeof explicit === "string")
            assert.equal(result.decision.placement, explicit);
          record.added = added;
        } else {
          assert.equal(result.decision.action, expected);
          if (expected === "add_room" || expected === "add_wing")
            assert.equal(
              rooms(after.project),
              rooms(before.project) + (expected === "add_wing" ? 3 : 1),
            );
        }
        assert.deepEqual(after.pose, before.pose);
        assert.equal(after.deviceId, before.deviceId);
        assert.equal(after.hash, before.hash);
        await command({ kind: "undo" });
        assert.deepEqual(
          content((await snap()).project),
          content(before.project),
        );
        await command({ kind: "redo" });
        assert.deepEqual(
          content((await snap()).project),
          content(after.project),
        );
        if (text === cases[0][0]) {
          await page.screenshot({ path: `${out}/${lab}.png` });
          await page.reload();
          await ready();
          assert.deepEqual(
            content((await snap()).project),
            content(after.project),
          );
        }
        record.passed = true;
      } catch (error) {
        record.error = String(error);
        record.decision = lastAnswer;
        record.status = await page.locator("#scene-status").textContent();
        await page.screenshot({
          path: `${out}/${lab}-${report.cases.length}-failure.png`,
        });
      }
      report.cases.push(record);
      console.log(
        JSON.stringify({
          lab,
          text,
          passed: record.passed ?? false,
          totalMs: record.totalMs,
          error: record.error,
          status: record.passed ? undefined : record.status,
        }),
      );
    }
    for (const text of [
      "do not add anything anywhere",
      "add a dragon anywhere, your choice of size",
      "add a barrel and shoot lightning",
      "what could you add anywhere?",
      "add a dragon and a pumpkin anywhere",
      lab === "medieval"
        ? "add 9 buildings anywhere"
        : "add 9 pumpkins anywhere",
      lab === "medieval" ? "add a ghost anywhere" : "add a cottage anywhere",
    ]) {
      const before = await snap(),
        n = await page.evaluate(
          () => window.hauntPreview.editor().decisions.length,
        );
      const responsePromise = page.waitForResponse(
        (r) =>
          /\/api\/(haunt|medieval)\/decision$/.test(r.url()) &&
          r.request().method() === "POST",
      );
      await page.locator("#request").fill(text);
      await page.locator("#ask").click();
      const d = await (await responsePromise).json();
      await page.waitForFunction(
        () =>
          document.querySelector("#cancel-decision").hidden &&
          !document.querySelector("#ask").disabled,
        null,
        { timeout: 6000 },
      );
      const after = await snap();
      const passed =
        !!d &&
        JSON.stringify(after.project) === JSON.stringify(before.project) &&
        (await page.evaluate(
          (n) => window.hauntPreview.editor().decisions.length === n,
          n,
        ));
      report.refusals.push({ lab, text, passed, decision: d });
    }
  }
  report.checks.push(
    "Both labs preserve player pose and the working shader, save to Kiln, undo and redo as one edit, and reopen their first natural request from Kiln.",
  );
  assert.deepEqual(errors, []);
  assert.ok(
    report.cases.every((c) => c.passed),
    JSON.stringify(
      report.cases
        .filter((c) => !c.passed)
        .map((c) => ({ text: c.text, error: c.error })),
    ),
  );
  assert.ok(
    report.refusals.every((c) => c.passed),
    JSON.stringify(report.refusals.filter((c) => !c.passed)),
  );
  report.passed = true;
} finally {
  await writeFile(`${out}/receipt.json`, JSON.stringify(report, null, 2));
  await browser.close();
}
