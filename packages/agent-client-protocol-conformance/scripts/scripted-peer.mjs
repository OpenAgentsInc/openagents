import { createHash } from "node:crypto";
import readline from "node:readline";

const scenario = JSON.parse(process.env.OA_ACP_SCENARIO ?? '{"name":"empty","actions":[]}');
if (typeof scenario.exitOnStart === "number") process.exit(scenario.exitOnStart);
const actions = new Map(scenario.actions.map((action) => [action.method, action]));
const pending = new Map();
let nextReverseId = 10000;
let writeChain = Promise.resolve();

const writeNow = async (message, fragmentBytes = 0) => {
  const line = `${JSON.stringify(message)}\n`;
  if (fragmentBytes <= 0) {
    process.stdout.write(line);
    return;
  }
  for (let offset = 0; offset < line.length; offset += fragmentBytes) {
    process.stdout.write(line.slice(offset, offset + fragmentBytes));
    await new Promise((resolve) => setImmediate(resolve));
  }
};

const write = (message, fragmentBytes = 0) => {
  const pendingWrite = writeChain.then(() => writeNow(message, fragmentBytes));
  writeChain = pendingWrite.catch(() => undefined);
  return pendingWrite;
};

const reverse = (request, fragmentBytes) =>
  new Promise((resolve, reject) => {
    const id = nextReverseId++;
    pending.set(id, { resolve, reject });
    void write(
      { jsonrpc: "2.0", id, method: request.method, params: request.params },
      fragmentBytes,
    );
  });

const handle = async (message) => {
  if (!("method" in message)) {
    const waiter = pending.get(message.id);
    if (waiter === undefined) return;
    pending.delete(message.id);
    if (message.error) waiter.reject(new Error(message.error.message));
    else waiter.resolve(message.result);
    return;
  }
  const action = actions.get(message.method);
  if (action === undefined) {
    if ("id" in message)
      await write({
        jsonrpc: "2.0",
        id: message.id,
        error: { code: -32601, message: "script has no action" },
      });
    return;
  }
  if (action.expectParamsSha256) {
    const actual = createHash("sha256").update(JSON.stringify(message.params)).digest("hex");
    if (actual !== action.expectParamsSha256) {
      await write({
        jsonrpc: "2.0",
        id: message.id,
        error: { code: -32602, message: "params digest mismatch" },
      });
      return;
    }
  }
  if (action.stderr) process.stderr.write(action.stderr);
  if (action.delayMs) await new Promise((resolve) => setTimeout(resolve, action.delayMs));
  for (const notification of action.notifications ?? []) {
    await write(
      { jsonrpc: "2.0", method: notification.method, params: notification.params },
      action.fragmentBytes,
    );
  }
  for (const request of action.reverseRequests ?? []) {
    try {
      await reverse(request, action.fragmentBytes);
    } catch (error) {
      if (!action.ignoreReverseErrors) throw error;
    }
  }
  if (action.raw) process.stdout.write(action.raw);
  if (action.exitBeforeResponse) process.exit(action.exitCode ?? 1);
  if ("id" in message) {
    const response = action.error
      ? { jsonrpc: "2.0", id: message.id, error: action.error }
      : { jsonrpc: "2.0", id: message.id, result: action.result ?? {} };
    await write(response, action.fragmentBytes);
    if (action.duplicateResponse) await write(response, action.fragmentBytes);
    if (action.lateDuplicateMs) {
      setTimeout(() => void write(response, action.fragmentBytes), action.lateDuplicateMs);
    }
  }
  if (typeof action.exitCode === "number") process.exit(action.exitCode);
};

if (scenario.pauseInput) {
  process.stdin.pause();
  setInterval(() => undefined, 1_000);
} else {
  const rl = readline.createInterface({ input: process.stdin });
  rl.on("line", (line) => {
    try {
      void handle(JSON.parse(line));
    } catch (error) {
      process.stderr.write(`${String(error)}\n`);
      process.exitCode = 1;
    }
  });
}
