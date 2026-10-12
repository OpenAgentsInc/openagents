// phone-approve.mp4: a real device sign-in. `coder login` runs in a real
// terminal and asks to sign in; a phone-sized browser, signed in to the
// account, opens /device, types the code the terminal shows, and approves.
// The terminal then signs in and chooses to sync all its chats.
//
// node phone-approve.mjs  (env: FILM_BASE, FILM_SESSION_FILE, FILM_CODER,
//   FILM_CODER_STATE, FILM_OUT, FILM_WORK)
import fs from "node:fs";
import path from "node:path";
import * as c from "./cdp.mjs";
import { Session } from "./terminal.mjs";

const env = process.env;
const base = env.FILM_BASE;
const token = fs.readFileSync(env.FILM_SESSION_FILE, "utf8").trim();
const out = path.join(env.FILM_OUT, "phone-approve.mp4");
const IPHONE_UA =
  "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";

const login = new Session({
  cols: 100,
  rows: 24,
  command: [env.FILM_CODER, "login", "--state", env.FILM_CODER_STATE],
  cwd: env.HOME,
  env: {
    OPENAGENTS_ORIGIN: base,
    OPENAGENTS_COMPUTER_NAME: env.FILM_COMPUTER || "MacBook Pro",
    // No browser opens on a remote shell; the phone approves instead.
    SSH_CONNECTION: "film",
  },
});
await login.waitFor(/[A-Z]{4}-[A-Z]{4}/, 30000);
const code = login.screen().match(/[A-Z]{4}-[A-Z]{4}/)[0];
console.log(`phone-approve: the terminal shows ${code}`);

const cdp = await c.launch({ width: 393, height: 852, dpr: 3, mobile: true, userAgent: IPHONE_UA });
for (const [name, value] of [["oa_cloud_session", token], ["oa_theme", "dark"]]) {
  await cdp.send("Network.setCookie", { name, value, url: `${base}/` });
}
await c.navigate(cdp, `${base}/device`);
await c.waitFor(cdp, `!!document.querySelector('input[name=code]')`, 10000);
await c.sleep(800);
const rec = new c.Recorder(cdp, path.join(env.FILM_WORK, "phone-frames"));
await rec.start();
await c.sleep(900);
await c.click(cdp, "input[name=code]", { touch: true });
await c.sleep(500);
await c.type(cdp, code, { base: 140, jitter: 80 });
await c.sleep(600);
await c.click(cdp, await c.byText(cdp, "button", "Continue"), { touch: true });
await c.waitFor(cdp, `!!document.querySelector('button[value=approve]')`, 10000);
await c.sleep(1900);
await c.click(cdp, await c.byText(cdp, "button", "Approve"), { touch: true });
await c.waitFor(cdp, `document.body.innerText.includes('is signed in')`, 15000);
await c.sleep(2000);
await rec.stop();
rec.encode(out);
await cdp.close();

await login.waitFor("Signed in to OpenAgents", 30000);
await login.settle(1500, 15000);
console.log(login.screen().trim());
// The one-time question: where this computer's chats live. 2 keeps them
// here for now; the terminal clip turns sync on with /sync on.
if (/Type 1 or 2/.test(login.screen())) {
  await login.type("2\r");
}
await Promise.race([login.exited, c.sleep(20000)]);
login.save(path.join(env.FILM_WORK, "login.cast"));
login.kill();
console.log(`phone-approve: ${out}`);
