// Coder's new features for the launch film (#11432), recorded live:
//
//   FILM_TAKE=harness  terminal-harness.mp4: text in the input, Shift+Tab
//                      steps the rail through Auto, Claude (its model and
//                      plan windows), Codex, OpenCode, the ACP agents found
//                      here, and back to Auto. Nothing is sent.
//   FILM_TAKE=tray     terminal-tray.mp4: the chat on OpenCode, a background
//                      OpenCode agent started from the input, its row on the
//                      agent rail showing its model and each step it takes.
//   FILM_TAKE=rewind   terminal-rewind.mp4: a turn edits src/slugify.js,
//                      /rewind lists it with its file, Enter restores the
//                      file and puts the message back in the input.
//
// Coder (this checkout's coder-new) runs in the scratch slugkit repository
// (make-demo-repo.sh; FILM_REPO, default ~/code/slugkit, is made afresh)
// with the person's real HOME, so what is installed is real, and an
// isolated --state copy of their settings without account.json or
// sync.json, so nothing reaches openagents.com and nothing is written to
// ~/.openagents/coder. OPENAGENTS_TASKS points the task store (plan usage
// readings) at FILM_WORK/tasks. The tray and rewind takes pin the chat to
// OpenCode on its free model (opencode/big-pickle) first, so no turn runs
// on the person's paid logins.
//
// The replay keeps the recording's own pacing: a wait where nothing but a
// spinner or a timer changes is cut (0.7 s kept at each end), never sped
// up.
//
// env: FILM_TAKE, FILM_CODER, FILM_CODER_STATE, FILM_OUT, FILM_WORK,
//   FILM_REPO, FILM_STEP (seconds on each Shift+Tab pick, default 0.9),
//   FILM_PROBE=1 (print each screen and skip the render)
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { Session } from "./terminal.mjs";
import { replayAt, fit } from "./replay-terminal.mjs";

const env = process.env;
const here = path.dirname(new URL(import.meta.url).pathname);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const take = env.FILM_TAKE || "harness";
const probe = env.FILM_PROBE === "1";
const step = Number(env.FILM_STEP || 0.9) * 1000;
const repo = env.FILM_REPO || path.join(env.HOME, "code/slugkit");
const FREE = "opencode/big-pickle";

execFileSync("sh", [path.join(here, "make-demo-repo.sh"), repo], { stdio: "ignore" });
// The agents a background agent talks to find this build as `coder`.
const bin = path.join(env.FILM_WORK, "bin");
fs.mkdirSync(bin, { recursive: true });
fs.rmSync(path.join(bin, "coder"), { force: true });
fs.symlinkSync(env.FILM_CODER, path.join(bin, "coder"));
fs.mkdirSync(path.join(env.FILM_WORK, "tasks"), { recursive: true });

const { cols, rows } = await fit();
const coder = new Session({
  cols,
  rows,
  command: [env.FILM_CODER, "--state", env.FILM_CODER_STATE],
  cwd: repo,
  env: {
    PATH: `${bin}:${env.PATH}`,
    OPENAGENTS_COMPUTER_NAME: env.FILM_COMPUTER || "MacBook Pro",
    OPENAGENTS_TASKS: path.join(env.FILM_WORK, "tasks"),
    CODER_OPENCODE_MODEL: FREE,
    OPENCODE_CONFIG_CONTENT: JSON.stringify({ model: FREE }),
  },
});
const show = (label) => probe && console.log(`--- ${label} ---\n${coder.screen()}`);
const shiftTab = () => coder.input("\x1b[Z");

await coder.waitFor("❯", 30000);
await coder.settle(1200, 15000);

/// Shift+Tab until the rail names `name`.
async function pickRail(name) {
  for (let i = 0; i < 12; i++) {
    if (new RegExp(`${name} \\(`).test(coder.screen())) return;
    shiftTab();
    await sleep(500);
  }
  throw new Error(`no ${name} on the rail\n${coder.screen()}`);
}

if (take === "harness") {
  coder.mark("ready");
  show("ready");
  await sleep(900);
  await coder.type("Why are the tests failing?", { base: 55, jitter: 35 });
  await sleep(1100);
  // Auto, then each harness and agent, and back to Auto.
  for (let i = 0; i < 12; i++) {
    shiftTab();
    await sleep(i === 0 ? step + 700 : step);
    show(`pick ${i + 1}`);
    if (i > 0 && /─ Auto ─/.test(coder.screen())) break;
  }
  await sleep(900);
  coder.mark("end");
} else if (take === "tray") {
  await pickRail("OpenCode");
  await coder.settle(800, 5000);
  coder.mark("ready");
  show("ready");
  await sleep(700);
  await coder.type("/agent opencode Run the tests and say which ones fail", { base: 45, jitter: 30 });
  await sleep(600);
  coder.input("\r");
  await coder.waitFor(/running ·/, 20000);
  show("started");
  await coder.waitFor(/finished after/, 180000);
  show("finished");
  await sleep(1500);
  coder.mark("end");
} else if (take === "rewind") {
  await pickRail("OpenCode");
  await coder.settle(800, 5000);
  coder.mark("ready");
  show("ready");
  await sleep(700);
  await coder.type("Make slugify trim dashes from both ends.", { base: 50, jitter: 30 });
  await sleep(600);
  coder.input("\r");
  // The turn: wait until it has changed the file and finished.
  await coder.waitFor(/Working/, 30000);
  while (/Working/.test(coder.screen())) await sleep(300);
  await coder.settle(1500, 30000);
  show("answered");
  const changed = execFileSync("git", ["status", "--short"], { cwd: repo, encoding: "utf8" });
  if (!changed.trim()) throw new Error("the turn changed no file");
  await sleep(1200);
  await coder.type("/rewind", { base: 90, jitter: 40 });
  await sleep(600);
  coder.input("\r");
  await sleep(2600);
  show("list");
  coder.input("\r");
  await sleep(2600);
  show("restored");
  const left = execFileSync("git", ["status", "--short"], { cwd: repo, encoding: "utf8" });
  if (left.trim()) throw new Error(`rewind left changes:\n${left}`);
  coder.mark("end");
} else {
  throw new Error(`unknown FILM_TAKE ${take}`);
}

coder.input("\x03");
await sleep(300);
coder.input("\x03");
await Promise.race([coder.exited, sleep(3000)]);
coder.kill();
const cast = path.join(env.FILM_WORK, `${take}.cast`);
coder.save(cast);
if (probe) process.exit(0);

/// Frame times on the recording's clock from `from` to `to`, at real
/// speed, jumping over each stretch longer than `over` seconds in which
/// nothing meaningful changed (keeping `keep` seconds at each end).
function times(events, from, to, { over = 2.0, keep = 0.7 } = {}) {
  const marks = events.filter((e) => (e[1] === "c" || e[1] === "i") && e[0] > from && e[0] < to).map((e) => e[0]);
  const jumps = [];
  let prev = from;
  for (const t of [...marks, to]) {
    if (t - prev > over) jumps.push([prev + keep, t - keep]);
    prev = t;
  }
  const out = [];
  let t = from;
  while (t < to) {
    const jump = jumps.find(([a, b]) => t >= a && t < b);
    if (jump) t = jump[1];
    out.push(t);
    t += 1 / 30;
  }
  return out;
}

const mark = (name) => coder.events.find((e) => e[1] === "m" && e[2] === name)[0];
const frames = times(coder.events, Math.max(0, mark("ready") - 0.3), mark("end"));
const out = path.join(env.FILM_OUT, `terminal-${take}.mp4`);
const r = await replayAt(cast, out, frames);
console.log(`terminal-${take}: ${r.seconds.toFixed(1)} s`);
