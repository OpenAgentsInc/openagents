/** Offline browser film export. Run through openagents browser run; assets stay private. */
import assert from "node:assert/strict";
import { readFile, writeFile, mkdir, rename, stat } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "node:http";
import { randomUUID, createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { chromium } from "playwright";
import { createBrowserVideoEncoder } from "./film/browser-video-encoder.ts";
import { FilmUploadLedger } from "./film/upload-ledger.ts";
import { validateFilmTimeline, editorialFrame } from "./film/timeline.ts";

const options = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  const key = process.argv[i],
    value = process.argv[i + 1];
  assert(
    [
      "--timeline",
      "--out",
      "--url",
      "--pause-ms",
      "--validate",
      "--codec",
    ].includes(key) &&
      value !== undefined &&
      !options.has(key),
    "Use each named film option once.",
  );
  options.set(key, value);
}
const checkout = resolve(fileURLToPath(new URL("..", import.meta.url))),
  input = resolve(options.get("--timeline") || "");
assert(
  options.has("--timeline"),
  "Set --timeline to a private film JSON file.",
);
assert(
  (await stat(input)).size <= 32 * 1024 * 1024,
  "Keep the film timeline below 32 MiB.",
);
const film = validateFilmTimeline(JSON.parse(await readFile(input, "utf8")));
if (options.get("--validate") === "true") {
  console.log(
    JSON.stringify({
      ok: true,
      frames: film.frames.length,
      width: film.width,
      height: film.height,
      frameRate: film.frameRate,
    }),
  );
  process.exit(0);
}
const output = resolve(options.get("--out") || "");
const partial = output.replace(/\.mp4$/, ".partial.mp4");
assert(
  options.has("--out") &&
    output !== checkout &&
    !output.startsWith(checkout + "/"),
  "Choose a private MP4 output outside this repository.",
);
assert(output.endsWith(".mp4"), "Choose an MP4 output filename.");
for (const filename of [output, partial]) {
  try {
    await stat(filename);
    throw Error(
      "Choose a new film output filename; its partial file must also be unused.",
    );
  } catch (e) {
    if (e.code !== "ENOENT") throw e;
  }
}
const url = new URL(options.get("--url") || "http://127.0.0.1:55304/");
assert(
  ["127.0.0.1", "localhost"].includes(url.hostname),
  "Record a local preview.",
);
const pauseMs = Number(options.get("--pause-ms") ?? 150);
assert(
  Number.isFinite(pauseMs) && pauseMs >= 50 && pauseMs <= 5000,
  "Use a GPU yield from 50 to 5000 milliseconds.",
);
const codec = options.get("--codec") ?? "rgba";
assert(["rgba", "h264"].includes(codec), "Choose rgba or h264 encoding.");
const endpoint =
  process.env.OPENAGENTS_CHROME_WS ||
  (process.env.OPENAGENTS_CHROME_PORT &&
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`);
assert(endpoint, "Run the film tool through openagents browser run.");
await mkdir(dirname(output), { recursive: true });
const token = randomUUID(),
  report = {
    schema: "aethyrforge.film-export.v1",
    ok: false,
    input,
    output,
    width: film.width,
    height: film.height,
    frameRate: film.frameRate,
    frames: [],
    errors: [],
    transportIssues: [],
    resourceSamples: [],
    pauseMs,
    codec,
    startedAt: new Date().toISOString(),
    url: url.href,
  };
const sha = (b) => createHash("sha256").update(b).digest("hex");
report.timelineSHA256 = sha(await readFile(input));
const args = [
  "-hide_banner",
  "-loglevel",
  "warning",
  "-n",
  ...(codec === "h264"
    ? ["-f", "h264"]
    : [
        "-f",
        "rawvideo",
        "-pixel_format",
        "rgba",
        "-video_size",
        `${film.width}x${film.height}`,
      ]),
  "-framerate",
  `${film.frameRate.numerator}/${film.frameRate.denominator}`,
  "-i",
  "pipe:0",
  "-an",
  ...(codec === "h264"
    ? [
        "-c:v",
        "copy",
        "-bsf:v",
        `setts=pts=N*${film.frameRate.denominator}/(${film.frameRate.numerator}*TB):dts=N*${film.frameRate.denominator}/(${film.frameRate.numerator}*TB):duration=${film.frameRate.denominator}/(${film.frameRate.numerator}*TB)`,
        "-video_track_timescale",
        String(film.frameRate.numerator * 1000),
      ]
    : [
        "-c:v",
        "libx264",
        "-preset",
        "fast",
        "-crf",
        "18",
        "-pix_fmt",
        "yuv420p",
      ]),
  "-threads",
  "2",
  "-movflags",
  "+faststart",
  partial,
];
const encoder = spawn(
  process.platform === "win32" ? "ffmpeg" : "nice",
  process.platform === "win32" ? args : ["-n", "10", "ffmpeg", ...args],
  { stdio: ["pipe", "ignore", "pipe"] },
);
let encodingError,
  stderr = "",
  received = 0,
  lastRaw;
encoder.stderr.on("data", (b) => {
  stderr = (stderr + b).slice(-16000);
});
encoder.on("error", (e) => {
  encodingError = e;
});
encoder.stdin.on("error", (e) => {
  encodingError = e;
});
let encoderOutcome;
// Attach both handlers before waiting for the browser or a frame. Spawn failures
// become ordinary outcomes, so a delayed caller never leaves an unhandled rejection.
const encoderExit = new Promise((resolveExit) => {
  encoder.once("error", (error) => {
    encoderOutcome = { error };
    resolveExit(encoderOutcome);
  });
  encoder.once("exit", (code, signal) => {
    encoderOutcome = { code, signal };
    resolveExit(encoderOutcome);
  });
});
await Promise.race([encoderExit, new Promise((r) => setTimeout(r, 100))]);
if (encoderOutcome || encodingError) {
  encoder.stdin.destroy();
  encoder.kill("SIGTERM");
  throw (
    encodingError ||
    encoderOutcome.error ||
    Error(`The encoder stopped before capture: ${stderr}`)
  );
}
const black = Buffer.alloc(film.width * film.height * 4);
for (let i = 3; i < black.length; i += 4) black[i] = 255;
let sampleResources = async () => {};
async function writeFrame(raw, index, renderMs, pixelHash) {
  if (encodingError) throw encodingError;
  const frame = film.frames[index],
    pixels =
      codec === "h264"
        ? raw
        : editorialFrame(raw, frame, film.width, film.height);
  if (!encoder.stdin.write(pixels)) await once(encoder.stdin, "drain");
  report.frames.push({
    index,
    ptsUs: frame.ptsUs,
    worldTimeSeconds: frame.worldTimeSeconds,
    rendered: !frame.black && !frame.holdPrevious,
    sha256: codec === "h264" ? pixelHash : sha(pixels),
    uploadBytes: raw.length,
    renderMs,
  });
  received++;
  if (received % 120 === 0) {
    await sampleResources();
    console.log(
      JSON.stringify({
        frames: received,
        total: film.frames.length,
        videoSeconds: frame.ptsUs / 1e6,
        rendered: report.frames.filter((f) => f.rendered).length,
      }),
    );
  }
}
const uploads = new FilmUploadLedger();
const server = createServer(async (req, res) => {
  res.setHeader("Connection", "close");
  if (req.headers.origin !== url.origin) {
    res.writeHead(403);
    res.end();
    return;
  }
  res.setHeader("Access-Control-Allow-Origin", url.origin);
  res.setHeader("Vary", "Origin");
  if (req.method === "OPTIONS") {
    res.setHeader("Access-Control-Allow-Methods", "POST");
    res.setHeader(
      "Access-Control-Allow-Headers",
      "Content-Type, X-Film-Render-Ms, X-Film-Pixel-Sha",
    );
    res.writeHead(204);
    res.end();
    return;
  }
  const match = new RegExp(`^/${token}/(\\d+)$`).exec(req.url || "");
  if (req.method !== "POST" || !match) {
    res.writeHead(409);
    res.end();
    return;
  }
  if (uploads.busy) {
    res.writeHead(503);
    res.end();
    return;
  }
  try {
    const index = Number(match[1]),
      repeat = uploads.reserve(index),
      frame = film.frames[index];
    if (!frame) throw Error("Unexpected extra film frame.");
    let bytes = 0;
    const parts = [];
    for await (const chunk of req) {
      bytes += chunk.length;
      if (bytes > (codec === "h264" ? 16 * 1024 * 1024 : black.length))
        throw Error("The film frame exceeds its size.");
      parts.push(chunk);
    }
    const uploaded = Buffer.concat(parts),
      uploadHash = sha(uploaded);
    uploads.verify(uploadHash);
    if (repeat) {
      uploads.commit(uploadHash);
      res.writeHead(204);
      res.end();
      return;
    }
    let raw;
    const pixelHash = req.headers["x-film-pixel-sha"];
    if (codec === "h264") {
      assert(
        bytes > 0 &&
          typeof pixelHash === "string" &&
          /^[a-f0-9]{64}$/.test(pixelHash),
      );
      raw = uploaded;
    } else if (frame.black) {
      assert.equal(bytes, 0);
      raw = black;
      lastRaw = undefined;
    } else if (frame.holdPrevious) {
      assert.equal(bytes, 0);
      assert(lastRaw);
      raw = lastRaw;
    } else {
      assert.equal(bytes, black.length);
      raw = uploaded;
      lastRaw = raw;
    }
    const renderMs = Number(req.headers["x-film-render-ms"] ?? 0);
    assert(Number.isFinite(renderMs) && renderMs >= 0 && renderMs <= 180000);
    await writeFrame(raw, index, renderMs, pixelHash);
    uploads.commit(uploadHash);
    assert.equal(uploads.next, received);
    res.writeHead(204);
    res.end();
  } catch (error) {
    if (
      req.aborted ||
      ["ECONNRESET", "ERR_STREAM_PREMATURE_CLOSE"].includes(error.code)
    )
      report.transportIssues.push(String(error));
    else report.errors.push(String(error));
    res.writeHead(500);
    res.end(String(error));
  } finally {
    uploads.release();
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const upload = `http://127.0.0.1:${server.address().port}/${token}`;
let browser, context, page;
try {
  browser = await chromium.connectOverCDP(endpoint);
  report.browser = browser.version();
  sampleResources = async () => {
    report.resourceSamples.push({
      frame: received,
      node: process.memoryUsage(),
    });
  };
  await sampleResources();
  context = await browser.newContext({
    viewport: { width: film.width, height: film.height },
    serviceWorkers: "block",
  });
  page = await context.newPage();
  page.on("pageerror", (e) => report.errors.push(e.message));
  page.on("requestfailed", (request) => {
    const issue = {
      origin: new URL(request.url()).origin,
      error: request.failure()?.errorText,
    };
    report.transportIssues.push(issue);
    console.log(JSON.stringify({ requestFailed: issue }));
  });
  page.on("console", (m) => {
    if (
      m.type() === "error" &&
      /validation|shader|pipeline|GPUDevice|WebGPU/i.test(m.text())
    )
      report.errors.push(m.text());
  });
  await page.goto(url.href);
  await page.waitForFunction(
    () => {
      const s = window.aethyrforge?.snapshot().status;
      if (s?.phase === "error") throw Error(s.message);
      return s?.phase === "ready" && s.frame > 2;
    },
    null,
    { timeout: 150000 },
  );
  report.before = await page.evaluate(() => window.aethyrforge.snapshot());
  await page.evaluate(
    ({ width, height }) =>
      window.aethyrforge.beginFilmCapture({ width, height }),
    film,
  );
  await page.evaluate(
    async ({
      frames,
      upload,
      pauseMs,
      width,
      height,
      frameRate,
      codec,
      encoderSource,
      editorialSource,
    }) => {
      let selected, lastPixels;
      const filmEncoder =
        codec === "h264"
          ? await new Function("return (" + encoderSource + ")")()({
              width,
              height,
              frameRate,
            })
          : undefined;
      const editPixels =
        codec === "h264"
          ? new Function("return (" + editorialSource + ")")()
          : undefined;
      const blackPixels = new Uint8Array(width * height * 4);
      for (let i = 3; i < blackPixels.length; i += 4) blackPixels[i] = 255;
      try {
        for (let i = 0; i < frames.length; i++) {
          const frame = frames[i];
          if (frame.shotId && frame.shotId !== selected) {
            selected = frame.shotId;
            const cameraRequest = window.aethyrforge.camera(selected);
            if (!cameraRequest)
              throw Error(`Camera ${selected} is unavailable in this world.`);
            const start = performance.now();
            while (
              !window.aethyrforge
                .snapshot()
                .receipts.some(
                  (r) =>
                    r.requestId === cameraRequest &&
                    r.ok &&
                    r.message === "Camera " + selected,
                )
            ) {
              const failure = window.aethyrforge
                .snapshot()
                .receipts.find((r) => r.requestId === cameraRequest && !r.ok);
              if (failure) throw Error(failure.message);
              if (performance.now() - start > 30000)
                throw Error("The film camera took too long to respond.");
              await new Promise((r) => setTimeout(r, 10));
            }
          }
          const renderStart = performance.now();
          let pixels;
          if (frame.black) {
            pixels = blackPixels.buffer;
            lastPixels = undefined;
          } else if (frame.holdPrevious) {
            if (codec === "h264") pixels = lastPixels;
          } else {
            pixels = await window.aethyrforge.capturePixels({
              time: frame.worldTimeSeconds,
              camera: frame.camera,
              exposure: frame.exposure,
              ...(frame.transforms === undefined
                ? {}
                : { transforms: frame.transforms }),
            });
            if (codec === "h264") lastPixels = pixels;
          }
          const renderMs =
            frame.black || frame.holdPrevious
              ? 0
              : performance.now() - renderStart;
          let pixelHash = "",
            body;
          if (filmEncoder) {
            const edited = editPixels(
              new Uint8Array(pixels),
              frame,
              width,
              height,
            );
            const hash = new Uint8Array(
              await crypto.subtle.digest("SHA-256", edited),
            );
            pixelHash = Array.from(hash, (n) =>
              n.toString(16).padStart(2, "0"),
            ).join("");
            body = await filmEncoder.encode(edited.buffer, i);
          } else
            body =
              frame.black || frame.holdPrevious ? new Uint8Array() : pixels;
          pixels = undefined;
          let accepted = false;
          for (let attempt = 0; attempt < 5; attempt++) {
            try {
              const response = await fetch(upload + "/" + i, {
                method: "POST",
                headers: {
                  "Content-Type": "application/octet-stream",
                  "X-Film-Render-Ms": String(renderMs),
                  "X-Film-Pixel-Sha": pixelHash,
                },
                body,
              });
              if (response.status === 503)
                throw TypeError("The previous upload is still finishing.");
              if (!response.ok) throw Error(await response.text());
              accepted = true;
              break;
            } catch (error) {
              if (!(error instanceof TypeError) || attempt === 4) throw error;
              await new Promise((r) => setTimeout(r, 100 * 2 ** attempt));
            }
          }
          if (!accepted) throw Error("The film frame could not upload.");
          body = undefined;
          if (!frame.black && !frame.holdPrevious)
            await new Promise((r) => setTimeout(r, pauseMs));
        }
      } finally {
        filmEncoder?.close();
        lastPixels = undefined;
      }
    },
    {
      frames: film.frames,
      upload,
      pauseMs,
      width: film.width,
      height: film.height,
      frameRate: film.frameRate,
      codec,
      encoderSource: createBrowserVideoEncoder.toString(),
      editorialSource: editorialFrame.toString(),
    },
  );
  assert.equal(received, film.frames.length);
  assert.deepEqual(report.errors, []);
  report.after = await page.evaluate(() => window.aethyrforge.snapshot());
  await page.evaluate(() => window.aethyrforge.endFilmCapture());
  encoder.stdin.end();
  const outcome = await encoderExit;
  if (outcome.error) throw outcome.error;
  assert.equal(outcome.code, 0, stderr);
  await rename(partial, output);
  report.ok = true;
  report.bytes = (await stat(output)).size;
  console.log(
    JSON.stringify({ ok: true, output, frames: received, bytes: report.bytes }),
  );
} catch (error) {
  report.failure = String(error.stack || error);
  process.exitCode = 1;
  console.error(report.failure);
} finally {
  try {
    if (page && !page.isClosed())
      await page.evaluate(() => {
        window.aethyrforge.cancelCapture();
        return window.aethyrforge.endFilmCapture();
      });
  } catch {}
  await context?.close();
  await browser?.close();
  server.close();
  if (!report.ok) {
    encoder.stdin.destroy();
    encoder.kill("SIGTERM");
  }
  report.encoderLog = stderr;
  report.finishedAt = new Date().toISOString();
  report.elapsedSeconds =
    (Date.parse(report.finishedAt) - Date.parse(report.startedAt)) / 1000;
  const samples = report.frames
    .filter((frame) => frame.rendered)
    .map((frame) => frame.renderMs)
    .sort((a, b) => a - b);
  report.captureTimeMs = {
    count: samples.length,
    median: samples[Math.floor(samples.length / 2)],
    p95: samples[Math.floor(samples.length * 0.95)],
  };
  await writeFile(output + ".json", JSON.stringify(report, null, 2) + "\n");
}
