// Films concept-voice.html, the Verse trailer's mock-up phone screen
// (#12458), frame by frame at the phone's size (393x852 CSS px at 3x).
//
//   node scripts/film/footage/concept-voice.mjs [OUT.mp4] [--seconds 9.5] [--variant haunt]
//
// Writes ~/work/film-out/verse-trailer/footage/concept-voice.mp4 by default.
// `--variant haunt` films the second cut's screen: the Haunted House.
// The page shows a feature that doesn't exist yet; the cut tags it CONCEPT.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { launch, navigate, evaluate, shoot, encodeFrames } from "./cdp.mjs";

const args = process.argv.slice(2);
const at = args.indexOf("--seconds");
const seconds = at >= 0 ? Number(args[at + 1]) : 9.5;
const vi = args.indexOf("--variant");
const variant = vi >= 0 ? args[vi + 1] : "";
const out = (args.find((a, i) => !a.startsWith("--") && !["--seconds", "--variant"].includes(args[i - 1])) ||
  "~/work/film-out/verse-trailer/footage/concept-voice.mp4").replace(/^~/, os.homedir());
const fonts = (process.env.FILM_FONTS || "~/work/film-out/fonts").replace(/^~/, os.homedir());
const page = path.join(path.dirname(fileURLToPath(import.meta.url)), "concept-voice.html");

const cdp = await launch({ width: 393, height: 852, dpr: 3, mobile: true });
await navigate(cdp, pathToFileURL(page).href + (variant ? `?v=${encodeURIComponent(variant)}` : ""));
await evaluate(cdp, `(async () => {
  const faces = [["Geist", "Geist-Regular.ttf", 400], ["Geist", "Geist-Regular.ttf", 600],
    ["Geist Mono", "GeistMono-Medium.ttf", 500], ["Geist Mono", "GeistMono-Regular.ttf", 400]];
  for (const [family, file, weight] of faces) {
    const f = new FontFace(family, "url(${pathToFileURL(fonts).href}/" + file + ")", { weight: String(weight) });
    document.fonts.add(await f.load());
  }
  return true;
})()`);
const dir = fs.mkdtempSync(path.join(os.tmpdir(), "concept-voice-"));
const frames = Math.round(seconds * 30);
for (let k = 0; k < frames; k++) {
  await evaluate(cdp, `window.filmTime = ${k / 30}; window.draw(); true`);
  await shoot(cdp, dir, k);
}
await cdp.close();
encodeFrames(dir, out, 393 * 3, 852 * 3);
fs.rmSync(dir, { recursive: true, force: true });
console.log(out);
