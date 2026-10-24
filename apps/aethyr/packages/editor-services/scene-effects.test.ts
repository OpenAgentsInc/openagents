import assert from "node:assert/strict";
import test from "node:test";
import { EditorService } from "./service.ts";
import { DEFAULT_SCENE_EFFECTS } from "../runtime/scene-effects.ts";
import type { PreviewMessage } from "./protocol.ts";
import { editorAtoms } from "./state.ts";

function withService(
  path: string,
  run: (service: EditorService, sent: PreviewMessage[]) => void,
) {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  const previousLocation = Object.getOwnPropertyDescriptor(
    globalThis,
    "location",
  );
  const sent: PreviewMessage[] = [];
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: { location: { origin: "http://127.0.0.1:55340" } },
  });
  Object.defineProperty(globalThis, "location", {
    configurable: true,
    value: { pathname: path },
  });
  const frame = {
    src: "",
    focus() {},
    contentWindow: {
      postMessage(message: PreviewMessage) {
        sent.push(message);
      },
    },
  } as unknown as HTMLIFrameElement;
  const service = new EditorService(
    frame,
    "http://127.0.0.1:55341/preview.html",
  );
  try {
    run(service, sent);
  } finally {
    service.close();
    if (previousWindow)
      Object.defineProperty(globalThis, "window", previousWindow);
    else Reflect.deleteProperty(globalThis, "window");
    if (previousLocation)
      Object.defineProperty(globalThis, "location", previousLocation);
    else Reflect.deleteProperty(globalThis, "location");
  }
}

test("effect controls use the versioned preview command and receipt path", () => {
  withService("/", (service, sent) => {
    const settings = structuredClone(DEFAULT_SCENE_EFFECTS);
    settings.wind.speed = 8;
    const id = service.sceneEffects(settings);
    assert.equal(sent.length, 1);
    assert.equal(sent[0].requestId, id);
    assert.equal(sent[0].sessionId, service.sessionId);
    assert.equal(sent[0].type, "setSceneEffects");
    assert.equal(
      service.registry.get(editorAtoms.sceneEffects).wind.speed,
      4,
      "Sending an edit does not claim it has activated.",
    );
    if (sent[0].type !== "setSceneEffects") throw Error("Missing effect edit.");
    settings.grass.bladeHeight = 1.5;
    assert.equal(
      sent[0].settings.grass.bladeHeight,
      DEFAULT_SCENE_EFFECTS.grass.bladeHeight,
    );
    const preview = service.previewImpact();
    assert.equal(sent[1].type, "previewImpact");
    assert.equal(sent[1].requestId, preview);
    service.settle(id);
    service.settle(preview);
  });
});

test("effect changes bound in-flight receipts and recover after a request settles", () => {
  withService("/", (service, sent) => {
    const ids = Array.from({ length: 16 }, () =>
      service.sceneEffects(DEFAULT_SCENE_EFFECTS),
    );
    assert.throws(
      () => service.sceneEffects(DEFAULT_SCENE_EFFECTS),
      /current changes/,
    );
    assert.throws(() => service.previewImpact(), /current changes/);
    assert.equal(sent.length, 16);
    service.settle(ids[0]);
    service.previewImpact();
    assert.equal(sent.length, 17);
  });
});

test("invalid candidates do not send partial edits or consume pending requests", () => {
  withService("/", (service, sent) => {
    assert.throws(() =>
      service.sceneEffects({
        ...DEFAULT_SCENE_EFFECTS,
        grass: { ...DEFAULT_SCENE_EFFECTS.grass, bladeWidth: NaN },
      }),
    );
    assert.equal(sent.length, 0);
  });
});

test("a published player cannot request live edits or cosmetic preview actions", () => {
  withService("/player.html", (service, sent) => {
    assert.throws(
      () => service.sceneEffects(DEFAULT_SCENE_EFFECTS),
      /Open the editor/,
    );
    assert.throws(() => service.previewImpact(), /Open the editor/);
    assert.equal(sent.length, 0);
  });
});
