// The launch film's one-conversation shot (#11432): one chat moving live
// between the terminal, the website, and the iPhone app, every surface
// recorded at once on one clock.
//
//   1. In the terminal, the person asks Coder about the failing tests. The
//      chat appears on the website (sidebar, then the open thread) and in
//      the phone's chat list, which opens it.
//   2. From the phone, the person replies "Fix it…". Coder in the terminal
//      takes it and works; the website shows it too.
//   3. On the website, the person replies "Commit it…". Coder takes it;
//      the phone shows it.
//
// Outputs in FILM_OUT: terminal.mp4, web.mp4, phone.mp4 (each at its own
// pixel size, 30 fps), cut identically: a wait where nothing changes on any
// surface jumps over its middle, so the clips stay in sync side by side.
// beats.json gives each beat's time in the clips and on the shared clock.
//
// env: FILM_BASE, FILM_GATEWAY, FILM_OUT, FILM_WORK, FILM_SIM, FILM_CODER,
//   CARGO_TARGET_DIR; FILM_SETUP (a JSON file from boot.mjs's setup, so
//   both use the same account and Coder), FILM_COMPUTER.
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import * as c from "./cdp.mjs";
import { Session } from "./terminal.mjs";
import { replayAt, fit } from "./replay-terminal.mjs";
import { signedIn } from "./web.mjs";
import * as setup from "./setup.mjs";

const env = process.env;
const here = path.dirname(new URL(import.meta.url).pathname);
const root = path.resolve(here, "../../..");
const base = env.FILM_BASE;
const work = env.FILM_WORK;
const out = env.FILM_OUT;
const sim = env.FILM_SIM;
const computer = env.FILM_COMPUTER || "MacBook Pro";
const ASK = env.FILM_ASK || "Why are the tests failing? Look, but don't change anything yet.";
const PHONE = env.FILM_PHONE_REPLY || "Fix it and run the tests.";
const WEB = env.FILM_WEB_REPLY || "Commit it with a short message.";
fs.mkdirSync(out, { recursive: true });
const now = () => Date.now() / 1000;
const beats = {};
const beat = (name, at = now()) => {
  beats[name] = at;
  console.log(`beat ${name}`);
};

// `node sync-shoot.mjs --edit WORK` cuts a kept take again.
if (process.argv[2] === "--edit") {
  await edit(JSON.parse(fs.readFileSync(path.join(process.argv[3], "take.json"), "utf8")));
  process.exit(0);
}

// Account, repository, Coder signed in: from boot.mjs, or made here.
let s;
if (env.FILM_SETUP && fs.existsSync(env.FILM_SETUP)) {
  s = JSON.parse(fs.readFileSync(env.FILM_SETUP, "utf8"));
} else {
  const a = setup.account(env.FILM_GATEWAY, base, work);
  const r = setup.demoRepo(work);
  s = { ...a, ...r, coderState: path.join(work, "coder-state") };
  await setup.coderLogin({ coder: env.FILM_CODER, state: s.coderState, home: s.home, base,
    sessionFile: s.sessionFile, computer });
}

// The phone: the app through its film UI test, on Your chats.
const film = path.join(work, "phone-sync");
fs.rmSync(film, { recursive: true, force: true });
fs.mkdirSync(film, { recursive: true });
const target = env.CARGO_TARGET_DIR;
const xcode = spawn(
  "xcodebuild",
  ["test", "-project", path.join(root, "bins/openagents-ios/host/OpenAgents.xcodeproj"),
    "-scheme", "OpenAgents", "-destination", `id=${sim}`,
    "-derivedDataPath", path.join(target, "openagents-ios/DerivedData-film"),
    "-only-testing:OpenAgentsUITests/FilmUITests/testSyncForTheFilm",
    `OPENAGENTS_RUST_LIBRARY_DIR=${path.join(target, "aarch64-apple-ios-sim/debug")}`,
    "CODE_SIGN_IDENTITY=-", "PROVISIONING_PROFILE_SPECIFIER="],
  {
    env: {
      ...process.env,
      TEST_RUNNER_OPENAGENTS_UITEST_FILM_SYNC: film,
      TEST_RUNNER_OPENAGENTS_UITEST_ORIGIN: base,
      TEST_RUNNER_OPENAGENTS_UITEST_SESSION_FILE: s.phoneSession,
    },
    stdio: ["ignore", fs.openSync(path.join(work, "phone-sync-xcodebuild.log"), "w"), "inherit"],
  },
);
const xcodeDone = new Promise((ok) => xcode.on("exit", ok));
const phoneFile = (name) => path.join(film, name);
const phoneStamp = (name) => (fs.existsSync(phoneFile(name)) ? Number(fs.readFileSync(phoneFile(name), "utf8")) : null);
while (!fs.existsSync(phoneFile("ready"))) {
  if (xcode.exitCode !== null) throw new Error(`The phone's UI test ended early; see ${work}/phone-sync-xcodebuild.log`);
  await c.sleep(500);
}
console.log("phone: on Your chats");

// The terminal: Coder in the repository, on its home screen.
const { cols, rows } = await fit();
const coder = new Session({
  cols,
  rows,
  command: [env.FILM_CODER, "--state", s.coderState],
  cwd: s.repo,
  env: { HOME: s.home, OPENAGENTS_ORIGIN: base, OPENAGENTS_COMPUTER_NAME: computer },
});
await coder.waitFor("❯", 30000);

// The website: signed in, on the home page.
const web = await signedIn(base, s.sessionFile);
await c.navigate(web, `${base}/`);
await c.sleep(1200);

// Everything records from here, on the wall clock.
setup.statusBar(sim);
const phone = await setup.recordSimulator(sim, path.join(work, "phone-raw.mp4"));
const rec = new c.Recorder(web, path.join(work, "web-frames"));
await rec.start();
// What changes on the website, spinners aside: the sidebar and the thread.
const webChanges = [];
let webStop = false;
const SIG = `(document.querySelector('.oa-chat-list')?.innerText || '') + '|' + (document.querySelector('.oa-thread')?.innerText || '') + '|' + location.pathname`;
const webWatch = (async () => {
  let last = "";
  while (!webStop) {
    try {
      const sig = await c.evaluate(web, SIG);
      if (sig !== last) {
        last = sig;
        webChanges.push(now());
      }
    } catch {}
    await c.sleep(150);
  }
})();
fs.writeFileSync(phoneFile("open"), "open");
beat("start");
await c.sleep(2500);

// 1. The terminal asks.
beat("terminal_typing");
await coder.type(ASK, { base: 34, jitter: 26 });
await c.sleep(350);
coder.input("\r");
beat("terminal_sent");

const ROW = `[...document.querySelectorAll('a.oa-nav-item')].find(a => /^Terminal/.test(a.querySelector('.oa-nav-item-detail')?.innerText || ''))`;
const webOpen = (async () => {
  await c.waitFor(web, `!!${ROW}`, 60000, "the terminal chat on the website");
  beat("web_sidebar");
  await c.sleep(900);
  await c.evaluate(web, `(${ROW}).setAttribute('data-film-target', '1')`);
  await c.click(web, "[data-film-target]");
  await c.waitFor(web, `document.querySelectorAll('.oa-thread .oa-message').length >= 1`, 20000, "the thread");
  beat("web_opened");
})();

async function coderAnswered(label) {
  await coder.waitFor("Working", 60000);
  for (const started = Date.now(); ; ) {
    await c.sleep(300);
    if (!/Working/.test(coder.screen())) break;
    if (Date.now() - started > 10 * 60 * 1000) throw new Error(`Coder didn't finish:\n${coder.screen()}`);
  }
  beat(label);
}
await coderAnswered("terminal_answered_1");
await webOpen;
const textOn = (needle) => `(document.querySelector('.oa-thread')?.innerText || '').includes(${JSON.stringify(needle)})`;
for (const end = Date.now() + 60000; !phoneStamp("opened") && Date.now() < end; ) await c.sleep(200);
if (phoneStamp("seen")) beat("phone_listed", phoneStamp("seen"));
if (phoneStamp("opened")) beat("phone_opened", phoneStamp("opened"));
await c.sleep(3000);

// 2. The phone replies.
fs.writeFileSync(phoneFile("reply"), PHONE);
for (const end = Date.now() + 60000; !phoneStamp("replied") && Date.now() < end; ) await c.sleep(100);
if (!phoneStamp("replied")) throw new Error("The phone didn't send its reply.");
beat("phone_reply_sent", phoneStamp("replied"));
const webSaw = (needle, name) =>
  c.waitFor(web, textOn(needle), 120000, name).then(() => beat(name));
const webGotPhone = webSaw(PHONE, "web_shows_phone_reply");
await coder.waitFor(PHONE.slice(0, 12), 120000);
beat("terminal_shows_phone_reply");
await coderAnswered("terminal_answered_2");
await webGotPhone;
await c.sleep(3000);

// 3. The website replies.
beat("web_typing");
await c.click(web, "textarea[name=q]");
await c.type(web, WEB, { base: 45, jitter: 35 });
await c.sleep(400);
await c.pressEnter(web);
beat("web_reply_sent");
await coder.waitFor(WEB.slice(0, 12), 120000);
beat("terminal_shows_web_reply");
await coderAnswered("terminal_answered_3");
await c.sleep(4000);
beat("end");

// Stop everything.
webStop = true;
await webWatch;
await rec.stop();
await web.close();
await phone.stop();
fs.writeFileSync(phoneFile("finish"), "finish");
coder.input("\x03");
await Promise.race([coder.exited, c.sleep(3000)]);
coder.kill();
const cast = path.join(work, "terminal.cast");
coder.save(cast);
await xcodeDone;

// Everything the cut needs, kept so `--edit` can cut the take again.
const take = {
  beats,
  webChanges,
  webFrames: rec.frames,
  webSize: web.size,
  phoneStarted: phone.started,
  phoneRaw: path.join(work, "phone-raw.mp4"),
  cast,
  castEpoch: coder.epoch0,
  terminalMarks: coder.events.filter((e) => e[1] === "c" || e[1] === "i").map((e) => coder.epoch(e[0])),
  messages: { terminal: ASK, phone: PHONE, web: WEB },
};
fs.writeFileSync(path.join(work, "take.json"), JSON.stringify(take));
await edit(take);

async function edit(take) {
const { beats } = take;
// The shared cut: every moment something changes on any surface, plus the
// beats. A stretch longer than 2 s with no change keeps 0.7 s at each end
// and jumps over the middle, on every surface alike.
const raw = take.phoneRaw;
const marks = [
  ...take.terminalMarks,
  ...take.webChanges,
  ...setup.videoChanges(raw).map((t) => take.phoneStarted + t),
  ...Object.values(beats),
]
  .filter((t) => t >= beats.start && t <= beats.end)
  .sort((a, b) => a - b);
const keep = [];
let from = beats.start;
for (let i = 1; i < marks.length; i++) {
  if (marks[i] - marks[i - 1] > 2.0) {
    keep.push([from, marks[i - 1] + 0.7]);
    from = marks[i] - 0.7;
  }
}
keep.push([from, beats.end]);
const times = [];
for (const [a, b] of keep) for (let t = a; t < b; t += 1 / 30) times.push(t);
const clip = (t) => {
  // Where a shared-clock time lands in the cut clips.
  let at = 0;
  for (const [a, b] of keep) {
    if (t < a) return at;
    if (t <= b) return at + (t - a);
    at += b - a;
  }
  return at;
};

c.encodeFramesAt(take.webFrames, path.join(out, "web.mp4"), times, take.webSize);
await replayAt(take.cast, path.join(out, "terminal.mp4"), times.map((t) => t - take.castEpoch));
setup.encodeVideoAt(raw, path.join(out, "phone.mp4"), times.map((t) => Math.max(0, t - take.phoneStarted)));

const report = {
  note: "Times in seconds. `clip` is the time in terminal.mp4, web.mp4, and phone.mp4 (all cut alike); `clock` is seconds since the shot started on the shared wall clock.",
  duration: times.length / 30,
  messages: take.messages,
  cuts: keep.map(([a, b]) => ({ from: +(a - beats.start).toFixed(3), to: +(b - beats.start).toFixed(3) })),
  beats: Object.fromEntries(
    Object.entries(beats).map(([k, t]) => [k, { clip: +clip(t).toFixed(3), clock: +(t - beats.start).toFixed(3) }]),
  ),
};
fs.writeFileSync(path.join(out, "beats.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report.beats, null, 1));
}
