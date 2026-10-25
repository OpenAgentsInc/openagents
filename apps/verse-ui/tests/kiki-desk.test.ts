// Kiki's house editor desk in Chromium, over a copy of the release's
// controls: movable, closable cards, agent card commands, Kiln requests,
// inventory placement through the release's own Add prop, payouts, and the
// guest one-change rule. Run `npm run build` first (it loads the built
// ui.css); set AETHYR_CHROMIUM as for the renderer tests.
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { build } from "esbuild";
import { chromium, type Browser, type Page } from "playwright";

const here = import.meta.dirname;
const cssFile = join(
  here,
  "../../../crates/openagents-web/content/verse-ui/ui.css",
);

const BRIDGE = `
<div class="studio-bridge">
  <aside id="edit-panel" data-active="objects">
    <div id="prop-tools"><input id="edit-place-asset"><button id="edit-place" type="button">Add prop</button></div>
    <input id="edit-x" value="1"><input id="edit-y" value="0"><input id="edit-z" value="2">
    <textarea id="edit-kiln-request"></textarea>
    <button id="edit-request-review" type="button">Review request</button>
    <p id="edit-kiln-status"></p>
    <button id="edit-undo" type="button">Undo</button>
    <ol id="edit-history-entries"></ol>
    <p id="visit-earnings">Earnings: <strong>Not recorded</strong></p>
    <p id="edit-status"></p>
  </aside>
  <button id="project-play" type="button" hidden disabled>Play</button>
  <button id="project-publish" type="button">Publish</button>
  <button id="project-revert" type="button" hidden disabled>Revert</button>
  <button id="request-compile" type="button">Compile world effect</button>
  <button id="world-edit" type="button">Edit</button>
  <p id="voice-heard"></p>
  <dialog id="edit-auth"><h2 id="edit-auth-title">Confirm your change</h2>
    <button id="edit-cancel" type="button" onclick="this.closest('dialog').close()">Cancel</button></dialog>
</div>
<div id="house-hud" class="house-hud"><div class="aui-root">
  <section class="hud-panel" data-aethyr-id="world-voice"><p>Microphone off</p></section>
</div></div>
<div id="house-editor" class="house-editor"><div class="aui-root">
  <section class="hud-panel aui-sheet" data-aethyr-id="edit-panel"><button class="aui-close" type="button">×</button><p>Jarvis</p></section>
</div></div>
<script id="creator-assets" type="application/json">[{"id":"fantasy:lamp","name":"Lamp"},{"id":"fantasy:barrel","name":"Barrel"}]</script>
<script>
  window.clicks = [];
  document.addEventListener("click", (e) => { if (e.target.id) window.clicks.push(e.target.id); }, true);
</script>`;

const captures = process.env.AETHYR_CAPTURE_DIR;
const capture = async (name: string) => {
  if (captures)
    await page.screenshot({ path: join(captures, `kiki-${name}.png`) });
};

let browser: Browser;
let page: Page;
let script: string;

before(async () => {
  const bundle = await build({
    entryPoints: [join(here, "kiki/harness.ts")],
    bundle: true,
    write: false,
    format: "iife",
    platform: "browser",
    target: "es2023",
    jsx: "automatic",
    tsconfig: join(here, "../tsconfig.json"),
    define: { "process.env.NODE_ENV": '"production"' },
    logLevel: "silent",
  });
  script = bundle.outputFiles[0]!.text;
  browser = await chromium.launch({
    executablePath: process.env.AETHYR_CHROMIUM || undefined,
  });
  page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
});

after(async () => {
  await browser?.close();
});

async function open(kiki = true) {
  // setContent keeps the window, so the last desk's timers stop first.
  await page.evaluate(() => (window as any).kiki?.dispose?.()).catch(() => {});
  await page.setContent(
    `<!doctype html><html lang="en" class="dark${kiki ? " kiki" : ""}" data-verse-ui="studio"><head>` +
      `<meta charset="utf-8"><style>${readFileSync(cssFile, "utf8")}</style></head>` +
      `<body class="${kiki ? "kiki" : ""}" data-kiki-guest="true">${BRIDGE}</body></html>`,
  );
  await page.addScriptTag({ content: script });
  await page
    .evaluate(() => {
      localStorage.clear?.();
    })
    .catch(() => {});
  await page.evaluate(() => window.kiki.mount());
  await page.locator('[data-kiki-pane="chat"]').waitFor();
}

const clicks = () => page.evaluate(() => (window as any).clicks as string[]);
const agent = async (text: string) => {
  await page.getByRole("tab", { name: "Agent" }).click();
  await page.getByLabel("Message the agent").fill(text);
  await page.getByLabel("Message the agent").press("Enter");
};

test("the editor's cards are glass panes with a header and an ×", async () => {
  await open();
  for (const card of [
    "tools",
    "voice",
    "chat",
    "inventory",
    "payouts",
    "actions",
  ])
    assert(
      await page
        .locator(`[data-kiki-pane="${card}"] > [data-kiki-bar]`)
        .count(),
      `${card} has a header`,
    );
  const glass = await page.locator('[data-kiki-pane="chat"]').evaluate((el) => {
    const s = getComputedStyle(el);
    return [s.borderTopLeftRadius, s.backdropFilter];
  });
  assert.equal(glass[0], "14px");
  assert.match(glass[1]!, /blur\(18px\)/);
  // The Jarvis sheet's own ×, which leaves the editor, gives way to the card's.
  assert(
    !(await page
      .locator('[data-aethyr-id="edit-panel"] > .aui-close')
      .isVisible()),
  );
  await capture("desktop");
  await page.setViewportSize({ width: 390, height: 844 });
  await capture("phone");
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth > innerWidth,
  );
  assert(!overflow, "no sideways scroll on a phone");
  await page.setViewportSize({ width: 1440, height: 900 });
});

test("× closes a card, the agent opens and closes cards by name", async () => {
  await open();
  await page.getByRole("button", { name: "Close World inventory" }).click();
  assert(!(await page.locator('[data-kiki-pane="inventory"]').isVisible()));
  await agent("show inventory");
  await page
    .locator('[data-kiki-pane="inventory"]')
    .waitFor({ state: "visible" });
  await agent("close the tools window");
  assert(!(await page.locator('[data-kiki-pane="tools"]').isVisible()));
  await page.getByText("Closed Tools.").waitFor();
  await agent("open tools");
  await page.locator('[data-kiki-pane="tools"]').waitFor({ state: "visible" });
  // A closed chat leaves a way back.
  await page.getByRole("button", { name: "Close Chat" }).click();
  await page.getByRole("button", { name: "Chat with the agent" }).click();
  await page.locator('[data-kiki-pane="chat"]').waitFor({ state: "visible" });
  // Commands the house's voice controls heard work too.
  await page.evaluate(() => {
    document.getElementById("voice-heard")!.textContent = "Heard: hide chat";
  });
  await page.locator('[data-kiki-pane="chat"]').waitFor({ state: "hidden" });
});

test("cards move by dragging their header or with the arrow keys", async () => {
  await open();
  const pane = page.locator('[data-kiki-pane="tools"]');
  const bar = pane.locator("> [data-kiki-bar]");
  const start = (await pane.boundingBox())!;
  const grip = (await bar.boundingBox())!;
  await page.mouse.move(grip.x + 30, grip.y + 10);
  await page.mouse.down();
  await page.mouse.move(grip.x + 330, grip.y + 210, { steps: 5 });
  await page.mouse.up();
  const moved = (await pane.boundingBox())!;
  assert(Math.abs(moved.x - (start.x + 300)) <= 2, `x ${moved.x}`);
  assert(Math.abs(moved.y - (start.y + 200)) <= 2, `y ${moved.y}`);
  assert(Math.abs(moved.width - start.width) <= 2, "keeps its width");
  await bar.focus();
  await page.keyboard.press("ArrowLeft");
  const nudged = (await pane.boundingBox())!;
  assert(Math.abs(nudged.x - (moved.x - 16)) <= 2);
  // A card can't be dragged out of reach.
  await page.mouse.move(nudged.x + 30, nudged.y + 10);
  await page.mouse.down();
  await page.mouse.move(-2000, -2000, { steps: 3 });
  await page.mouse.up();
  const kept = (await pane.boundingBox())!;
  assert(kept.y >= 0 && kept.x + kept.width >= 48);
});

test("chat has World and Agent rooms; the agent sends requests to Kiln", async () => {
  await open();
  await page.getByRole("tab", { name: "World" }).click();
  await page.getByText("World chat isn't open in this world yet.").waitFor();
  assert(await page.getByLabel("Message people in this world").isDisabled());
  await agent("Make the lamps glow green");
  assert.equal(
    await page.locator("#edit-kiln-request").inputValue(),
    "Make the lamps glow green",
  );
  assert((await clicks()).includes("edit-request-review"));
  const mine = page.locator(".kiki-msg.kiki-mine").last();
  assert.equal(
    await mine.locator(".kiki-txt").textContent(),
    "Make the lamps glow green",
  );
  assert.equal(await mine.locator(".kiki-who time").count(), 1);
  const side = await mine.evaluate((el) => getComputedStyle(el).alignSelf);
  assert.equal(side, "flex-end");
  const size = await mine
    .locator(".kiki-txt")
    .evaluate((el) => getComputedStyle(el).fontSize);
  assert.equal(size, "15px");
  // Kiln's answers show as its messages.
  await page.evaluate(() => {
    document.getElementById("edit-kiln-status")!.textContent =
      "Kiln is working on it.";
  });
  await page
    .locator(".kiki-msg", { hasText: "Kiln is working on it." })
    .waitFor();
  // A page with world chat opens the World room.
  await page.evaluate(() => {
    document.body.dataset.kikiWorldChat = "open";
    document.dispatchEvent(
      new CustomEvent("kiki:world-message", {
        detail: { name: "Ada", text: "Hi there" },
      }),
    );
  });
  await page.getByRole("tab", { name: "World" }).click();
  await page.locator(".kiki-msg", { hasText: "Hi there" }).waitFor();
  await page
    .getByLabel("Message people in this world")
    .waitFor({ state: "visible" });
  await page.waitForFunction(
    () =>
      !(
        document.querySelector(
          '[aria-label="Message people in this world"]',
        ) as HTMLInputElement
      ).disabled,
  );
});

test("characters speak in World chat and answer when named", async () => {
  await open();
  // A string, so the bundler's helpers don't enter the page.
  await page.evaluate(`
    window.asked = [];
    window.hauntDemo = { npc_chat: (text) => window.asked.push(text) };
    window.dispatchEvent(new CustomEvent("aethyr-npc", { detail: {
      sequence: 1, npc: "ghost", name: "Ghost", kind: "greet", text: "Who goes there?",
    } }));
  `);
  await page.getByRole("tab", { name: "World" }).click();
  await page.locator(".kiki-msg", { hasText: "Who goes there?" }).waitFor();
  const box = page.getByLabel("Message people in this world");
  await page.waitForFunction(
    () =>
      !(
        document.querySelector(
          '[aria-label="Message people in this world"]',
        ) as HTMLInputElement
      ).disabled,
  );
  await box.fill("@ghost hello");
  await box.press("Enter");
  await page.waitForFunction(
    () => (window as unknown as { asked: string[] }).asked.length === 1,
  );
  assert.deepEqual(
    await page.evaluate(() => (window as unknown as { asked: string[] }).asked),
    ["@ghost hello"],
  );
});

test("inventory adds an asset through the release's Add prop", async () => {
  await open();
  const inventory = page.locator('[data-kiki-pane="inventory"]');
  await inventory.getByText("Adds at 1, 0, 2").waitFor();
  await inventory.getByLabel("Find an asset").fill("barr");
  assert.equal(await inventory.locator(".kiki-assets li").count(), 1);
  await inventory.getByRole("button", { name: "Add Barrel" }).click();
  assert.equal(
    await page.locator("#edit-place-asset").inputValue(),
    "fantasy:barrel",
  );
  assert((await clicks()).includes("edit-place"));
  // Without the release's Add prop, nothing can be added.
  await page.evaluate(() =>
    document.getElementById("prop-tools")!.setAttribute("hidden", ""),
  );
  await inventory.getByText("Props can't be added here right now.").waitFor();
  assert(
    await inventory.getByRole("button", { name: "Add Barrel" }).isDisabled(),
  );
});

test("payouts show the recorded amounts, and 0 when nothing was paid", async () => {
  await open();
  const payouts = page.locator('[data-kiki-pane="payouts"]');
  await payouts.getByText("No changes yet.").waitFor();
  await payouts.getByText("Paid: 0").waitFor();
  await page.evaluate(() => {
    document.getElementById("visit-earnings")!.textContent =
      "Earnings: 2,100 msat recorded";
    const li = document.createElement("li");
    li.textContent = "Moved the lamp";
    document.getElementById("edit-history-entries")!.append(li);
  });
  await payouts.getByText("Earnings: 2,100 msat recorded").waitFor();
  await payouts.getByText("Moved the lamp").waitFor();
  // Edit mode keeps Play, Compile, Revert, and Publish, as the release offers them.
  const actions = page.locator('[data-kiki-pane="actions"]');
  assert(await actions.getByRole("button", { name: "Play" }).isDisabled());
  assert(await actions.getByRole("button", { name: "Revert" }).isDisabled());
  await actions.getByRole("button", { name: "Publish" }).click();
  await actions.getByRole("button", { name: "Compile" }).click();
  const pressed = await clicks();
  assert(
    pressed.includes("project-publish") && pressed.includes("request-compile"),
  );
});

test("a guest's second change asks for a revert", async () => {
  await open();
  // The first change goes to the house's confirmation as usual.
  await page.evaluate(() =>
    (document.getElementById("edit-auth") as HTMLDialogElement).show(),
  );
  await page.waitForTimeout(400);
  assert(
    await page.evaluate(
      () => (document.getElementById("edit-auth") as HTMLDialogElement).open,
    ),
  );
  assert.equal(await page.locator(".kiki-practice").count(), 0);
  await page.evaluate(() => {
    (document.getElementById("edit-auth") as HTMLDialogElement).close();
    const li = document.createElement("li");
    li.textContent = "Moved the lamp";
    document.getElementById("edit-history-entries")!.append(li);
  });
  await page.waitForTimeout(350);
  await page.evaluate(() =>
    (document.getElementById("edit-auth") as HTMLDialogElement).show(),
  );
  const box = page.getByRole("alertdialog", {
    name: "Revert to make another change",
  });
  await box.waitFor();
  assert(
    !(await page.evaluate(
      () => (document.getElementById("edit-auth") as HTMLDialogElement).open,
    )),
  );
  const revert = box.getByRole("button", { name: "Revert" });
  const color = await revert.evaluate(
    (el) => getComputedStyle(el).backgroundColor,
  );
  assert.equal(color, "rgb(232, 121, 43)");
  await box.getByRole("button", { name: "Keep this change" }).waitFor();
  await capture("practice");
  await revert.click();
  await box.waitFor({ state: "hidden" });
  assert((await clicks()).includes("edit-undo"));
});

test("without `kiki`, the house keeps its own look", async () => {
  await open(false);
  const style = await page
    .locator('[data-aethyr-id="world-voice"]')
    .evaluate((el) => getComputedStyle(el, "::before").display);
  assert.notEqual(style, "none");
});
