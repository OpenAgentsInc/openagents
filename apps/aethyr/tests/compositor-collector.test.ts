import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
const { startCompositorTrace, browserPresentationInfo } = await import(
  new URL("../tools/compositor-collector.mjs", import.meta.url).href
);

class Session extends EventEmitter {
  calls: string[] = [];
  hanging: string | null = null;
  earlyClose = false;
  version = {
    product: "Chrome/155.0.8059.85",
    userAgent: "Mozilla/5.0 (Macintosh) Chrome/155",
  };
  command: string[] | null = ["--enable-automation"];
  async send(method: string) {
    this.calls.push(method);
    if (this.hanging === method) return new Promise(() => {});
    if (method === "Tracing.end")
      queueMicrotask(() =>
        this.earlyClose
          ? this.emit("close")
          : this.emit("Tracing.tracingComplete", {
              stream: "private-stream",
              dataLossOccurred: false,
            }),
      );
    if (method === "IO.read")
      return {
        data: Buffer.from([0, 255, 1, 128]).toString("base64"),
        base64Encoded: true,
        eof: true,
      };
    if (method === "Browser.getVersion") return this.version;
    if (method === "Browser.getBrowserCommandLine") {
      if (this.command === null) throw Error("Unavailable");
      return { arguments: this.command };
    }
    return {};
  }
  async detach() {
    this.calls.push("detach");
    this.emit("close");
  }
}
const browser = (session: Session) => ({
  newBrowserCDPSession: async () => session,
});
// No browser, GPU, account, or product assets are used by these transport mocks.
test("binary trace collection closes its stream and session", async () => {
  const dir = await mkdtemp(join(tmpdir(), "af-compositor-mock-"));
  try {
    const session = new Session(),
      path = join(dir, "trace");
    const recording = await startCompositorTrace(browser(session), path);
    const result = await recording.stop();
    assert.deepEqual(await readFile(path), Buffer.from([0, 255, 1, 128]));
    assert.equal(result.bytes, 4);
    assert.equal(result.dataLossOccurred, false);
    assert.equal(
      session.calls.filter((value) => value === "IO.close").length,
      1,
    );
    assert.equal(session.calls.filter((value) => value === "detach").length, 1);
    await assert.rejects(recording.stop(), /only once/);
    await recording.abort();
    assert.equal(session.calls.filter((value) => value === "detach").length, 1);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("file-open failure and CDP close still dispose tracing resources", async () => {
  const dir = await mkdtemp(join(tmpdir(), "af-compositor-mock-"));
  try {
    const path = join(dir, "trace");
    await writeFile(path, "preserve");
    const session = new Session(),
      recording = await startCompositorTrace(browser(session), path);
    await assert.rejects(recording.stop(), /EEXIST/);
    assert.ok(session.calls.includes("IO.close"));
    assert.ok(session.calls.includes("detach"));
    assert.equal(await readFile(path, "utf8"), "preserve");
    const closed = new Session();
    closed.earlyClose = true;
    const broken = await startCompositorTrace(
      browser(closed),
      join(dir, "other"),
      { timeoutMs: 100 },
    );
    await assert.rejects(broken.stop(), /closed before flushing/);
    assert.ok(closed.calls.includes("detach"));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("end/read stalls are bounded and abort closes an unconsumed stream", async () => {
  const dir = await mkdtemp(join(tmpdir(), "af-compositor-mock-"));
  try {
    for (const method of ["Tracing.end", "IO.read"]) {
      const session = new Session();
      session.hanging = method;
      const recording = await startCompositorTrace(
        browser(session),
        join(dir, method),
        { timeoutMs: 100 },
      );
      const started = Date.now();
      await assert.rejects(recording.stop(), /timed out/);
      assert.ok(Date.now() - started < 1000);
      assert.ok(session.calls.includes("detach"));
      if (method === "IO.read") assert.ok(session.calls.includes("IO.close"));
    }
    const session = new Session();
    const recording = await startCompositorTrace(
      browser(session),
      join(dir, "abort"),
    );
    await recording.abort();
    assert.ok(session.calls.includes("IO.close"));
    assert.ok(session.calls.includes("detach"));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("headless mode is explicit and unknown command lines stay unknown", async () => {
  const headed = new Session();
  assert.equal(
    (await browserPresentationInfo(browser(headed))).headless,
    false,
  );
  const headless = new Session();
  headless.command = ["--headless=new", "--user-data-dir=private"];
  const result = await browserPresentationInfo(browser(headless));
  assert.equal(result.headless, true);
  assert.ok(!JSON.stringify(result).includes("private"));
  const unknown = new Session();
  unknown.command = null;
  assert.equal(
    (await browserPresentationInfo(browser(unknown))).headless,
    null,
  );
});

test("same-process preview uses only the explicit parent-session fallback and exact child canvas", async () => {
  const { collectCanvasIdentity } = await import(
    new URL("../tools/compositor-collector.mjs", import.meta.url).href
  );
  const dir = await mkdtemp(join(tmpdir(), "af-compositor-mock-"));
  const previewBase = "http://127.0.0.1:55335/preview.html";
  const previewFragment =
    "#session=owned-probe&editorOrigin=http%3A%2F%2F127.0.0.1%3A55334";
  const previewURL = previewBase + previewFragment;
  const selectedFrame = { url: () => previewURL };
  const sharedError =
    "This frame does not have a separate CDP session, it is a part of the parent frame's session";
  const doc = {
    nodeName: "#document",
    nodeId: 1,
    backendNodeId: 10,
    children: [
      {
        nodeName: "CANVAS",
        nodeId: 2,
        backendNodeId: 999,
        attributes: ["id", "world"],
      },
      {
        nodeName: "IFRAME",
        nodeId: 3,
        backendNodeId: 20,
        frameId: "editor-id",
        attributes: ["id", "preview-frame"],
        contentDocument: {
          nodeName: "#document",
          nodeId: 4,
          backendNodeId: 21,
          children: [
            {
              nodeName: "CANVAS",
              nodeId: 5,
              backendNodeId: 42,
              attributes: ["id", "world"],
            },
          ],
        },
      },
    ],
  };
  const make = (
    error = sharedError,
    url = previewBase,
    ownerBackendNodeId = 20,
  ) => {
    const session = new Session(),
      calls: unknown[] = [];
    const send = session.send.bind(session);
    (session as any).send = async (
      method: string,
      params?: { frameId?: string },
    ) => {
      if (method === "DOM.getDocument") return { root: doc };
      if (method === "Page.getFrameTree")
        return {
          frameTree: {
            frame: { id: "editor-id", url: "http://127.0.0.1:55334/" },
            childFrames: [
              {
                frame: { id: "preview-id", url, urlFragment: previewFragment },
              },
            ],
          },
        };
      if (method === "DOM.getFrameOwner") {
        assert.equal(params?.frameId, "preview-id");
        return { backendNodeId: ownerBackendNodeId };
      }
      if (method === "LayerTree.enable")
        session.emit("LayerTree.layerTreeDidChange", {
          layers: [
            { backendNodeId: 42, layerId: "correct-layer" },
            { backendNodeId: 999, layerId: "wrong-layer" },
          ],
        });
      return send(method);
    };
    const page = {
      locator: () => ({
        elementHandle: async () => ({
          contentFrame: async () => selectedFrame,
        }),
      }),
      context: () => ({
        newCDPSession: async (target: unknown) => {
          calls.push(target);
          if (target === selectedFrame) throw Error(error);
          return session;
        },
      }),
      waitForTimeout: async () => {},
    };
    return { session, page, calls };
  };
  try {
    const shared = make();
    const identity = await collectCanvasIdentity({
      browser: browser(new Session()),
      page: shared.page,
      directory: dir,
    });
    assert.deepEqual(shared.calls, [selectedFrame, shared.page]);
    assert.equal(identity.backendNodeId, 42);
    assert.equal(identity.frameId, "preview-id");
    assert.equal(identity.frameURL, previewURL);
    assert.equal(identity.frameOwnerBackendNodeId, 20);
    assert.equal(identity.cdpSessionScope, "shared-page");
    assert.equal(identity.ownerFrameField, "editor-id");
    assert.equal(identity.frameDocumentBackendNodeId, 21);
    assert.deepEqual(identity.layerIds, ["correct-layer"]);
    const unrelated = make("Target closed");
    await assert.rejects(
      collectCanvasIdentity({
        browser: browser(new Session()),
        page: unrelated.page,
        directory: dir,
      }),
      /Target closed/,
    );
    assert.deepEqual(unrelated.calls, [selectedFrame]);
    const wrong = make(sharedError, "http://127.0.0.1:55335/foreign.html");
    await assert.rejects(
      collectCanvasIdentity({
        browser: browser(new Session()),
        page: wrong.page,
        directory: dir,
      }),
      /No bounded FrameTree URL match/,
    );
    assert.ok(wrong.session.calls.includes("detach"));
    const foreignOwner = make(sharedError, previewBase, 999);
    await assert.rejects(
      collectCanvasIdentity({
        browser: browser(new Session()),
        page: foreignOwner.page,
        directory: dir,
      }),
      /no unique matching DOM frame owner/,
    );
    assert.ok(foreignOwner.session.calls.includes("detach"));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
