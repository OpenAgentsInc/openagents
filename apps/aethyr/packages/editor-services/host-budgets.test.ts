import { test } from "node:test";
import assert from "node:assert/strict";
import {
  HOST_ACTION_BYTES,
  HOST_FRAME_BYTES,
  AUTHORIZATION_BODY_BYTES,
  checkAuthorFrame,
} from "./host-budgets.ts";
test("eight material image pairs fit through authoring wrappers; oversized frames are refused", () => {
  const action = {
    type: "restore_material_project",
    document: "d".repeat(48 * 1024),
    materials: Array.from({ length: 8 }, (_, i) => ({
      target: `wolf-${i}`,
      generated: "p".repeat(21848),
      handmade: "p".repeat(21848),
    })),
  };
  const frame = {
    protocol: "aethyrforge.preview.v1",
    sessionId: "s".repeat(128),
    requestId: "r".repeat(128),
    type: "author",
    base: 0,
    commandId: "c".repeat(128),
    action,
  };
  assert.ok(JSON.stringify(action).length > 65536);
  checkAuthorFrame(action, frame);
  assert.equal(HOST_ACTION_BYTES, 516096);
  assert.equal(HOST_FRAME_BYTES, 524288);
  assert.equal(AUTHORIZATION_BODY_BYTES, 540672);
  assert.throws(
    () =>
      checkAuthorFrame(action, {
        ...frame,
        padding: "x".repeat(HOST_FRAME_BYTES),
      }),
    /too large/,
  );
  assert.throws(
    () => checkAuthorFrame({ padding: "x".repeat(HOST_ACTION_BYTES) }, frame),
    /too large/,
  );
});

test("refused oversized author messages do not enter the service queue or reach the preview", async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: { location: { origin: "https://editor.example" } },
  });
  const { EditorService } = await import("./service.ts");
  const { editorAtoms } = await import("./state.ts");
  const posts: unknown[] = [];
  const frame = {
    src: "",
    contentWindow: {
      postMessage(message: unknown) {
        posts.push(message);
      },
    },
  } as unknown as HTMLIFrameElement;
  const service = new EditorService(frame, "https://preview.example");
  try {
    const action = {
      type: "restore_material_project",
      document: "d".repeat(48 * 1024),
      materials: Array.from({ length: 8 }, () => ({
        generated: "p".repeat(21848),
        handmade: "p".repeat(21848),
      })),
    };
    const request = {
      type: "author",
      action,
      base: 0,
      commandId: "restore-large",
    } as any;
    service.send(request, true);
    assert.equal(posts.length, 1);
    assert.equal(service.registry.get(editorAtoms.pending), 1);
    assert.throws(
      () =>
        service.send(
          {
            ...request,
            action: { ...action, padding: "x".repeat(HOST_ACTION_BYTES) },
          },
          true,
        ),
      /too large/,
    );
    assert.equal(posts.length, 1);
    assert.equal(service.registry.get(editorAtoms.pending), 1);
    service.send({ ...request, commandId: "next-valid" }, true);
    assert.equal(posts.length, 2);
    assert.equal(service.registry.get(editorAtoms.pending), 2);
  } finally {
    service.close();
    if (previous) Object.defineProperty(globalThis, "window", previous);
    else Reflect.deleteProperty(globalThis, "window");
  }
});
