import assert from "node:assert/strict";
import test from "node:test";
import { EditorService } from "./service.ts";
import { editorAtoms } from "./state.ts";
import {
  canEditMmorpg,
  validMmorpgEditingRequest,
  INITIAL_STATUS,
  PREVIEW_PROTOCOL,
  isPreviewEvent,
  type PreviewMessage,
} from "./protocol.ts";

test("local editing is unavailable to public, player, and read-only pages", () => {
  assert.equal(canEditMmorpg(false, "/"), true);
  assert.equal(canEditMmorpg(false, "/editor.html"), true);
  assert.equal(canEditMmorpg(true, "/"), false);
  assert.equal(canEditMmorpg(false, "/player.html"), false);
  assert.equal(canEditMmorpg(false, "/build/player.html"), false);
  assert.equal(canEditMmorpg(false, "/", true), false);
});

test("editing requests admit only a boolean and the existing bridge envelope", () => {
  for (const editing of [false, true]) {
    assert.equal(
      validMmorpgEditingRequest({ type: "mmorpgEditing", editing }),
      true,
    );
    assert.equal(
      validMmorpgEditingRequest({
        type: "mmorpgEditing",
        editing,
        protocol: PREVIEW_PROTOCOL,
        sessionId: "current",
        requestId: "request",
      }),
      true,
    );
  }
  for (const value of [
    null,
    [],
    {},
    { type: "mmorpgEditing" },
    { type: "mmorpgEditing", editing: "true" },
    { type: "mmorpgEditing", editing: 1 },
    { type: "mmorpgEditing", editing: true, camera: [0, 0, 0] },
    { type: "mmorpgEditing", editing: false, protocol: "other" },
    { type: "mmorpgEditing", editing: false, sessionId: "" },
  ])
    assert.equal(validMmorpgEditingRequest(value), false);
});

test("editing status is accepted only from a matching session with a boolean", () => {
  const event = {
    type: "status",
    protocol: PREVIEW_PROTOCOL,
    sessionId: "current",
    status: { ...INITIAL_STATUS, mmorpgEditing: true },
  };
  assert.equal(isPreviewEvent(event, "current"), true);
  assert.equal(isPreviewEvent(event, "other"), false);
  assert.equal(
    isPreviewEvent(
      { ...event, status: { ...event.status, mmorpgEditing: "true" } },
      "current",
    ),
    false,
  );
});

test("the service sends a tracked mode request without changing position, camera or acknowledged state", () => {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  const previousLocation = Object.getOwnPropertyDescriptor(
    globalThis,
    "location",
  );
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: { location: { origin: "http://editor.test" } },
  });
  const localLocation = { pathname: "/" };
  Object.defineProperty(globalThis, "location", {
    configurable: true,
    value: localLocation,
  });
  const sent: PreviewMessage[] = [];
  const frame = {
    src: "",
    contentWindow: { postMessage: (m: PreviewMessage) => sent.push(m) },
  } as unknown as HTMLIFrameElement;
  const service = new EditorService(frame, "http://preview.test/preview.html");
  try {
    const original = {
      ...INITIAL_STATUS,
      camera: [6, 3, 20] as [number, number, number],
      mmorpg: { connection: "online" as const, actors: [], events: [] },
    };
    service.registry.set(editorAtoms.status, original);
    const requestId = service.mmorpgEditing(true);
    assert.deepEqual(sent, [
      {
        type: "mmorpgEditing",
        editing: true,
        protocol: PREVIEW_PROTOCOL,
        sessionId: service.sessionId,
        requestId,
      },
    ]);
    assert.equal(service.registry.get(editorAtoms.pending), 1);
    assert.equal(service.registry.get(editorAtoms.status), original);
    assert.equal(
      service.registry.get(editorAtoms.status).mmorpgEditing,
      undefined,
    );
    service.settle(requestId);
    localLocation.pathname = "/player.html";
    assert.throws(() => service.mmorpgEditing(true), /Open the editor/);
    localLocation.pathname = "/";
    assert.throws(
      () => service.mmorpgEditing("true" as unknown as boolean),
      /Play or Edit/,
    );
    service.registry.set(editorAtoms.status, {
      ...original,
      mmorpg: undefined,
    });
    assert.throws(() => service.mmorpgEditing(true), /Enter the world/);
    assert.equal(sent.length, 1);
  } finally {
    service.close();
    if (previousWindow)
      Object.defineProperty(globalThis, "window", previousWindow);
    else Reflect.deleteProperty(globalThis, "window");
    if (previousLocation)
      Object.defineProperty(globalThis, "location", previousLocation);
    else Reflect.deleteProperty(globalThis, "location");
  }
});
