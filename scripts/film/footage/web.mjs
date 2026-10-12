// Web helpers for the launch film footage (#11432): a signed-in, dark,
// desktop-sized browser on the local site, and asking a question the way a
// person does (click the composer, type, press Enter).
import fs from "node:fs";
import * as c from "./cdp.mjs";

export const DESKTOP = { width: 1512, height: 982, dpr: 2 };

export async function signedIn(base, sessionFile, size = DESKTOP) {
  const token = fs.readFileSync(sessionFile, "utf8").trim();
  const cdp = await c.launch(size);
  for (const [name, value] of [["oa_cloud_session", token], ["oa_theme", "dark"]]) {
    await cdp.send("Network.setCookie", { name, value, url: `${base}/` });
  }
  return cdp;
}

/// True while the open chat is still answering.
export const ANSWERING = `(() => { const t = document.querySelector('.oa-thread'); return !!t && /Working/.test(t.innerText); })()`;

/// Types `question` into the composer and sends it.
export async function ask(cdp, question, typing = {}) {
  await c.click(cdp, "textarea[name=q]");
  await c.sleep(250);
  await c.type(cdp, question, typing);
  await c.sleep(typing.beforeSend ?? 350);
  await c.pressEnter(cdp);
}

/// How much answer text the open chat shows.
export const ANSWER_TEXT = `[...document.querySelectorAll('.oa-thread .oa-message[data-role=assistant] .oa-markdown')].map(e => e.innerText).join('').trim().length`;

/// Waits for the answer in the open chat to finish and stop moving.
export async function answered(cdp, timeoutMs = 120000) {
  await c.waitFor(cdp, `location.pathname.startsWith('/chat/')`, 20000, "the chat page");
  await c.sleep(1500);
  await c.waitFor(cdp, `!${ANSWERING} && ${ANSWER_TEXT} > 0`, timeoutMs, "the answer");
  // The answer is revealed with a short animation; wait until it is still.
  let last = "";
  for (let still = 0; still < 8; ) {
    await c.sleep(150);
    const now = await c.evaluate(cdp, `document.querySelector('.oa-thread').getBoundingClientRect().height + ':' + document.querySelector('.oa-thread').innerText.length + ':' + [...document.querySelectorAll('.oa-thread *')].filter(e => e.getAnimations && e.getAnimations().length).length`);
    still = now === last ? still + 1 : 0;
    last = now;
  }
}

/// Deletes the open chat through its own Delete page.
export async function deleteOpenChat(cdp, base) {
  const p = await c.evaluate(cdp, "location.pathname");
  await c.navigate(cdp, `${base}${p}/delete`);
  await c.sleep(400);
  await c.click(cdp, await c.byText(cdp, "button", "Delete"));
  await c.sleep(1200);
}

/// Asks each question on its own new chat and waits for its answer.
export async function seed(base, sessionFile, questions) {
  const cdp = await signedIn(base, sessionFile, { width: 1512, height: 982, dpr: 1 });
  for (const q of questions) {
    await c.navigate(cdp, `${base}/`);
    await c.sleep(700);
    await ask(cdp, q, { base: 2, jitter: 2 });
    await answered(cdp);
    console.log(`seeded: ${q}`);
  }
  await cdp.close();
}
