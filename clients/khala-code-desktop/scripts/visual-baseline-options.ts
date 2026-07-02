import { resolve } from "node:path"

export type KhalaCodeVisualBaselineOptions = Readonly<{
  baselineDir: string
  bless?: boolean
  requireBaseline?: boolean
}>

export const defaultKhalaCodeVisualBaselineDir = (): string =>
  resolve(import.meta.dir, "../visual-baselines")

export const defaultKhalaCodeVisualBaselineOptions =
  (): KhalaCodeVisualBaselineOptions => ({
    baselineDir: defaultKhalaCodeVisualBaselineDir(),
  })

export const khalaCodeVisualBaselineOptionsFromArgs = (
  args: ReadonlyArray<string>,
  env: Readonly<Record<string, string | undefined>> = process.env,
): KhalaCodeVisualBaselineOptions => ({
  baselineDir: resolve(
    argValue(args, "--baseline-dir") ??
    env.KHALA_CODE_VISUAL_BASELINE_DIR ??
    defaultKhalaCodeVisualBaselineDir(),
  ),
  bless:
    flag(args, "--bless-baselines") ||
    env.KHALA_CODE_VISUAL_BASELINE_BLESS === "1",
  requireBaseline:
    flag(args, "--require-baselines") ||
    env.KHALA_CODE_VISUAL_BASELINE_REQUIRE === "1",
})

const argValue = (
  args: ReadonlyArray<string>,
  name: string,
): string | undefined => {
  const index = args.indexOf(name)
  if (index === -1) return undefined
  return args[index + 1]
}

const flag = (args: ReadonlyArray<string>, name: string): boolean =>
  args.includes(name)
