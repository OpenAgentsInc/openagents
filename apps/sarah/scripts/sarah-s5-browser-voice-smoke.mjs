import { execFile as execFileCallback } from "node:child_process";
import { mkdir, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import { chromium } from "playwright-core";

const execFile = promisify(execFileCallback);

const baseUrl = (process.env.SARAH_S5_SMOKE_BASE_URL ?? "http://127.0.0.1:8790/sarah").replace(
  /\/+$/,
  "",
);
const chromePath =
  process.env.SARAH_S5_CHROME_PATH ??
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const promptText =
  process.env.SARAH_S5_VOICE_PROMPT ??
  "Sarah, use your demo sales context tool to show me proof your tools are connected, then tell me the result out loud.";
const sayVoice = process.env.SARAH_S5_SAY_VOICE ?? "Samantha";
const keepArtifacts = process.env.SARAH_S5_KEEP_ARTIFACTS === "1";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function makeVoiceFixture(workDir) {
  if (process.env.SARAH_S5_VOICE_AUDIO_PATH) {
    return {
      audioPath: process.env.SARAH_S5_VOICE_AUDIO_PATH,
      source: "SARAH_S5_VOICE_AUDIO_PATH",
      voice: null,
    };
  }

  const promptAiff = join(workDir, "sarah-s5-prompt.aiff");
  const promptWav = join(workDir, "sarah-s5-prompt-with-silence.wav");
  await execFile("say", ["-v", sayVoice, "-o", promptAiff, promptText]);
  await execFile("ffmpeg", [
    "-y",
    "-hide_banner",
    "-loglevel",
    "error",
    "-i",
    promptAiff,
    "-f",
    "lavfi",
    "-t",
    "3",
    "-i",
    "anullsrc=channel_layout=mono:sample_rate=48000",
    "-filter_complex",
    "[0:a]aresample=48000,pan=mono|c0=c0[a0];[a0][1:a]concat=n=2:v=0:a=1",
    "-ac",
    "1",
    "-ar",
    "48000",
    promptWav,
  ]);

  return {
    audioPath: promptWav,
    source: "macOS say + ffmpeg silence fixture",
    voice: sayVoice,
  };
}

async function waitForProof(page) {
  return page.waitForFunction(
    () => {
      const bodyText = document.body.innerText;
      return (
        /Sarah, use your demo sales.*tools are connected/i.test(bodyText) &&
        /tell me the result out loud/i.test(bodyText) &&
        (bodyText.match(/\nSARAH\n/g) ?? []).length >= 2 &&
        /demo .*success|tool returned.*result|returned.*result|call succeeded/i.test(
          bodyText,
        ) &&
        bodyText.includes("Tool outcomes") &&
        bodyText.includes("demo sales context") &&
        bodyText.includes("tool bridge")
      );
    },
    null,
    { timeout: 90_000 },
  );
}

async function run() {
  const workDir = await mkdtemp(join(tmpdir(), "sarah-s5-browser-voice-"));
  const artifactDir =
    process.env.SARAH_S5_ARTIFACT_DIR ??
    (keepArtifacts ? join(workDir, "artifacts") : null);
  if (artifactDir) await mkdir(artifactDir, { recursive: true });

  const fixture = await makeVoiceFixture(workDir);
  const browser = await chromium.launch({
    executablePath: chromePath,
    headless: false,
    args: [
      "--use-fake-device-for-media-stream",
      "--use-fake-ui-for-media-stream",
      `--use-file-for-fake-audio-capture=${fixture.audioPath}`,
      "--autoplay-policy=no-user-gesture-required",
      "--no-first-run",
      "--no-default-browser-check",
    ],
  });

  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    await page.goto(`${baseUrl}/`, { waitUntil: "domcontentloaded" });
    await page.getByRole("button", { name: "Connect" }).click();
    await page.getByRole("button", { name: "Disconnect" }).waitFor({
      state: "visible",
      timeout: 45_000,
    });
    await page.getByRole("button", { name: "Start mic" }).click();
    await page.getByRole("button", { name: "Stop mic" }).waitFor({
      state: "visible",
      timeout: 15_000,
    });
    await sleep(8_000);
    await page.getByRole("button", { name: "Stop mic" }).click();
    try {
      await waitForProof(page);
    } catch (error) {
      const bodyText = await page.locator("body").innerText();
      throw new Error(
        `Browser voice proof did not appear before timeout. Last body:\n${bodyText}`,
        { cause: error },
      );
    }

    const bodyText = await page.locator("body").innerText();
    const screenshotPath = artifactDir
      ? join(artifactDir, "sarah-s5-browser-voice-smoke.png")
      : null;
    if (screenshotPath) await page.screenshot({ path: screenshotPath, fullPage: true });

    const evidence = {
      schema: "sarah.s5_browser_voice_smoke.v1",
      generatedAt: new Date().toISOString(),
      baseUrl,
      prompt: promptText,
      fakeMicrophoneFixture: fixture,
      chrome: {
        path: chromePath,
        automation: "playwright-core with system Chrome",
      },
      assertions: {
        transcribedSpokenPrompt:
          /Sarah, use your demo sales.*tools are connected/i.test(bodyText) &&
          /tell me the result out loud/i.test(bodyText),
        spokeToolResult:
          (bodyText.match(/\nSARAH\n/g) ?? []).length >= 2 &&
          /demo .*success|tool returned.*result|returned.*result|call succeeded/i.test(
            bodyText,
          ),
        renderedToolReceipt: bodyText.includes("Tool outcomes"),
        invokedDemoSalesContext: bodyText.includes("demo sales context"),
        returnedToolBridgeSummary: bodyText.includes("tool bridge"),
      },
      artifactPaths: {
        screenshot: screenshotPath,
      },
      bodyText,
    };

    console.log(JSON.stringify(evidence, null, 2));
  } finally {
    await browser.close();
  }
}

await run();
