import { execFile } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const ADAPTER_PACKAGE = "@resend/chat-sdk-adapter";
const CHAT_PACKAGE = "chat";
const STATE_PACKAGE = "@chat-adapter/state-memory";

const packageJson = JSON.parse(
  await readFile(join(process.cwd(), "package.json"), "utf8"),
);

const npmView = async (name, fields) => {
  const { stdout } = await execFileAsync("npm", [
    "view",
    `${name}@latest`,
    ...fields,
    "--json",
  ]);
  return JSON.parse(stdout);
};

const majorOfRange = value => {
  if (typeof value !== "string") return null;
  const match = value.match(/\d+/u);
  return match ? Number(match[0]) : null;
};

const dependencyVersion = name =>
  packageJson.dependencies?.[name] ?? packageJson.devDependencies?.[name] ?? null;

const adapter = await npmView(ADAPTER_PACKAGE, [
  "version",
  "peerDependencies",
  "dependencies",
]);
const chat = await npmView(CHAT_PACKAGE, [
  "version",
  "peerDependencies",
  "dependencies",
]);
const state = await npmView(STATE_PACKAGE, [
  "version",
  "peerDependencies",
  "dependencies",
]);

const sarahAiVersion = dependencyVersion("ai");
const sarahAiMajor = majorOfRange(sarahAiVersion);
const chatAiPeer = chat.peerDependencies?.ai ?? null;
const chatAiPeerMajor = majorOfRange(chatAiPeer);
const compatible =
  sarahAiMajor !== null &&
  chatAiPeerMajor !== null &&
  sarahAiMajor === chatAiPeerMajor;

const evidence = {
  schema: "sarah.s8_resend_adapter_compat.v1",
  generatedAt: new Date().toISOString(),
  status: compatible ? "passed" : "blocked",
  packages: {
    sarah: {
      ai: sarahAiVersion,
      "@ai-sdk/react": dependencyVersion("@ai-sdk/react"),
      "@ai-sdk/gateway": dependencyVersion("@ai-sdk/gateway"),
      eve: dependencyVersion("eve"),
    },
    adapter: {
      name: ADAPTER_PACKAGE,
      version: adapter.version,
      peerDependencies: adapter.peerDependencies ?? {},
      dependencies: adapter.dependencies ?? {},
    },
    chat: {
      name: CHAT_PACKAGE,
      version: chat.version,
      peerDependencies: chat.peerDependencies ?? {},
      dependencies: chat.dependencies ?? {},
    },
    state: {
      name: STATE_PACKAGE,
      version: state.version,
      peerDependencies: state.peerDependencies ?? {},
      dependencies: state.dependencies ?? {},
    },
  },
  finding: compatible
    ? "The latest public Resend Chat SDK adapter's Chat SDK peer matches Sarah's AI SDK major."
    : "The latest public Resend Chat SDK adapter still peers on the public Chat SDK line whose ai peer does not match Sarah's AI SDK realtime canary major.",
  remainingExitGate: compatible
    ? "Install the adapter in a branch, wire the Eve chatSdkChannel, and run a real approved sarah@ Resend send smoke."
    : "Wait for a Resend Chat SDK adapter / Chat SDK release compatible with Sarah's AI SDK 7 realtime stack, or approve the existing Sarah Resend REST sender as the production email send path.",
};

const outPath = join(
  process.cwd(),
  "docs",
  "evidence",
  "2026-07-08-s8-resend-adapter-compat.json",
);
await mkdir(dirname(outPath), { recursive: true });
await writeFile(outPath, `${JSON.stringify(evidence, null, 2)}\n`);

console.log(JSON.stringify(evidence, null, 2));
