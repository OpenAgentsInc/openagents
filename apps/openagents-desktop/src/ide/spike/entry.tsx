import PierreWorkerUrl from "@pierre/diffs/worker/worker.js?worker&url";
import { getOrCreateWorkerPoolSingleton, terminateWorkerPoolSingleton } from "@pierre/diffs/worker";
import * as monaco from "monaco-editor";
import "monaco-editor/min/vs/editor/editor.main.css";
import "monaco-editor/esm/vs/language/css/monaco.contribution.js";
import "monaco-editor/esm/vs/language/html/monaco.contribution.js";
import "monaco-editor/esm/vs/language/json/monaco.contribution.js";
import "monaco-editor/esm/vs/language/typescript/monaco.contribution.js";
import CssWorkerUrl from "monaco-editor/esm/vs/language/css/css.worker?worker&url";
import EditorWorkerUrl from "monaco-editor/esm/vs/editor/editor.worker?worker&url";
import HtmlWorkerUrl from "monaco-editor/esm/vs/language/html/html.worker?worker&url";
import JsonWorkerUrl from "monaco-editor/esm/vs/language/json/json.worker?worker&url";
import TypeScriptWorkerUrl from "monaco-editor/esm/vs/language/typescript/ts.worker?worker&url";
import { createRoot } from "react-dom/client";

import {
  PierreDiffAdapter,
  PierreDiffCollectionAdapter,
  PierreReviewAdapter,
  type PierreDiffScaleObservation,
} from "../pierre-diffs-adapter.tsx";
import { ideReviewSourceFixtures } from "../review-fixture.ts";
import type { IdePackageSpikeSnapshot } from "../package-spike-contract.ts";
import {
  tokyoNightDesktopThemeProjection,
  tokyoNightMonacoThemeData,
} from "../tokyo-night-theme.ts";
import "./spike.css";

const query = new URLSearchParams(location.search);
const cycle = Math.max(0, Number.parseInt(query.get("cycle") ?? "0", 10) || 0);
const failureLabel = query.get("failWorker");
const rootNode = document.getElementById("ide-package-spike-root");

const workerUrls: Readonly<Record<string, string>> = {
  editor: EditorWorkerUrl,
  json: JsonWorkerUrl,
  css: CssWorkerUrl,
  scss: CssWorkerUrl,
  less: CssWorkerUrl,
  html: HtmlWorkerUrl,
  handlebars: HtmlWorkerUrl,
  razor: HtmlWorkerUrl,
  typescript: TypeScriptWorkerUrl,
  javascript: TypeScriptWorkerUrl,
};

const trackedWorkers = new Set<Worker>();
let createdWorkers = 0;
const createTrackedWorker = (url: string, label: string): Worker => {
  if (failureLabel === label) throw new Error(`injected-${label}-worker-failure`);
  const worker = new Worker(url, { type: "module", name: `oa-ide-spike-${label}` });
  createdWorkers += 1;
  trackedWorkers.add(worker);
  const terminate = worker.terminate.bind(worker);
  worker.terminate = () => {
    trackedWorkers.delete(worker);
    terminate();
  };
  return worker;
};

globalThis.MonacoEnvironment = {
  getWorker: (_workerId: string, label: string) =>
    createTrackedWorker(workerUrls[label] ?? workerUrls.editor!, label || "editor"),
};

const loadedUrls = (): ReadonlyArray<string> =>
  performance
    .getEntriesByType("resource")
    .map((entry) => entry.name)
    .sort();

const externalUrls = (): ReadonlyArray<string> =>
  loadedUrls().filter((url) => {
    try {
      return new URL(url).origin !== location.origin;
    } catch {
      return true;
    }
  });

let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let reactRoot: ReturnType<typeof createRoot> | null = null;
let disposed = false;
let scaleObservation: PierreDiffScaleObservation | null = null;
let lastReviewSourceClasses: IdePackageSpikeSnapshot["pierre"]["reviewSourceClasses"] = [];

const queryDeep = (root: Document | Element | ShadowRoot, selector: string): Element | null => {
  const direct = root.querySelector(selector);
  if (direct !== null) return direct;
  for (const element of root.querySelectorAll("*")) {
    if (element.shadowRoot !== null) {
      const nested = queryDeep(element.shadowRoot, selector);
      if (nested !== null) return nested;
    }
  }
  return null;
};

const queryFixtureDeep = (mode: "unified" | "split", selector: string): Element | null => {
  const fixture = document.querySelector(`[data-oa-diff-mode='${mode}']`);
  return fixture === null ? null : queryDeep(fixture, selector);
};

const snapshot = (
  phase: IdePackageSpikeSnapshot["phase"],
  languageWorkersReady: ReadonlyArray<"editor" | "json" | "css" | "html" | "typescript">,
  pierreWorkerInitialized: boolean,
): IdePackageSpikeSnapshot => {
  const observedSourceClasses = [...document.querySelectorAll<HTMLElement>("[data-oa-pierre-review]")]
    .map((element) => element.dataset.oaPierreReview)
    .filter((value): value is NonNullable<typeof value> => value !== undefined);
  if (observedSourceClasses.length > 0) {
    lastReviewSourceClasses = observedSourceClasses as IdePackageSpikeSnapshot["pierre"]["reviewSourceClasses"];
  }
  return ({
  schemaVersion: "openagents.desktop.ide-package-spike.v1",
  phase,
  cycle,
  themeId: tokyoNightDesktopThemeProjection.id,
  themeBeforeEditorPaint: true,
  monaco: {
    modelCount: monaco.editor.getModels().length,
    editorsCreated: editor === null ? 0 : 1,
    languageWorkersReady,
    failureLabel,
  },
  pierre: {
    rendered: document.querySelector("[data-oa-pierre-fixture]") !== null,
    unified: queryFixtureDeep("unified", "[data-code][data-unified]") !== null,
    split:
      queryFixtureDeep("split", "[data-code][data-deletions]") !== null &&
      queryFixtureDeep("split", "[data-code][data-additions]") !== null,
    annotation: document.querySelector("[data-oa-pierre-annotation='comment']") !== null,
    selectedRange: queryFixtureDeep("unified", "[data-selected-line]") !== null,
    workerInitialized: pierreWorkerInitialized,
    virtualized:
      scaleObservation !== null &&
      scaleObservation.renderedItems > 0 &&
      scaleObservation.renderedItems < scaleObservation.totalItems,
    scaleItems: scaleObservation?.totalItems ?? 0,
    renderedScaleItems: scaleObservation?.renderedItems ?? 0,
    reviewSourceClasses: lastReviewSourceClasses,
  },
  resources: {
    activeWorkers: trackedWorkers.size,
    createdWorkers,
    externalUrls: externalUrls(),
    loadedUrls: loadedUrls(),
  },
  domNodes: document.querySelectorAll("*").length,
  });
};

const waitForPaint = async (): Promise<void> => {
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
};

const dispose = async (
  languageWorkersReady: ReadonlyArray<"editor" | "json" | "css" | "html" | "typescript">,
  pierreWorkerInitialized: boolean,
): Promise<IdePackageSpikeSnapshot> => {
  if (!disposed) {
    disposed = true;
    reactRoot?.unmount();
    reactRoot = null;
    editor?.dispose();
    editor = null;
    for (const model of monaco.editor.getModels()) model.dispose();
    terminateWorkerPoolSingleton();
    for (const worker of [...trackedWorkers]) worker.terminate();
    rootNode?.replaceChildren();
    await waitForPaint();
  }
  const result = snapshot("disposed", languageWorkersReady, pierreWorkerInitialized);
  (globalThis as { __oaIdePackageSpike?: IdePackageSpikeSnapshot }).__oaIdePackageSpike = result;
  return result;
};

const run = async (): Promise<void> => {
  if (rootNode === null) throw new Error("fixture root missing");

  // Theme data lands before any Monaco model/editor or Pierre component exists.
  document.documentElement.dataset.oaTheme = tokyoNightDesktopThemeProjection.id;
  const palette = tokyoNightDesktopThemeProjection.effectNative;
  document.documentElement.style.setProperty("--oa-ide-bg", palette.background);
  document.documentElement.style.setProperty("--oa-ide-surface", palette.surface);
  document.documentElement.style.setProperty("--oa-ide-fg", palette.textPrimary);
  document.documentElement.style.setProperty("--oa-ide-muted", palette.textMuted);
  document.documentElement.style.setProperty("--oa-ide-accent", palette.accent);
  document.documentElement.style.setProperty("--oa-ide-border", palette.border);
  monaco.editor.defineTheme("openagents-tokyo-night", tokyoNightMonacoThemeData());

  const shell = document.createElement("section");
  shell.className = "oa-ide-spike";
  shell.innerHTML = `
    <header><strong>Tokyo Night package fixture</strong><span>CSP / offline / ASAR</span></header>
    <div class="oa-ide-spike-grid">
      <section aria-label="Monaco fixture"><h2>Monaco</h2><div id="oa-monaco-fixture"></div></section>
      <section aria-label="Pierre Diffs fixture"><h2>Pierre Diffs</h2><div id="oa-pierre-fixture" data-oa-pierre-fixture="true"></div></section>
    </div>`;
  rootNode.replaceChildren(shell);

  const editorNode = document.getElementById("oa-monaco-fixture");
  const pierreNode = document.getElementById("oa-pierre-fixture");
  if (editorNode === null || pierreNode === null) throw new Error("fixture hosts missing");

  const sourceModel = monaco.editor.createModel(
    "export const packageSpike: string = 'Tokyo Night'\n",
    "typescript",
    monaco.Uri.parse(`inmemory://openagents/ide-spike-${cycle}.ts`),
  );
  editor = monaco.editor.create(editorNode, {
    model: sourceModel,
    theme: "openagents-tokyo-night",
    automaticLayout: true,
    minimap: { enabled: false },
    accessibilitySupport: "on",
  });

  const patch = [
    "diff --git a/src/theme.ts b/src/theme.ts",
    "index 1111111..2222222 100644",
    "--- a/src/theme.ts",
    "+++ b/src/theme.ts",
    "@@ -1,2 +1,3 @@",
    "-export const theme = 'khala'",
    "+export const theme = 'tokyo-night'",
    "+export const vim = false",
    " export const editor = 'monaco'",
  ].join("\n");
  reactRoot = createRoot(pierreNode);
  const scalePatches = Array.from({ length: 200 }, (_, index) => {
    const name = `src/scale-${String(index).padStart(3, "0")}.ts`;
    return {
      fileRef: `ide.file.scale-${String(index).padStart(3, "0")}`,
      patch: [
        `diff --git a/${name} b/${name}`,
        "index 1111111..2222222 100644",
        `--- a/${name}`,
        `+++ b/${name}`,
        "@@ -1 +1 @@",
        `-export const value = ${index}`,
        `+export const value = ${index + 1}`,
      ].join("\n"),
    };
  });
  const reviewSources = ideReviewSourceFixtures();
  reactRoot.render(
    <div className="oa-ide-diff-pair">
      <div data-oa-diff-mode="unified">
        <PierreDiffAdapter
          projection={{
            schemaVersion: "openagents.desktop.pierre-diff-projection.v1",
            reviewRef: "ide.review.package-spike",
            fileRef: "ide.file.theme-ts",
            patch,
            mode: "unified",
            contextLines: 3,
            selection: { start: 1, side: "additions", end: 2, endSide: "additions" },
            annotations: [{ kind: "comment", side: "additions", lineNumber: 2, label: "Owned review annotation" }],
          }}
        />
      </div>
      <div data-oa-diff-mode="split">
        <PierreDiffAdapter
          projection={{
            schemaVersion: "openagents.desktop.pierre-diff-projection.v1",
            reviewRef: "ide.review.package-spike-split",
            fileRef: "ide.file.theme-ts",
            patch,
            mode: "split",
            contextLines: 3,
            selection: null,
            annotations: [],
          }}
        />
      </div>
      <div className="oa-ide-review-source-corpus" data-oa-review-source-corpus="true">
        {reviewSources.map((source) => <PierreReviewAdapter
          key={source.reviewRef}
          source={source}
          options={{
            mode: source._tag === "CandidateComparison" ? "split" : "unified",
            contextLines: 3,
            selection: null,
            annotations: source._tag === "DraftExternalConflict"
              ? [{ kind: "conflict", side: "additions", lineNumber: 1, label: "External change" }]
              : source._tag === "AgentProposal"
                ? [{ kind: "proposal_rationale", side: "additions", lineNumber: 1, label: "Fixture rationale" }]
                : [],
          }}
        />)}
      </div>
      <div className="oa-ide-scale-fixture" data-oa-pierre-scale="true">
        <PierreDiffCollectionAdapter
          projection={{
            schemaVersion: "openagents.desktop.pierre-diff-collection.v1",
            reviewRef: "ide.review.package-spike-scale",
            files: scalePatches,
            mode: "unified",
            contextLines: 3,
            viewportHeight: 220,
          }}
          onObservation={(observation) => {
            scaleObservation = observation;
          }}
        />
      </div>
    </div>,
  );

  const languageWorkersReady: Array<"editor" | "json" | "css" | "html" | "typescript"> = [];
  let pierreWorkerInitialized = false;
  try {
    const workerModels = [
      monaco.editor.createModel(
        "{}",
        "json",
        monaco.Uri.parse(`inmemory://openagents/${cycle}.json`),
      ),
      monaco.editor.createModel(
        "a{}",
        "css",
        monaco.Uri.parse(`inmemory://openagents/${cycle}.css`),
      ),
      monaco.editor.createModel(
        "<main></main>",
        "html",
        monaco.Uri.parse(`inmemory://openagents/${cycle}.html`),
      ),
    ];
    createTrackedWorker(workerUrls.editor!, "editor");
    languageWorkersReady.push("editor");
    // Monaco activates a language contribution only after a model is
    // encountered by a view; constructing an unattached model is not enough.
    editor.setModel(workerModels[0]!);
    await new Promise((resolve) => setTimeout(resolve, 25));
    const jsonWorker = await monaco.json.getWorker();
    await jsonWorker(workerModels[0]!.uri);
    languageWorkersReady.push("json");
    createTrackedWorker(workerUrls.css!, "css");
    languageWorkersReady.push("css");
    createTrackedWorker(workerUrls.html!, "html");
    languageWorkersReady.push("html");
    editor.setModel(sourceModel);
    await new Promise((resolve) => setTimeout(resolve, 25));
    const typeScriptWorker = await monaco.typescript.getTypeScriptWorker();
    await typeScriptWorker(sourceModel.uri);
    languageWorkersReady.push("typescript");

    const pool = getOrCreateWorkerPoolSingleton({
      poolOptions: {
        poolSize: 1,
        totalASTLRUCacheSize: 4,
        workerFactory: () => createTrackedWorker(PierreWorkerUrl, "pierre"),
      },
      highlighterOptions: {
        langs: ["typescript"],
        theme: "tokyo-night",
        preferredHighlighter: "shiki-js",
      },
    });
    await pool.initialize(["typescript"]);
    pierreWorkerInitialized = pool.isInitialized() && pool.isWorkingPool();
    await waitForPaint();
    const ready = snapshot(
      failureLabel === null ? "ready" : "expected_failure",
      languageWorkersReady,
      pierreWorkerInitialized,
    );
    (globalThis as { __oaIdePackageSpike?: IdePackageSpikeSnapshot }).__oaIdePackageSpike = ready;
    (
      globalThis as {
        __oaDisposeIdePackageSpike?: () => Promise<IdePackageSpikeSnapshot>;
      }
    ).__oaDisposeIdePackageSpike = () => dispose(languageWorkersReady, pierreWorkerInitialized);
  } catch (error) {
    const expected = failureLabel !== null;
    if (!expected) throw error;
    await dispose(languageWorkersReady, pierreWorkerInitialized);
    const failedBase = snapshot("expected_failure", languageWorkersReady, pierreWorkerInitialized);
    const failed = {
      ...failedBase,
      monaco: {
        ...failedBase.monaco,
        failureLabel: error instanceof Error ? error.message : String(error),
      },
    };
    (globalThis as { __oaIdePackageSpike?: IdePackageSpikeSnapshot }).__oaIdePackageSpike = failed;
  }
};

void run().catch((error) => {
  document.documentElement.dataset.oaIdePackageSpikeError =
    error instanceof Error ? error.message : String(error);
});
