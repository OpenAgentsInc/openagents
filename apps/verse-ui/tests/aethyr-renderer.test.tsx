// Renderer snapshots for every node kind at 390 px and 1,440 px, the intents
// the DOM sends back, and the entry demo running in a browser.
//
// The browser half loads the built stylesheet
// (crates/openagents-web/content/verse-ui/ui.css), so run `npm run build`
// first. Set AETHYR_CHROMIUM to a Chromium binary when Playwright's own is
// not installed, AETHYR_CAPTURE_DIR to keep screenshots, and
// UPDATE_SNAPSHOTS=1 to rewrite tests/snapshots/aethyr-renderer.json.
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { renderToStaticMarkup } from "react-dom/server";
import { build } from "esbuild";
import { chromium, type Browser, type Page } from "playwright";
import { AethyrView } from "../src/aethyr/renderer";
import { ELEMENT_KINDS, type Intent, type View } from "../src/aethyr/protocol";

const here = import.meta.dirname;
const kindsDir = join(here, "../../../crates/aethyr-ui/fixtures/kinds");
const cssFile = join(
  here,
  "../../../crates/openagents-web/content/verse-ui/ui.css",
);
const snapshotFile = join(here, "snapshots/aethyr-renderer.json");
const update = process.env.UPDATE_SNAPSHOTS === "1";
const captures = process.env.AETHYR_CAPTURE_DIR;
const WIDTHS = [390, 1440] as const;

const fixture = (kind: string): View =>
  JSON.parse(readFileSync(join(kindsDir, `${kind}.json`), "utf8"));

type Snapshot = Record<string, Record<string, unknown>>;
const actual: Snapshot = {};
const record = (kind: string, key: string, value: unknown) => {
  (actual[kind] ??= {})[key] = value;
};

let browser: Browser;
let page: Page;

before(async () => {
  const bundle = await build({
    entryPoints: [join(here, "aethyr/harness.tsx")],
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
  const script = bundle.outputFiles[0]!.text;
  browser = await chromium.launch({
    executablePath: process.env.AETHYR_CHROMIUM || undefined,
  });
  page = await browser.newPage({ viewport: { width: 1440, height: 1600 } });
  await page.setContent(
    `<!doctype html><html lang="en" class="dark" data-verse-ui="aethyr"><head>` +
      `<meta charset="utf-8"><style>${readFileSync(cssFile, "utf8")}</style></head>` +
      `<body><main id="root" style="padding:16px"></main></body></html>`,
  );
  await page.addScriptTag({ content: script });
  if (captures) mkdirSync(captures, { recursive: true });
});

after(async () => {
  await browser?.close();
  const expected = existsSync(snapshotFile)
    ? (JSON.parse(readFileSync(snapshotFile, "utf8")) as Snapshot)
    : {};
  if (update) {
    mkdirSync(join(here, "snapshots"), { recursive: true });
    writeFileSync(snapshotFile, JSON.stringify(actual, null, 2) + "\n");
    return;
  }
  for (const kind of Object.keys(actual))
    assert.deepEqual(
      JSON.parse(JSON.stringify(actual[kind])),
      expected[kind],
      `${kind} differs from tests/snapshots/aethyr-renderer.json (UPDATE_SNAPSHOTS=1 rewrites it)`,
    );
});

/** Layout facts that depend on width but not on installed fonts. */
async function layout(width: number) {
  await page.setViewportSize({ width, height: 1600 });
  return page.evaluate(() => {
    const sized = new Set([
      "panel",
      "stack",
      "list",
      "field",
      "sheet",
      "dialog",
      "media_slot",
      "engine_surface",
    ]);
    return [...document.querySelectorAll<HTMLElement>("[data-aethyr-id]")].map(
      (el) => {
        const style = getComputedStyle(el);
        const kind = el.dataset.aethyrKind!;
        const box = el.getBoundingClientRect();
        return {
          id: el.dataset.aethyrId,
          tag: el.tagName.toLowerCase(),
          role: el.getAttribute("role") ?? undefined,
          display: style.display,
          position: style.position,
          ...(style.display.includes("flex") && {
            direction: style.flexDirection,
            wrap: style.flexWrap,
          }),
          visible: el.checkVisibility(),
          ...(sized.has(kind) &&
            el.checkVisibility() && { width: Math.round(box.width) }),
        };
      },
    );
  });
}

for (const kind of ELEMENT_KINDS)
  test(`renders ${kind} at 390 and 1440 px`, async () => {
    const view = fixture(kind);
    record(
      kind,
      "markup",
      renderToStaticMarkup(<AethyrView view={view} onIntent={() => {}} />),
    );
    await page.evaluate((v) => window.harness.render(v), view);
    for (const width of WIDTHS) {
      record(kind, String(width), await layout(width));
      if (captures)
        await page.screenshot({
          path: join(captures, `${kind}-${width}.png`),
          fullPage: true,
          animations: "disabled",
        });
    }
    // Every node in the tree is in the DOM, under its own ID.
    const ids = await page.evaluate(() =>
      [...document.querySelectorAll("[data-aethyr-id]")].map((el) =>
        el.getAttribute("data-aethyr-id"),
      ),
    );
    assert.equal(new Set(ids).size, ids.length);
  });

test("every fixture has a snapshot", () => {
  const files = readdirSync(kindsDir).map((f) => f.replace(/\.json$/, ""));
  assert.deepEqual(files.sort(), [...ELEMENT_KINDS].sort());
});

async function intentsAfter(view: View, act: () => Promise<void>) {
  await page.setViewportSize({ width: 1440, height: 1600 });
  await page.evaluate((v) => {
    window.harness.intents.length = 0;
    window.harness.render(v);
  }, view);
  await act();
  return page.evaluate(() => window.harness.intents.slice()) as Promise<
    Intent[]
  >;
}

const strip = (intents: Intent[]) =>
  intents.map(({ node, kind, value }) =>
    value === undefined ? { node, kind } : { node, kind, value },
  );

test("the DOM sends typed intents and never changes state itself", async () => {
  const buttons = fixture("button");
  assert.deepEqual(
    strip(
      await intentsAfter(buttons, async () => {
        await page.getByRole("button", { name: "Continue as guest" }).click();
        await page.getByRole("link", { name: "Back to OpenAgents" }).click();
        await page
          .getByRole("button", { name: "Create my planet" })
          .click({ force: true });
      }),
    ),
    [
      { node: "button-primary", kind: "activate" },
      { node: "button-link", kind: "activate" },
    ],
  );
  // A plain click on a link is the controller's to handle: no navigation.
  assert.equal(new URL(page.url()).pathname, "blank");

  const chips = fixture("chip");
  const chipIntents = await intentsAfter(chips, async () => {
    await page.getByRole("button", { name: "Blue Rush" }).click();
  });
  assert.deepEqual(strip(chipIntents), [
    { node: "chip-blue-rush", kind: "activate" },
  ]);
  assert.equal(chipIntents[0]!.revision, chips.revision);
  assert.equal(chipIntents[0]!.instance, chips.instance);
  // The renderer did not select the chip; only a new tree can.
  assert.equal(
    await page
      .getByRole("button", { name: "Blue Rush" })
      .getAttribute("aria-pressed"),
    "false",
  );

  const list = fixture("list");
  assert.deepEqual(
    strip(
      await intentsAfter(list, async () => {
        await page.getByRole("option", { name: "Blue Rush" }).click();
        await page
          .getByRole("option", { name: "Haunted House" })
          .press("ArrowDown");
        await page.keyboard.press("Enter");
      }),
    ),
    [
      { node: "list", kind: "select", value: "list-blue-rush" },
      { node: "list", kind: "select", value: "list-blue-rush" },
    ],
  );

  const fields = fixture("field");
  assert.deepEqual(
    strip(
      await intentsAfter(fields, async () => {
        const name = page.getByLabel("Planet name");
        await name.press("End");
        await name.press("s");
        await name.press("Enter");
      }),
    ),
    [
      { node: "field-name", kind: "input", value: "Kikis" },
      { node: "field-name", kind: "submit" },
    ],
  );

  const dialog = fixture("dialog");
  assert.deepEqual(
    strip(
      await intentsAfter(dialog, async () => {
        assert.equal(
          await page.locator("dialog").evaluate((d) => d.matches(":modal")),
          true,
        );
        await page.keyboard.press("Escape");
        await page
          .getByRole("button", { name: "Close Your next world is waiting." })
          .click();
      }),
    ),
    [
      { node: "dialog", kind: "dismiss" },
      { node: "dialog", kind: "dismiss" },
    ],
  );
  // Escape asked; the controller decides. The dialog is still open.
  assert.equal(
    await page.locator("dialog").evaluate((d) => (d as HTMLDialogElement).open),
    true,
  );

  const toast = fixture("toast");
  assert.deepEqual(
    strip(
      await intentsAfter(toast, async () => {
        await page.getByRole("button", { name: "Close message" }).click();
      }),
    ),
    [{ node: "toast-danger", kind: "dismiss" }],
  );
});

test("the entry demo runs its controller in the browser", async () => {
  for (const width of WIDTHS) {
    await page.setViewportSize({ width, height: 900 });
    await page.evaluate(async () => {
      window.harness.trail.length = 0;
      await window.harness.entry({
        accounts: true,
        hauntPath: "/haunted-house",
      });
    });
    const dialog = page.locator("dialog[data-aethyr-id=verse-entry]");
    assert.equal(await dialog.evaluate((d) => d.matches(":modal")), true);
    await page
      .getByRole("heading", { name: "Your next world is waiting." })
      .waitFor();
    if (captures)
      await page.screenshot({
        path: join(captures, `entry-account-${width}.png`),
        animations: "disabled",
      });

    await page.getByRole("button", { name: "Continue as guest" }).click();
    await page.getByRole("heading", { name: "Where shall we go?" }).waitFor();
    assert.equal(
      await page.getByRole("button", { name: "Continue as guest" }).count(),
      0,
    );
    if (captures)
      await page.screenshot({
        path: join(captures, `entry-choice-${width}.png`),
        animations: "disabled",
      });

    await page.getByRole("button", { name: "Explore the planets" }).click();
    await page.waitForFunction(
      () =>
        !(
          document.querySelector(
            "dialog[data-aethyr-id=verse-entry]",
          ) as HTMLDialogElement
        ).open,
    );
    if (captures)
      await page.screenshot({
        path: join(captures, `entry-closed-${width}.png`),
        animations: "disabled",
      });
    assert.deepEqual(await page.evaluate(() => window.harness.trail.slice()), [
      "guest",
    ]);
    const revision = await page
      .locator("[data-aethyr-root]")
      .getAttribute("data-aethyr-revision");
    assert.equal(revision, "3");
  }
});
