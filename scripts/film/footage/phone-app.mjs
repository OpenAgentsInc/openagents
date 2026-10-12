// phone-app.mp4: the OpenAgents iPhone app (bins/openagents-ios, a local
// simulator debug build) signed in to the same account, on Running, while
// Coder works a real task on the computer. The card for Coder's chat appears
// as it starts ("Working", its timer, Stop and Message) and turns to "Done"
// when it finishes. Recorded with `simctl io recordVideo` at the simulator's
// own pixel size, with a clean status bar; the stretch while Coder works is
// shortened to about a second.
//
// env: FILM_BASE, FILM_GATEWAY, FILM_OUT, FILM_WORK, FILM_SIM (simulator
//   UDID), FILM_CODER, FILM_TASK, CARGO_TARGET_DIR
// `node phone-app.mjs --edit RAW` only re-cuts a kept raw take into FILM_OUT.
import { spawn, execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import * as c from "./cdp.mjs";
import { Session } from "./terminal.mjs";
import { signedIn } from "./web.mjs";

const env = process.env;
const here = path.dirname(new URL(import.meta.url).pathname);
const root = path.resolve(here, "../../..");
const base = env.FILM_BASE;
const work = env.FILM_WORK;
const sim = env.FILM_SIM;
const out = path.join(env.FILM_OUT, "phone-app.mp4");
const task =
  env.FILM_TASK || "The tests in this repo are failing. Find out why, fix slugify, and run the tests.";
const computer = env.FILM_COMPUTER || "MacBook Pro";
const now = () => Date.now() / 1000;

/// When the screen visibly changes in RAW: mean per-pixel change between
/// frames at 10 fps, small grayscale. Clock and timer ticks stay below it.
function changes(raw) {
  const w = 120;
  const h = 262;
  const buf = execFileSync("ffmpeg", ["-loglevel", "error", "-i", raw, "-vf", `fps=10,scale=${w}:${h},format=gray`,
    "-f", "rawvideo", "-"], { maxBuffer: 1 << 30 });
  const n = Math.floor(buf.length / (w * h));
  const found = [];
  for (let i = 1; i < n; i++) {
    let sum = 0;
    for (let k = 0; k < w * h; k++) sum += Math.abs(buf[i * w * h + k] - buf[(i - 1) * w * h + k]);
    if (sum / (w * h) > 0.05) found.push(i / 10);
  }
  return found;
}

/// The edit: the empty list, the card appearing and working (about 5 s),
/// the wait shortened to under a second, then the card turning to Done.
async function edit(raw, out, end) {
  const seen = changes(raw);
  if (seen.length < 2) throw new Error(`expected the card to appear and finish; changes at ${seen}`);
  const appear = seen[0];
  const finish = seen[seen.length - 1];
  end = end ?? Number(execFileSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", raw]));
  const a = Math.max(0, appear - 1.5);
  const b = appear + 3.5;
  const d0 = finish - 0.7;
  const d1 = Math.min(end - 0.2, finish + 3.0);
  const squeeze = Math.max(0.01, (d0 - b) / 0.8);
  const [w, h] = execFileSync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height",
    "-of", "csv=p=0", raw]).toString().trim().split(",").map(Number);
  execFileSync(
    "ffmpeg",
    ["-hide_banner", "-loglevel", "error", "-y", "-i", raw, "-filter_complex",
      `[0:v]fps=30,split=3[x][y][z];` +
      `[x]trim=${a}:${b},setpts=PTS-STARTPTS[p1];` +
      `[y]trim=${b}:${d0},setpts=(PTS-STARTPTS)/${squeeze}[p2];` +
      `[z]trim=${d0}:${d1},setpts=PTS-STARTPTS[p3];` +
      `[p1][p2][p3]concat=n=3:v=1,fps=30,format=yuv420p[v]`,
      "-map", "[v]", "-c:v", "libx264", "-preset", "slow", "-crf", "12", "-an", "-movflags", "+faststart", out],
    { stdio: "inherit" },
  );
  console.log(`phone-app: card at ${appear} s, done at ${finish} s in the raw take; ${w}x${h}`);
}

// `node phone-app.mjs --edit RAW` cuts a kept raw take again.
if (process.argv[2] === "--edit") {
  await edit(process.argv[3], out);
  process.exit(0);
}

// A fresh test account. Its tokens go to private files, never the output.
const account = JSON.parse(
  execFileSync("curl", ["-fsS", "-X", "POST", `${env.FILM_GATEWAY}/v1/accounts`,
    "-H", "content-type: application/json", "-d", JSON.stringify({ label: env.FILM_ACCOUNT || "Alex" })]),
);
const sessionFile = path.join(work, "phone-session");
fs.writeFileSync(sessionFile, account.session_token, { mode: 0o600 });
// The phone's saved session (simulator only; the app's AccountGate hook).
const phoneSession = path.join(work, "phone-session.json");
fs.writeFileSync(
  phoneSession,
  JSON.stringify({
    origin: base,
    account: account.account.id,
    label: account.account.label,
    expires_at: account.session.expires_at,
    token: account.session_token,
  }),
  { mode: 0o600 },
);

// Coder on this computer signs in to the account with a device code,
// approved on the website, and syncs all its chats.
const home = path.join(work, "phone-home");
const repo = path.join(home, "code/slugkit");
const coderState = path.join(work, "phone-coder-state");
fs.mkdirSync(coderState, { recursive: true, mode: 0o700 });
execFileSync(path.join(here, "make-demo-repo.sh"), [repo]);
const coderEnv = { HOME: home, OPENAGENTS_ORIGIN: base, OPENAGENTS_COMPUTER_NAME: computer };
const login = new Session({
  cols: 100,
  rows: 24,
  command: [env.FILM_CODER, "login", "--state", coderState],
  cwd: home,
  env: { ...coderEnv, SSH_CONNECTION: "film" },
});
await login.waitFor(/[A-Z]{4}-[A-Z]{4}/, 30000);
const code = login.screen().match(/[A-Z]{4}-[A-Z]{4}/)[0];
const web = await signedIn(base, sessionFile, { width: 1200, height: 900, dpr: 1 });
await c.navigate(web, `${base}/device`);
await c.click(web, "input[name=code]");
await c.type(web, code, { base: 5, jitter: 5 });
await c.click(web, await c.byText(web, "button", "Continue"));
await c.waitFor(web, `!!document.querySelector('button[value=approve]')`, 10000);
await c.click(web, await c.byText(web, "button", "Approve"));
await c.waitFor(web, `document.body.innerText.includes('is signed in')`, 15000);
await web.close();
await login.waitFor("Type 1 or 2", 30000);
await login.type("1\r");
await Promise.race([login.exited, c.sleep(15000)]);
login.kill();
console.log("phone-app: Coder signed in and syncing");

// The app, through its film UI test: signed in with the saved session, on
// Running, until we write `finish`.
const film = path.join(work, "phone-film");
fs.rmSync(film, { recursive: true, force: true });
fs.mkdirSync(film, { recursive: true });
const target = env.CARGO_TARGET_DIR;
const xcode = spawn(
  "xcodebuild",
  ["test", "-project", path.join(root, "bins/openagents-ios/host/OpenAgents.xcodeproj"),
    "-scheme", "OpenAgents", "-destination", `id=${sim}`,
    "-derivedDataPath", path.join(target, "openagents-ios/DerivedData-film"),
    "-only-testing:OpenAgentsUITests/FilmUITests",
    `OPENAGENTS_RUST_LIBRARY_DIR=${path.join(target, "aarch64-apple-ios-sim/debug")}`,
    "CODE_SIGN_IDENTITY=-", "PROVISIONING_PROFILE_SPECIFIER="],
  {
    env: {
      ...process.env,
      TEST_RUNNER_OPENAGENTS_UITEST_FILM: film,
      TEST_RUNNER_OPENAGENTS_UITEST_ORIGIN: base,
      TEST_RUNNER_OPENAGENTS_UITEST_SESSION_FILE: phoneSession,
    },
    stdio: ["ignore", fs.openSync(path.join(work, "phone-xcodebuild.log"), "w"), "inherit"],
  },
);
const xcodeDone = new Promise((ok) => xcode.on("exit", ok));
while (!fs.existsSync(path.join(film, "ready"))) {
  if (xcode.exitCode !== null) throw new Error(`The film UI test ended early; see ${work}/phone-xcodebuild.log`);
  await c.sleep(500);
}
console.log("phone-app: the app shows Running");

execFileSync("xcrun", ["simctl", "status_bar", sim, "override", "--time", "9:41", "--batteryState", "discharging",
  "--batteryLevel", "100", "--cellularMode", "active", "--cellularBars", "4", "--wifiBars", "3",
  "--dataNetwork", "wifi", "--operatorName", ""]);
await c.sleep(1500);
const raw = path.join(work, "phone-app-raw.mp4");
fs.rmSync(raw, { force: true });
const video = spawn("xcrun", ["simctl", "io", sim, "recordVideo", "--codec=h264", "--force", raw], {
  stdio: ["ignore", "ignore", "pipe"],
});
// recordVideo says "Recording started" on stderr once frames flow.
await new Promise((ok) => video.stderr.on("data", (d) => /started/i.test(String(d)) && ok()));
const t0 = now();
await c.sleep(2500);

// Coder works the task on the computer (not filmed here).
const coder = new Session({
  cols: 120,
  rows: 32,
  command: [env.FILM_CODER, "--state", coderState],
  cwd: repo,
  env: coderEnv,
});
await coder.waitFor("❯", 30000);
await c.sleep(800);
await coder.type(task, { base: 8, jitter: 4 });
coder.input("\r");
const sent = now() - t0;
await coder.waitFor("Working", 30000);
for (const started = Date.now(); ; ) {
  await c.sleep(500);
  const s = coder.screen();
  if (!/Working/.test(s) && /pass/i.test(s)) break;
  if (Date.now() - started > 10 * 60 * 1000) throw new Error(`Coder didn't finish:\n${s}`);
}
const done = now() - t0;
await c.sleep(7000);
coder.input("\x03");
await Promise.race([coder.exited, c.sleep(3000)]);
coder.kill();
const end = now() - t0;

video.kill("SIGINT");
await new Promise((ok) => video.on("exit", ok));
fs.writeFileSync(path.join(film, "finish"), "finish");
await xcodeDone;
execFileSync("xcrun", ["simctl", "status_bar", sim, "clear"]);

await edit(raw, out, end);
console.log(`phone-app: ${out} (sent ${sent.toFixed(1)} s, done ${done.toFixed(1)} s)`);
