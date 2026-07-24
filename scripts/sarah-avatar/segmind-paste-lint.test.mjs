import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  assertBritishZedPronunciationSafe,
  findBritishZedPronunciationRisks,
  isBritishVoiceSelection,
} from "./segmind-paste-lint-lib.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const cli = join(here, "segmind-talking-avatar.mjs");

test("recognizes British voice and language selectors", () => {
  assert.equal(
    isBritishVoiceSelection({
      voice: "Narrator (British)",
      voiceLanguage: "English (US)",
    }),
    true,
  );
  assert.equal(isBritishVoiceSelection({ voice: "Leda", voiceLanguage: "English (UK)" }), true);
  assert.equal(isBritishVoiceSelection({ voice: "Leda", voiceLanguage: "en-GB" }), true);
  assert.equal(isBritishVoiceSelection({ voice: "Leda", voiceLanguage: "English (US)" }), false);
});

test("finds each standalone uppercase Zed in British spoken paste", () => {
  const risks = findBritishZedPronunciationRisks({
    script: "We fork Zed. Zed-based tools remain compatible with zed.",
    voice: "Leda (British)",
    voiceLanguage: "English (UK)",
  });

  assert.deepEqual(
    risks.map(({ value }) => value),
    ["Zed", "Zed"],
  );
});

test("does not flag lowercase zed, embedded text, or non-British selection", () => {
  assert.deepEqual(
    findBritishZedPronunciationRisks({
      script: "We fork zed. Zedless is not the product name.",
      voice: "Leda",
      voiceLanguage: "English (UK)",
    }),
    [],
  );
  assert.deepEqual(
    findBritishZedPronunciationRisks({
      script: "We fork Zed.",
      voice: "Leda",
      voiceLanguage: "English (US)",
    }),
    [],
  );
});

test("blocks risky British paste with an actionable override", () => {
  assert.throws(
    () =>
      assertBritishZedPronunciationSafe({
        script: "We fork Zed.",
        voice: "Leda",
        voiceLanguage: "English (UK)",
      }),
    /write lowercase "zed".*--allow-pronunciation-risk/,
  );

  const risks = assertBritishZedPronunciationSafe({
    script: "We fork Zed.",
    voice: "Leda",
    voiceLanguage: "English (UK)",
    allowPronunciationRisk: true,
  });
  assert.equal(risks.length, 1);
});

test("CLI blocks risky paste before resolving credentials or spending", () => {
  const result = spawnSync(
    process.execPath,
    [
      cli,
      "--image",
      "https://example.com/sarah.png",
      "--script",
      "We fork Zed.",
      "--voice-language",
      "English (UK)",
    ],
    {
      encoding: "utf8",
      env: {
        ...process.env,
        SEGMIND_API_KEY: "",
        SEGMIND_ENV_FILE: "/dev/null",
      },
    },
  );

  assert.equal(result.status, 1);
  assert.match(result.stderr, /write lowercase "zed"/);
  assert.doesNotMatch(result.stderr, /no Segmind API key/);
});

test("CLI override warns and continues to the credential gate", () => {
  const result = spawnSync(
    process.execPath,
    [
      cli,
      "--image",
      "https://example.com/sarah.png",
      "--script",
      "We fork Zed.",
      "--voice-language",
      "en-GB",
      "--allow-pronunciation-risk",
    ],
    {
      encoding: "utf8",
      env: {
        ...process.env,
        SEGMIND_API_KEY: "",
        SEGMIND_ENV_FILE: "/dev/null",
      },
    },
  );

  assert.equal(result.status, 1);
  assert.match(result.stderr, /overriding 1 uppercase "Zed"/);
  assert.match(result.stderr, /no Segmind API key/);
});
