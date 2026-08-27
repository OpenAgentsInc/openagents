import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  assertNoMusicPlateUntouched,
  buildMixFilterComplex,
  buildMixFfmpegArgs,
  buildMusicRequestBody,
  DEFAULT_MODEL_ID,
  DEFAULT_PROMPT,
  defaultElevenLabsEnvPath,
  deriveBedPath,
  deriveWithMusicPath,
  durationSecToMusicLengthMs,
  generateMusicBedBytes,
  musicApiUrl,
  resolveElevenLabsApiKey,
} from "./elevenlabs-music-bed-lib.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const cliSource = readFileSync(join(here, "elevenlabs-music-bed.mjs"), "utf8");
const libSource = readFileSync(join(here, "elevenlabs-music-bed-lib.mjs"), "utf8");

test("resolveElevenLabsApiKey prefers env over file", () => {
  const key = resolveElevenLabsApiKey({
    env: { ELEVENLABS_API_KEY: " from-env " },
    readFileSync: () => "ELEVENLABS_API_KEY=from-file",
  });
  assert.equal(key, "from-env");
});

test("resolveElevenLabsApiKey reads secrets file when env unset", () => {
  const key = resolveElevenLabsApiKey({
    env: { HOME: "/tmp/fake-home" },
    envFile: "/tmp/fake-home/work/.secrets/elevenlabs.env",
    readFileSync: (p) => {
      assert.match(p, /elevenlabs\.env$/);
      return "# comment\nELEVENLABS_API_KEY=file-secret-value\n";
    },
  });
  assert.equal(key, "file-secret-value");
});

test("resolveElevenLabsApiKey returns undefined when missing", () => {
  const key = resolveElevenLabsApiKey({
    env: {},
    envFile: "/no/such/elevenlabs.env",
    readFileSync: () => {
      throw new Error("ENOENT");
    },
  });
  assert.equal(key, undefined);
});

test("defaultElevenLabsEnvPath points at ~/work/.secrets/elevenlabs.env", () => {
  assert.equal(
    defaultElevenLabsEnvPath("/Users/operator"),
    "/Users/operator/work/.secrets/elevenlabs.env",
  );
});

test("buildMusicRequestBody uses music_v2 and force_instrumental", () => {
  const body = buildMusicRequestBody({ musicLengthMs: 44640 });
  assert.equal(body.model_id, DEFAULT_MODEL_ID);
  assert.equal(body.force_instrumental, true);
  assert.equal(body.music_length_ms, 44640);
  assert.equal(body.prompt, DEFAULT_PROMPT);
});

test("buildMusicRequestBody rejects out-of-range lengths", () => {
  assert.throws(() => buildMusicRequestBody({ musicLengthMs: 100 }), /between/);
  assert.throws(() => buildMusicRequestBody({ musicLengthMs: 700000 }), /between/);
  assert.throws(() => buildMusicRequestBody({ musicLengthMs: 1.5 }), /integer/);
});

test("durationSecToMusicLengthMs rounds to ms", () => {
  assert.equal(durationSecToMusicLengthMs(44.64), 44640);
  assert.equal(durationSecToMusicLengthMs(2.1), 3000);
});

test("deriveWithMusicPath and deriveBedPath preserve plate naming", () => {
  const rc = "/tmp/Sarah/262/262-rc-no-music.mp4";
  assert.equal(deriveWithMusicPath(rc), "/tmp/Sarah/262/262-rc-with-music.mp4");
  assert.equal(deriveBedPath({ rcPath: rc }), "/tmp/Sarah/262/262-music-bed.mp3");
  assert.equal(
    deriveBedPath({ outDir: "/tmp/Sarah/262", episode: "262" }),
    "/tmp/Sarah/262/262-music-bed.mp3",
  );
});

test("assertNoMusicPlateUntouched refuses same path", () => {
  assert.throws(
    () =>
      assertNoMusicPlateUntouched("/tmp/missing-rc-no-music.mp4", "/tmp/missing-rc-no-music.mp4"),
    /refuse to overwrite/,
  );
});

test("buildMixFilterComplex matches Ep261 louder recipe", () => {
  const filter = buildMixFilterComplex({ durationSec: 44.64 });
  assert.match(filter, /loudnorm/);
  assert.match(filter, /volume=-3dB/);
  assert.match(filter, /afade=t=in:st=0:d=1\.5/);
  assert.match(filter, /afade=t=out:st=41\.64:d=3/);
  assert.match(filter, /amix=inputs=2:duration=first/);
  assert.match(filter, /alimiter/);
  assert.match(filter, /\[aout\]$/);
});

test("buildMixFfmpegArgs copies video and remuxes audio", () => {
  const args = buildMixFfmpegArgs({
    rcPath: "/tmp/262-rc-no-music.mp4",
    bedPath: "/tmp/262-music-bed.mp3",
    outPath: "/tmp/262-rc-with-music.mp4",
    durationSec: 40,
  });
  assert.ok(args.includes("-filter_complex"));
  assert.ok(args.includes("-c:v"));
  assert.ok(args.includes("copy"));
  assert.equal(args[args.length - 1], "/tmp/262-rc-with-music.mp4");
});

test("musicApiUrl pins mp3_48000_192 query", () => {
  assert.equal(musicApiUrl(), "https://api.elevenlabs.io/v1/music?output_format=mp3_48000_192");
});

test("generateMusicBedBytes uses mocked fetch (no live ElevenLabs)", async () => {
  const calls = [];
  const fakeBytes = Buffer.from("ID3fake-mp3");
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    return {
      ok: true,
      status: 200,
      headers: { get: () => "audio/mpeg" },
      arrayBuffer: async () =>
        fakeBytes.buffer.slice(fakeBytes.byteOffset, fakeBytes.byteOffset + fakeBytes.byteLength),
    };
  };
  const result = await generateMusicBedBytes({
    apiKey: "test-key-not-real",
    musicLengthMs: 10000,
    prompt: "Instrumental sparse pad. No vocals.",
    fetchImpl,
  });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://api.elevenlabs.io/v1/music?output_format=mp3_48000_192");
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.headers["xi-api-key"], "test-key-not-real");
  const body = JSON.parse(calls[0].init.body);
  assert.equal(body.model_id, "music_v2");
  assert.equal(body.force_instrumental, true);
  assert.equal(body.music_length_ms, 10000);
  assert.deepEqual(Buffer.from(result.bytes), fakeBytes);
});

test("generateMusicBedBytes surfaces HTTP errors without raw key dump", async () => {
  await assert.rejects(
    () =>
      generateMusicBedBytes({
        apiKey: "secret",
        musicLengthMs: 10000,
        fetchImpl: async () => ({
          ok: false,
          status: 401,
          text: async () => "xi-api-key invalid secret",
        }),
      }),
    /HTTP 401.*\[redacted\]/,
  );
});

test("CLI wires secret path, --mix, and never hardcodes a key", () => {
  assert.match(cliSource, /elevenlabs\.env/);
  assert.match(cliSource, /--mix/);
  assert.match(cliSource, /rc-no-music/);
  assert.match(cliSource, /rc-with-music/);
  assert.match(cliSource, /loudnorm/);
  assert.doesNotMatch(cliSource, /ELEVENLABS_API_KEY=[A-Za-z0-9]{8,}/);
  assert.doesNotMatch(libSource, /ELEVENLABS_API_KEY=[A-Za-z0-9]{8,}/);
  assert.match(cliSource, /generateMusicBedBytes/);
});
