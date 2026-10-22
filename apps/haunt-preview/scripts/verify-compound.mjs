import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "@playwright/test";
const base = process.env.HAUNT_URL;
if (!base) throw new Error("Set HAUNT_URL to the local creator server.");
const out = resolve(
  process.env.HAUNT_EVIDENCE || "dist/verification",
  "compound",
);
await mkdir(out, { recursive: true });
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const page = await browser.contexts()[0].newPage();
await page.setViewportSize({ width: 1440, height: 960 });
const report = { cases: [], failures: [], checks: [] },
  errors = [];
let requests = 0;
page.on("pageerror", (e) => errors.push(String(e)));
page.on("request", (r) => {
  if (/\/api\/(haunt|medieval)\/decision$/.test(r.url())) requests++;
});
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
const content = (p) => ({ ...p, revision: 0 });
const matrix = {
  haunt: [
    ["add 3 ghosts and make them chase me", { ghost: 3 }, "chase"],
    ["spawn two ghosts then have them follow me", { ghost: 2 }, "chase"],
    ["add a ghost and make it float", { ghost: 1 }, "float"],
    ["add 2 pumpkins and 3 candles", { pumpkin: 2, candle: 3 }],
    ["add two ghosts and make them small", { ghost: 2 }, null, 0.6],
    [
      "add 2 ghosts, set the light blue, and make them chase me",
      { ghost: 2 },
      "chase",
    ],
    ["put a ghost at (1, 2) and make it chase me", { ghost: 1 }, "chase"],
    ["add 3 ghosts and stop all ghosts", { ghost: 3 }, "still"],
  ],
  medieval: [
    ["add 2 houses and put benches outside", { cottage: 2, bench: 2 }],
    [
      "put two cottages wherever you want then place a bench outside each",
      { cottage: 2, bench: 2 },
    ],
    [
      "add 2 houses and put 2 benches outside each house",
      { cottage: 2, bench: 4 },
    ],
    [
      "create a townhouse and put a fountain outside it",
      { townhouse: 1, fountain: 1 },
    ],
    ["add two barrels and three chairs", { barrel: 2, chair: 3 }],
    [
      "add a cottage and put benches outside and turn the light blue",
      { cottage: 1, bench: 1 },
    ],
    ["add a room and add a chair", { chair: 1 }],
    [
      "add two buildings your choice anywhere and put small benches outside",
      { building: 2, bench: 2 },
    ],
  ],
};
try {
  for (const [lab, cases] of Object.entries(matrix)) {
    await page.goto(new URL(lab === "haunt" ? "/" : "/medieval/", base).href);
    await ready();
    if (lab === "haunt")
      await command({
        kind: "add_asset",
        entity: {
          id: "ghost-existing",
          kind: "ghost",
          x: 1,
          z: 2,
          scale: 1,
          behavior: "float",
        },
      });
    const baseline = (await snap()).project;
    await page.evaluate(() => {
      const write = window.kilnWrite;
      window.compoundWrites = 0;
      window.kilnWrite = async (path, source) => {
        if (path.endsWith("/scene.json")) window.compoundWrites++;
        return write(path, source);
      };
    });
    for (const [text, kinds, behavior, scale] of cases) {
      await command({
        kind: "replace",
        project: { ...baseline, revision: (await snap()).project.revision + 1 },
      });
      const before = await snap(),
        previous = await page.evaluate(
          () => window.hauntPreview.editor().decisions.length,
        ),
        calls = requests;
      await page.evaluate(() => {
        window.compoundWrites = 0;
      });
      const responsePromise = page.waitForResponse((r) =>
        /\/api\/(haunt|medieval)\/decision$/.test(r.url()),
      );
      const item = { lab, text };
      try {
        await page.locator("#request").fill(text);
        await page.locator("#ask").click();
        item.decision = await (await responsePromise).json();
        await page.waitForFunction(
          () =>
            document.querySelector("#cancel-decision").hidden &&
            !document.querySelector("#ask").disabled,
          null,
          { timeout: 6000 },
        );
        item.status = await page.locator("#scene-status").textContent();
        const result = await page.evaluate(() =>
          window.hauntPreview.editor().decisions.at(-1),
        );
        assert.equal(result?.text, text, item.status);
        assert.equal(item.decision.action, "compound");
        item.totalMs = result.totalMs;
        assert.equal(requests - calls, 1);
        assert.equal(await page.evaluate(() => window.compoundWrites), 1);
        const after = await snap(),
          added = after.project.entities.filter(
            (e) => !before.project.entities.some((b) => b.id === e.id),
          );
        item.added = added;
        assert.equal(after.project.revision, before.project.revision + 1);
        assert.equal(
          await page.evaluate(
            () => window.hauntPreview.editor().decisions.length,
          ),
          previous + 1,
        );
        for (const [kind, count] of Object.entries(kinds))
          assert.equal(
            added.filter((e) =>
              kind === "building"
                ? ["cottage", "townhouse"].includes(e.kind)
                : e.kind === kind,
            ).length,
            count,
          );
        assert.equal(
          added.length,
          Object.values(kinds).reduce((a, b) => a + b, 0),
        );
        if (behavior)
          assert.ok(
            added
              .filter((e) => e.kind === "ghost")
              .every((e) => e.behavior === behavior),
          );
        if (scale) assert.ok(added.every((e) => e.scale === scale));
        if (lab === "haunt")
          assert.equal(
            after.project.entities.find((e) => e.id === "ghost-existing")
              .behavior,
            text.includes("all ghosts") ? "still" : "float",
          );
        if (text.includes("(1, 2)"))
          assert.deepEqual({ x: added[0].x, z: added[0].z }, { x: 1, z: 2 });
        assert.deepEqual(after.pose, before.pose);
        assert.equal(after.hash, before.hash);
        assert.equal(after.deviceId, before.deviceId);
        assert.equal(after.compiles, before.compiles);
        assert.ok(after.geometryBuilds <= before.geometryBuilds + 1);
        if (lab === "medieval" && text.includes("outside")) {
          const houses = added.filter((e) =>
              ["cottage", "townhouse"].includes(e.kind),
            ),
            props = added.filter(
              (e) => !["cottage", "townhouse"].includes(e.kind),
            );
          for (const house of houses)
            assert.ok(
              props.some((p) => Math.hypot(p.x - house.x, p.z - house.z) < 15),
            );
          for (const p of props)
            for (const house of houses)
              for (const r of after.project.town.buildings[house.id].floors[0]
                .rooms)
                assert.ok(
                  Math.abs(p.x - (house.x + r.x * house.scale)) >
                    (r.width * house.scale) / 2 ||
                    Math.abs(p.z - (house.z + r.z * house.scale)) >
                      (r.depth * house.scale) / 2,
                );
        }
        if (behavior === "chase") {
          const actors = after.actors.filter((a) =>
            added.some((e) => e.id === a.id),
          );
          await page.waitForFunction(
            (old) =>
              window.hauntPreview.snapshot().actors.some((a) => {
                const p = old.find((b) => b.id === a.id);
                return p && Math.hypot(a.x - p.x, a.z - p.z) > 0.1;
              }),
            actors,
            { timeout: 6000 },
          );
          item.moving = true;
        }
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
          if (lab === "medieval") {
            await page.locator("#house").focus();
            await page.keyboard.down("ArrowLeft");
            await page.waitForTimeout(500);
            await page.keyboard.up("ArrowLeft");
          }
          await page.screenshot({ path: `${out}/${lab}.png` });
          await page.reload();
          await ready();
          assert.deepEqual(
            content((await snap()).project),
            content(after.project),
          );
          await page.evaluate(() => {
            const write = window.kilnWrite;
            window.compoundWrites = 0;
            window.kilnWrite = async (path, source) => {
              if (path.endsWith("/scene.json")) window.compoundWrites++;
              return write(path, source);
            };
          });
        }
        item.passed = true;
      } catch (error) {
        item.error = String(error);
        await page
          .screenshot({
            path: `${out}/${lab}-${report.cases.length}-failure.png`,
          })
          .catch(() => {});
      }
      report.cases.push(item);
      console.log(
        JSON.stringify({
          lab,
          text,
          passed: item.passed ?? false,
          totalMs: item.totalMs,
          error: item.error,
        }),
      );
    }
    for (const text of lab === "haunt"
      ? [
          "add 3 ghosts and make them shoot fireballs",
          "add 2 pumpkins and make them chase me",
          "add a ghost and add 9 candles",
          "add a ghost and make it float and teleport me",
          "add a ghost and undo",
        ]
      : [
          "add 2 houses and put 9 benches outside",
          "add a cottage and add a dragon",
          "add 2 houses and make them chase me",
          "add a barrel and add a ghost",
        ]) {
      await command({
        kind: "replace",
        project: { ...baseline, revision: (await snap()).project.revision + 1 },
      });
      const before = await snap(),
        n = await page.evaluate(
          () => window.hauntPreview.editor().decisions.length,
        );
      const responsePromise = page.waitForResponse((r) =>
        /\/api\/(haunt|medieval)\/decision$/.test(r.url()),
      );
      await page.locator("#request").fill(text);
      await page.locator("#ask").click();
      const decision = await (await responsePromise).json();
      await page.waitForFunction(
        () =>
          document.querySelector("#cancel-decision").hidden &&
          !document.querySelector("#ask").disabled,
        null,
        { timeout: 6000 },
      );
      const after = await snap(),
        passed =
          JSON.stringify(after.project) === JSON.stringify(before.project) &&
          (await page.evaluate(
            (n) => window.hauntPreview.editor().decisions.length === n,
            n,
          ));
      report.failures.push({ lab, text, passed, decision });
    }
  }
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
    report.failures.every((c) => c.passed),
    JSON.stringify(report.failures.filter((c) => !c.passed)),
  );
  report.checks.push(
    "One decision HTTP request, one scene revision, one Kiln scene write and one undo/redo step per compound edit; unchanged player, shader, device, and compilation count.",
    "Chasing new ghosts move in the live simulation while existing ghosts keep their behavior; outside props are beside the newly created houses.",
    "Both labs reopen combined edits from Kiln; invalid later clauses leave no partial scene or history change.",
  );
  report.passed = true;
} finally {
  await writeFile(`${out}/receipt.json`, JSON.stringify(report, null, 2));
  await browser.close();
}
