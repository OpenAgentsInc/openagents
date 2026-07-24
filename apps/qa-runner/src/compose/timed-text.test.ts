import { describe, expect, test } from "vite-plus/test";
import {
  buildCueHtml,
  buildTimedTextFfmpegArgs,
  escapeHtml,
  parseTimedTextArgs,
  parseVideoProbe,
  validateTimedTextManifest,
  type TimedTextManifest,
} from "./timed-text.ts";

const manifest: TimedTextManifest = {
  schemaVersion: "openagents.media.timed_text.v1",
  cues: [
    {
      startSeconds: 0,
      endSeconds: 2.5,
      text: "PROJECT OMEGA\nYOUR LAST IDE",
      style: "title",
    },
    {
      startSeconds: 3,
      endSeconds: 6,
      text: "ZED - CURRENT",
      style: "state-label",
    },
  ],
};

describe("timed-text manifest", () => {
  test("accepts bounded cues inside the video duration", () => {
    expect(validateTimedTextManifest(manifest, 6)).toEqual(manifest);
  });

  test("rejects empty, inverted, oversized, and out-of-range cues", () => {
    expect(() => validateTimedTextManifest({ ...manifest, cues: [] }, 6)).toThrow(
      "at least one cue",
    );
    expect(() =>
      validateTimedTextManifest(
        {
          ...manifest,
          cues: [{ ...manifest.cues[0], startSeconds: 4, endSeconds: 2 }],
        },
        6,
      ),
    ).toThrow("greater than startSeconds");
    expect(() =>
      validateTimedTextManifest(
        {
          ...manifest,
          cues: [{ ...manifest.cues[0], text: "x".repeat(281) }],
        },
        6,
      ),
    ).toThrow("280-character limit");
    expect(() => validateTimedTextManifest(manifest, 5)).toThrow("ends after the source video");
  });
});

describe("timed-text frame rendering", () => {
  test("escapes markup and preserves line breaks", () => {
    expect(escapeHtml('<Omega & "Zed">')).toBe("&lt;Omega &amp; &quot;Zed&quot;&gt;");
    const html = buildCueHtml(
      {
        startSeconds: 0,
        endSeconds: 1,
        text: "<Omega>\nCURRENT",
        style: "state-label",
      },
      1280,
      720,
    );
    expect(html).toContain("&lt;Omega&gt;<br>CURRENT");
    expect(html).not.toContain("<Omega>");
    expect(html).toContain("1280px");
    expect(html).toContain("720px");
  });
});

describe("timed-text ffmpeg plan", () => {
  test("times each PNG overlay and preserves optional source audio", () => {
    const args = buildTimedTextFfmpegArgs(
      "/input/source.mp4",
      ["/tmp/cue-000.png", "/tmp/cue-001.png"],
      manifest,
      "/output/episode.mp4",
      "copy",
    );
    const graph = args[args.indexOf("-filter_complex") + 1];
    expect(graph).toContain("between(t\\,0\\,2.5)");
    expect(graph).toContain("between(t\\,3\\,6)");
    expect(args).toContain("0:a?");
    expect(args[args.indexOf("-c:a") + 1]).toBe("copy");
    expect(args.at(-1)).toBe("/output/episode.mp4");
  });

  test("can encode source audio as AAC", () => {
    const args = buildTimedTextFfmpegArgs(
      "source.mov",
      ["cue.png", "cue2.png"],
      manifest,
      "episode.mp4",
      "aac",
    );
    expect(args[args.indexOf("-c:a") + 1]).toBe("aac");
    expect(args[args.indexOf("-b:a") + 1]).toBe("192k");
  });
});

describe("timed-text probe and CLI", () => {
  test("reads video dimensions, duration, and audio presence", () => {
    expect(
      parseVideoProbe(
        JSON.stringify({
          streams: [{ codec_type: "video", width: 1920, height: 1080 }, { codec_type: "audio" }],
          format: { duration: "42.75" },
        }),
      ),
    ).toEqual({
      width: 1920,
      height: 1080,
      durationSeconds: 42.75,
      hasAudio: true,
    });
  });

  test("parses an explicit production command", () => {
    expect(
      parseTimedTextArgs([
        "--input",
        "raw.mp4",
        "--cues",
        "cues.json",
        "--out",
        "final.mp4",
        "--audio",
        "aac",
        "--force",
      ]),
    ).toEqual({
      input: "raw.mp4",
      cues: "cues.json",
      out: "final.mp4",
      audioMode: "aac",
      force: true,
    });
  });
});
