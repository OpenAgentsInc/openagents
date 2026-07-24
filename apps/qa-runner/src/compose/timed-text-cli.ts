#!/usr/bin/env node

import { Effect } from "effect";
import { parseTimedTextArgs, renderTimedText } from "./timed-text.ts";

function requireValue(value: string | undefined, flag: string): string {
  if (value === undefined || value.trim().length === 0) {
    throw new Error(`timed-text: ${flag} is required`);
  }
  return value;
}

async function main(): Promise<void> {
  const args = parseTimedTextArgs(process.argv.slice(2));
  const result = await Effect.runPromise(
    renderTimedText({
      inputPath: requireValue(args.input, "--input"),
      cuePath: requireValue(args.cues, "--cues"),
      outputPath: requireValue(args.out, "--out"),
      audioMode: args.audioMode,
      force: args.force,
      ...(args.ffmpeg !== undefined ? { ffmpegBin: args.ffmpeg } : {}),
      ...(args.ffprobe !== undefined ? { ffprobeBin: args.ffprobe } : {}),
    }),
  );

  process.stdout.write(
    `${JSON.stringify(
      {
        schemaVersion: "openagents.media.timed_text_render.v1",
        outputPath: result.outputPath,
        width: result.width,
        height: result.height,
        durationSeconds: result.durationSeconds,
        sourceAudioPresent: result.hasAudio,
        audioMode: result.audioMode,
        cueCount: result.cueCount,
      },
      null,
      2,
    )}\n`,
  );
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${message}\n`);
  process.exitCode = 1;
});
