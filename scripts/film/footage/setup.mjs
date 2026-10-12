// Shared setup for the launch film's live shots (#11432): a fresh test
// account on the local stack, the phone's saved session for it, and Coder
// on this computer signed in to it with a real device code.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import * as c from "./cdp.mjs";
import { Session } from "./terminal.mjs";
import { signedIn } from "./web.mjs";

const here = path.dirname(new URL(import.meta.url).pathname);

/// A fresh account through the account service's own sign-up. Writes the
/// session token (`session`) and the phone's saved session
/// (`phone-session.json`) to private files in `work`; prints neither.
export function account(gateway, base, work, label = "Alex") {
  const made = JSON.parse(
    execFileSync("curl", ["-fsS", "-X", "POST", `${gateway}/v1/accounts`,
      "-H", "content-type: application/json", "-d", JSON.stringify({ label })]),
  );
  const sessionFile = path.join(work, "session");
  fs.writeFileSync(sessionFile, made.session_token, { mode: 0o600 });
  const phoneSession = path.join(work, "phone-session.json");
  fs.writeFileSync(
    phoneSession,
    JSON.stringify({
      origin: base,
      account: made.account.id,
      label: made.account.label,
      expires_at: made.session.expires_at,
      token: made.session_token,
    }),
    { mode: 0o600 },
  );
  return { sessionFile, phoneSession };
}

/// The demo repository in a stand-in home, so paths read `~/code/slugkit`.
export function demoRepo(work) {
  const home = path.join(work, "home");
  const repo = path.join(home, "code/slugkit");
  execFileSync(path.join(here, "make-demo-repo.sh"), [repo]);
  return { home, repo };
}

/// `coder login` with a device code, approved on the website as the
/// account (an unrecorded browser), choosing to sync all chats.
export async function coderLogin({ coder, state, home, base, sessionFile, computer }) {
  fs.mkdirSync(state, { recursive: true, mode: 0o700 });
  const login = new Session({
    cols: 100,
    rows: 24,
    command: [coder, "login", "--state", state],
    cwd: home,
    env: { HOME: home, OPENAGENTS_ORIGIN: base, OPENAGENTS_COMPUTER_NAME: computer, SSH_CONNECTION: "film" },
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
}

/// A clean status bar on the simulator.
export function statusBar(sim) {
  execFileSync("xcrun", ["simctl", "status_bar", sim, "override", "--time", "9:41", "--batteryState", "discharging",
    "--batteryLevel", "100", "--cellularMode", "active", "--cellularBars", "4", "--wifiBars", "3",
    "--dataNetwork", "wifi", "--operatorName", ""]);
}

/// Starts `simctl io recordVideo`; resolves once frames flow, with the
/// Unix time that happened and a `stop()`.
export async function recordSimulator(sim, file) {
  fs.rmSync(file, { force: true });
  const { spawn } = await import("node:child_process");
  const video = spawn("xcrun", ["simctl", "io", sim, "recordVideo", "--codec=h264", "--force", file], {
    stdio: ["ignore", "ignore", "pipe"],
  });
  await new Promise((ok) => video.stderr.on("data", (d) => /started/i.test(String(d)) && ok()));
  const started = Date.now() / 1000;
  return {
    started,
    stop: async () => {
      video.kill("SIGINT");
      await new Promise((ok) => video.on("exit", ok));
    },
  };
}

/// Times (seconds into a video) where the picture visibly changes: mean
/// per-pixel change between frames at 10 fps, small grayscale.
export function videoChanges(file, threshold = 0.05) {
  const w = 120;
  const h = 262;
  const buf = execFileSync("ffmpeg", ["-loglevel", "error", "-i", file, "-vf", `fps=10,scale=${w}:${h},format=gray`,
    "-f", "rawvideo", "-"], { maxBuffer: 2 ** 31 });
  const n = Math.floor(buf.length / (w * h));
  const found = [];
  for (let i = 1; i < n; i++) {
    let sum = 0;
    for (let k = 0; k < w * h; k++) sum += Math.abs(buf[i * w * h + k] - buf[(i - 1) * w * h + k]);
    if (sum / (w * h) > threshold) found.push(i / 10);
  }
  return found;
}

/// Cuts a simulator video to the given times (seconds into it), one frame
/// each, at its own pixel size. `simctl recordVideo` writes a frame only
/// when the screen changes, so a still tail leaves the file short of the
/// shared clock; the last frame is held to cover it.
export function encodeVideoAt(file, out, times) {
  // Contiguous runs become trims; a jump starts a new one.
  const runs = [];
  for (const t of times) {
    const last = runs[runs.length - 1];
    if (last && Math.abs(t - last.end) < 0.5 / 30 + 1e-6) last.end = t + 1 / 30;
    else runs.push({ start: t, end: t + 1 / 30 });
  }
  const parts = runs.map((r, i) => `[s${i}]trim=${r.start.toFixed(4)}:${r.end.toFixed(4)},setpts=PTS-STARTPTS[p${i}]`);
  const graph =
    `[0:v]fps=30,tpad=stop_mode=clone:stop_duration=30,split=${runs.length}${runs.map((_, i) => `[s${i}]`).join("")};` +
    parts.join(";") + ";" +
    `${runs.map((_, i) => `[p${i}]`).join("")}concat=n=${runs.length}:v=1,fps=30,format=yuv420p[v]`;
  execFileSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", "-i", file, "-filter_complex", graph,
    "-map", "[v]", "-frames:v", String(times.length), "-c:v", "libx264", "-preset", "slow", "-crf", "12", "-an",
    "-movflags", "+faststart", out], { stdio: "inherit" });
}
