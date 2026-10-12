// Boot clips for the launch film (#11432): each surface starting, from
// cold, to its signed-in home screen, on the same test account that
// sync-shoot.mjs then uses (written to FILM_WORK/setup.json).
//
//   boot-terminal.mp4  a shell in ~/code/slugkit, `coder` typed, Coder's home
//   boot-web.mp4       Chrome opening the website, signed in
//   boot-phone.mp4     the iPhone home screen, the OpenAgents app opening
//                      to its signed-in home (a simulator debug build)
//
// The desktop app is not recorded: see the report in sync-v1/README.txt.
//
// env: FILM_BASE, FILM_GATEWAY, FILM_OUT, FILM_WORK, FILM_SIM, FILM_CODER
import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import * as c from "./cdp.mjs";
import { Session } from "./terminal.mjs";
import { replay, fit } from "./replay-terminal.mjs";
import { signedIn } from "./web.mjs";
import * as setup from "./setup.mjs";

const env = process.env;
const base = env.FILM_BASE;
const work = env.FILM_WORK;
const out = env.FILM_OUT;
const sim = env.FILM_SIM;
const computer = env.FILM_COMPUTER || "MacBook Pro";
fs.mkdirSync(out, { recursive: true });

// FILM_BOOT picks clips (e.g. "phone"); all three by default.
const only = new Set((env.FILM_BOOT || "terminal,web,phone").split(","));

// The account, the repository, and Coder signed in, for every shot; or a
// kept FILM_SETUP to record a clip again on the same account.
let s;
if (env.FILM_SETUP && fs.existsSync(env.FILM_SETUP)) {
  s = JSON.parse(fs.readFileSync(env.FILM_SETUP, "utf8"));
} else {
  const a = setup.account(env.FILM_GATEWAY, base, work);
  const r = setup.demoRepo(work);
  s = { ...a, ...r, coderState: path.join(work, "coder-state") };
  await setup.coderLogin({ coder: env.FILM_CODER, state: s.coderState, home: s.home, base,
    sessionFile: s.sessionFile, computer });
  fs.writeFileSync(path.join(work, "setup.json"), JSON.stringify(s), { mode: 0o600 });
}

// The terminal: `coder` on the PATH, as the installer puts it.
if (only.has("terminal")) {
const bin = path.join(s.home, ".local/bin");
fs.mkdirSync(bin, { recursive: true });
fs.writeFileSync(path.join(bin, "coder"), `#!/bin/sh\nexec "${env.FILM_CODER}" --state "${s.coderState}" "$@"\n`, {
  mode: 0o755,
});
{
  const { cols, rows } = await fit();
  const shell = new Session({
    cols,
    rows,
    command: ["bash", "--norc", "--noprofile", "-i"],
    cwd: s.repo,
    env: {
      HOME: s.home,
      PATH: `${bin}:/usr/bin:/bin`,
      PS1: "\\w $ ",
      OPENAGENTS_ORIGIN: base,
      OPENAGENTS_COMPUTER_NAME: computer,
      BASH_SILENCE_DEPRECATION_WARNING: "1",
    },
  });
  await shell.waitFor("slugkit $", 10000);
  await c.sleep(900);
  shell.mark("start");
  await shell.type("coder", { base: 110, jitter: 50 });
  await c.sleep(350);
  shell.input("\r");
  await shell.waitFor("❯", 30000);
  await shell.settle(800, 10000);
  await c.sleep(2000);
  shell.mark("end");
  shell.input("\x03");
  await c.sleep(500);
  shell.kill();
  const cast = path.join(work, "boot-terminal.cast");
  shell.save(cast);
  const mark = (name) => shell.events.find((e) => e[1] === "m" && e[2] === name)[0];
  await replay(cast, path.join(out, "boot-terminal.mp4"), { from: mark("start") - 0.8, to: mark("end"), hold: 0.3 });
  console.log("boot-terminal");
}
}

// The website: a fresh browser opening openagents.com, signed in.
if (only.has("web")) {
  const web = await signedIn(base, s.sessionFile);
  await c.navigate(web, "data:text/html,<body style='background:rgb(10,10,11)'></body>");
  const rec = new c.Recorder(web, path.join(work, "boot-web-frames"));
  await rec.start();
  await c.sleep(700);
  await c.navigate(web, `${base}/`);
  await c.sleep(2500);
  await rec.stop();
  rec.encode(path.join(out, "boot-web.mp4"));
  await web.close();
  fs.rmSync(path.join(work, "boot-web-frames"), { recursive: true, force: true });
  console.log("boot-web");
}

// The phone: from the home screen, the app opens signed in. The UI tests'
// runner app would show on the home screen; the shoots reinstall it.
if (only.has("phone")) {
  spawnSync("xcrun", ["simctl", "terminate", sim, "com.openagents.app"]);
  spawnSync("xcrun", ["simctl", "uninstall", sim, "com.openagents.app.uitests.xctrunner"]);
  await c.sleep(1500);
  setup.statusBar(sim);
  const raw = path.join(work, "boot-phone-raw.mp4");
  const video = await setup.recordSimulator(sim, raw);
  await c.sleep(1500);
  execFileSync("xcrun", ["simctl", "launch", sim, "com.openagents.app", "--account-origin", base, "--appearance", "dark"], {
    env: { ...process.env, SIMCTL_CHILD_OPENAGENTS_UITEST_ACCOUNT_SESSION: fs.readFileSync(s.phoneSession, "utf8") },
  });
  await c.sleep(5000);
  await video.stop();
  const [w, h] = execFileSync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height",
    "-of", "csv=p=0", raw]).toString().trim().split(",").map(Number);
  execFileSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", "-i", raw, "-vf", `fps=30,scale=${w}:${h},format=yuv420p`,
    "-c:v", "libx264", "-preset", "slow", "-crf", "12", "-an", "-movflags", "+faststart", path.join(out, "boot-phone.mp4")]);
  console.log("boot-phone");
}
