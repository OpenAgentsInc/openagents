import readline from "node:readline";

const mode = process.argv[2] ?? "normal";
const rl = readline.createInterface({ input: process.stdin });
const write = (value, ending = "\n") => process.stdout.write(`${JSON.stringify(value)}${ending}`);
const pendingReverse = new Map();
let reverseRootId = null;

if (mode === "malformed") setTimeout(() => process.stdout.write("not-json\n"), 10);
if (mode === "invalid-then-reverse") {
  setTimeout(
    () =>
      process.stdout.write(
        `{"jsonrpc":"1.0"}\n${JSON.stringify({
          jsonrpc: "2.0",
          id: "must-not-run",
          method: "fs/read_text_file",
          params: { sessionId: "s-1", path: "/tmp/input" },
        })}\n`,
      ),
    10,
  );
}
if (mode === "forced-malformed") setTimeout(() => process.stdout.write("not-json\n"), 10);
if (mode === "binary") setTimeout(() => process.stdout.write(Buffer.from([0xff, 0xfe, 0x0a])), 10);
if (mode === "sized" || mode === "partial") {
  const target = Number(process.argv[3]);
  const prefix = '{"jsonrpc":"2.0","method":"test/sized","params":{"pad":"';
  const suffix = '"}}';
  const line = `${prefix}${"x".repeat(target - Buffer.byteLength(prefix) - Buffer.byteLength(suffix))}${suffix}`;
  setTimeout(() => process.stdout.write(mode === "sized" ? `${line}\n` : line), 10);
}
if (mode === "burst") {
  const count = Number(process.argv[3]);
  setTimeout(
    () =>
      process.stdout.write(
        Array.from({ length: count }, (_, index) =>
          JSON.stringify({ jsonrpc: "2.0", method: "test/burst", params: { index } }),
        ).join("\n") + "\n",
      ),
    10,
  );
}
if (mode === "blank-burst") {
  setTimeout(() => process.stdout.write("\n".repeat(Number(process.argv[3]))), 10);
}
if (mode === "stderr") {
  process.stderr.write(
    "XAI_API_KEY=xai-secret-value token=cursor-login-token prompt=private-file-content\n",
  );
}
if (mode === "split-stderr") {
  process.stderr.write("XAI_API_KEY=xai-");
  setTimeout(() => process.stderr.write("secret-value prompt=private-content\n"), 2);
}
if (mode === "forced" || mode === "forced-malformed") {
  process.on("SIGTERM", () => {});
  rl.on("close", () => setInterval(() => {}, 1_000));
}
if (mode === "no-read") {
  rl.pause();
  process.stdin.pause();
  setInterval(() => {}, 1_000);
}

const finishReverse = () => {
  if (pendingReverse.size !== 3) return;
  write({ jsonrpc: "2.0", id: reverseRootId, result: Object.fromEntries(pendingReverse) });
  pendingReverse.clear();
};

rl.on("line", (line) => {
  const message = JSON.parse(line);
  if (!("method" in message)) {
    if (String(message.id) === "invalid") {
      write({ jsonrpc: "2.0", id: reverseRootId, result: message.error ?? message.result });
      return;
    }
    if (String(message.id) === "cursor-plan") {
      write({ jsonrpc: "2.0", id: reverseRootId, result: message.error ?? message.result });
      return;
    }
    pendingReverse.set(String(message.id), message.error ?? message.result);
    finishReverse();
    return;
  }
  if (!("id" in message)) return;
  if (message.method === "test/reverse") {
    reverseRootId = message.id;
    write({
      jsonrpc: "2.0",
      id: "permission",
      method: "session/request_permission",
      params: {
        sessionId: "s-1",
        toolCall: { toolCallId: "tool-1" },
        options: [{ optionId: "allow", name: "Allow", kind: "allow_once" }],
      },
    });
    write({
      jsonrpc: "2.0",
      id: "filesystem",
      method: "fs/read_text_file",
      params: { sessionId: "s-1", path: "/tmp/input" },
    });
    write({
      jsonrpc: "2.0",
      id: "terminal",
      method: "terminal/create",
      params: { sessionId: "s-1", command: "printf" },
    });
    return;
  }
  if (message.method === "test/reverse-extension") {
    reverseRootId = message.id;
    write({
      jsonrpc: "2.0",
      id: "cursor-plan",
      method: "cursor/create_plan",
      params: { toolCallId: "tool-plan", plan: "fixture" },
    });
    return;
  }
  if (message.method === "test/invalid-reverse") {
    reverseRootId = message.id;
    write({
      jsonrpc: "2.0",
      id: "invalid",
      method: "fs/read_text_file",
      params: { sessionId: "s-1", path: 42 },
    });
    return;
  }
  if (message.method === "test/never") return;
  if (message.method === "test/crash") return process.exit(7);
  if (message.method === "test/late") {
    setTimeout(() => write({ jsonrpc: "2.0", id: message.id, result: message.params }), 80);
    return;
  }
  const response = { jsonrpc: "2.0", id: message.id, result: message.params };
  if (mode === "fragmented") {
    const encoded = `${JSON.stringify(response)}\r\n\n`;
    process.stdout.write(encoded.slice(0, 3));
    setTimeout(() => process.stdout.write(encoded.slice(3)), 2);
    return;
  }
  if (mode === "coalesced") {
    process.stdout.write(
      `${JSON.stringify({
        jsonrpc: "2.0",
        method: "session/update",
        params: {
          sessionId: "s-1",
          update: {
            sessionUpdate: "agent_message_chunk",
            content: { type: "text", text: "hello" },
          },
        },
      })}\n${JSON.stringify(response)}\n`,
    );
    return;
  }
  if (mode === "response-then-update") {
    write(response);
    setImmediate(() =>
      write({
        jsonrpc: "2.0",
        method: "session/update",
        params: {
          sessionId: "s-1",
          update: {
            sessionUpdate: "agent_message_chunk",
            content: { type: "text", text: "tail" },
          },
        },
      }),
    );
    return;
  }
  write(response);
});

rl.on("close", () => {
  if (mode !== "forced") process.exit(0);
});
