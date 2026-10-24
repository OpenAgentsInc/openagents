import assert from "node:assert/strict";
import test from "node:test";
import { EditorService } from "./service.ts";
import { editorAtoms } from "./state.ts";
import {
  INITIAL_STATUS,
  PREVIEW_PROTOCOL,
  isPreviewEvent,
  type PreviewMessage,
} from "./protocol.ts";

test("chat focus messages admit no payload or foreign session", () => {
  const message = {
    type: "mmorpgFocusChat",
    protocol: PREVIEW_PROTOCOL,
    sessionId: "active",
  };
  assert.equal(isPreviewEvent(message, "active"), true);
  assert.equal(isPreviewEvent(message, "other"), false);
  assert.equal(
    isPreviewEvent({ ...message, protocol: "future" }, "active"),
    false,
  );
  assert.equal(
    isPreviewEvent({ ...message, input: { attack: true } }, "active"),
    false,
  );
});

test("only the online preview can focus chat, and movement clears before UI notification", async () => {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window"),
    previousLocation = Object.getOwnPropertyDescriptor(globalThis, "location");
  const listeners = new Map<string, Set<(event: any) => void>>(),
    sent: PreviewMessage[] = [];
  const fakeWindow = {
    location: { origin: "http://editor.test" },
    addEventListener(type: string, callback: (event: any) => void) {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type)!.add(callback);
    },
    removeEventListener(type: string, callback: (event: any) => void) {
      listeners.get(type)?.delete(callback);
    },
  };
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: fakeWindow,
  });
  Object.defineProperty(globalThis, "location", {
    configurable: true,
    value: { pathname: "/" },
  });
  const contentWindow = {
    postMessage(message: PreviewMessage) {
      sent.push(message);
    },
  };
  const frame = {
    src: "",
    contentWindow,
    addEventListener() {},
    removeEventListener() {},
  } as unknown as HTMLIFrameElement;
  const service = new EditorService(frame, "http://preview.test/preview.html");
  try {
    service.start();
    await new Promise<void>((resolve) => setImmediate(resolve));
    const deliver = (
      patch: Record<string, unknown> = {},
      origin = "http://preview.test",
      source: unknown = contentWindow,
    ) => {
      const data = {
        protocol: PREVIEW_PROTOCOL,
        sessionId: service.sessionId,
        type: "mmorpgFocusChat",
        ...patch,
      };
      for (const listener of listeners.get("message") ?? [])
        listener({ data, origin, source });
    };
    let focused = 0;
    const unsubscribe = service.onMmorpgFocusChat(() => {
      focused++;
      const message = sent.at(-1)!;
      assert.equal(message.type, "input");
      if (message.type === "input")
        assert.deepEqual(message.input, {
          forward: 0,
          right: 0,
          up: 0,
          lookX: 0,
          lookY: 0,
          sprint: false,
          jump: false,
          attack: false,
          dance: false,
        });
    });
    service.registry.set(editorAtoms.status, {
      ...INITIAL_STATUS,
      mmorpg: { connection: "login", actors: [], events: [] },
    });
    deliver();
    assert.equal(focused, 0);
    service.registry.set(editorAtoms.status, {
      ...INITIAL_STATUS,
      mmorpg: { connection: "online", actors: [], events: [] },
    });
    deliver({}, "http://other.test");
    deliver({}, "http://preview.test", {});
    deliver({ sessionId: "old-session" });
    deliver({ input: { attack: true } });
    assert.equal(focused, 0);
    deliver();
    assert.equal(focused, 1);
    unsubscribe();
    deliver();
    assert.equal(focused, 1);
    service.onMmorpgFocusChat(() => {
      focused++;
    });
    service.close();
    deliver();
    assert.equal(focused, 1);
  } finally {
    service.close();
    await new Promise<void>((resolve) => setImmediate(resolve));
    if (previousWindow)
      Object.defineProperty(globalThis, "window", previousWindow);
    else Reflect.deleteProperty(globalThis, "window");
    if (previousLocation)
      Object.defineProperty(globalThis, "location", previousLocation);
    else Reflect.deleteProperty(globalThis, "location");
  }
});
