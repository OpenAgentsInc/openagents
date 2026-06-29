import { dirname, resolve } from "node:path"

import {
  findIsolatedSceneDefinition,
  isolatedSceneUsage,
  type IsolatedSceneDefinition,
} from "./registry.js"

export type SceneCaptureTarget =
  | Readonly<{
      kind: "registered-scene"
      scene: IsolatedSceneDefinition
      outputPath: string
      pageQuery?: string
    }>
  | Readonly<{
      kind: "url"
      url: string
      outputPath: string
    }>

export type SceneCaptureParseResult =
  | Readonly<{ ok: true; target: SceneCaptureTarget }>
  | Readonly<{ ok: false; message: string }>

const isUrl = (value: string): boolean =>
  value.startsWith("http://") ||
  value.startsWith("https://") ||
  value.startsWith("file://")

const splitSceneQuery = (
  value: string,
): Readonly<{ sceneName: string; pageQuery?: string }> => {
  const index = value.indexOf("?")
  if (index < 0) return { sceneName: value }
  const sceneName = value.slice(0, index)
  const pageQuery = value.slice(index + 1)
  return pageQuery.length === 0 ? { sceneName } : { sceneName, pageQuery }
}

export const parseSceneCaptureArgs = (
  argv: ReadonlyArray<string>,
  cwd = process.cwd(),
): SceneCaptureParseResult => {
  const [targetArg, outputArg] = argv
  if (targetArg === undefined || outputArg === undefined) {
    return {
      ok: false,
      message:
        "Usage: bun scripts/capture-scene-headless.ts <scene-name|url> <out.png>\n" +
        `       ${isolatedSceneUsage()}`,
    }
  }
  const outputPath = resolve(cwd, outputArg)
  if (isUrl(targetArg)) {
    return {
      ok: true,
      target: { kind: "url", url: targetArg, outputPath },
    }
  }

  const { sceneName, pageQuery } = splitSceneQuery(targetArg)
  const scene = findIsolatedSceneDefinition(sceneName)
  if (scene === null) {
    return {
      ok: false,
      message: `Unknown capture target '${targetArg}'. ${isolatedSceneUsage()}`,
    }
  }

  return {
    ok: true,
    target: {
      kind: "registered-scene",
      scene,
      outputPath,
      ...(pageQuery === undefined ? {} : { pageQuery }),
    },
  }
}

export const captureOutputDir = (target: SceneCaptureTarget): string =>
  dirname(target.outputPath)
