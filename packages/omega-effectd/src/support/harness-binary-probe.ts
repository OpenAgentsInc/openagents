/**
 * Seven-agents Part 2 (#9183): shared, SAFE binary detection for the host-run
 * SDK-harness lanes (Goose, OpenCode, Pi). This is a READ-ONLY probe:
 *
 *  - It walks the caller-supplied PATH (never `process.env.PATH` mutation) and
 *    checks each candidate with `access(_, X_OK)`.
 *  - It NEVER runs an install command, NEVER edits PATH, and NEVER touches the
 *    owner's provider config/auth. A missing binary is reported honestly; the
 *    provider card (not this module) guides the owner to the official
 *    distribution — the same no-copied-installs guardrail the ACP provider
 *    probes (`probeGrokAcpExecutable`) already honor.
 *  - It confirms the executable actually runs by invoking its `--version`-style
 *    argument with a short timeout and a bounded output buffer.
 *
 * A lane reports `available` only when this probe returns `detected`, so the
 * boot roster never shows a dead card (the honest-readiness invariant).
 */

import { execFile } from "node:child_process"
import { constants } from "node:fs"
import { access, realpath } from "node:fs/promises"
import { delimiter, isAbsolute, resolve } from "node:path"
import { promisify } from "node:util"

const execFileAsync = promisify(execFile)

export type HarnessBinaryProbe =
  | Readonly<{
      state: "detected"
      /** The PATH-resolved (or explicitly supplied) executable path. */
      resolvedPath: string
      /** The canonical real path after symlink resolution. */
      realPath: string
      /** The bounded version string the executable reported. */
      reportedVersion: string
    }>
  | Readonly<{
      state: "not_detected"
      /** A public-safe reason (no secrets, no owner paths beyond the binary name). */
      reason: string
    }>

/**
 * Walk the supplied PATH list (read-only) for `executable`, returning the first
 * candidate that exists and is executable. Never mutates PATH.
 */
export const findExecutableOnPath = async (
  executable: string,
  pathValue: string | undefined,
): Promise<string | null> => {
  if (isAbsolute(executable)) {
    try {
      await access(executable, constants.X_OK)
      return executable
    } catch {
      return null
    }
  }
  for (const directory of (pathValue ?? "").split(delimiter)) {
    if (directory.trim() === "") continue
    const candidate = resolve(directory, executable)
    try {
      await access(candidate, constants.X_OK)
      return candidate
    } catch {
      // Continue through the bounded PATH list.
    }
  }
  return null
}

/**
 * Probe a coding-agent CLI by detecting it on PATH and confirming it runs.
 * `versionArgs` is a read-only invocation (`["--version"]`, `["version"]`, …)
 * that must print something; a binary that is present but cannot execute is
 * reported `not_detected` with an honest reason.
 */
export const probeHarnessBinary = async (input: {
  readonly executable: string
  readonly displayName: string
  readonly versionArgs: ReadonlyArray<string>
  readonly environment?: Readonly<Record<string, string | undefined>>
  readonly candidatePath?: string
  readonly timeoutMs?: number
}): Promise<HarnessBinaryProbe> => {
  const environment = input.environment ?? process.env
  const timeoutMs = input.timeoutMs ?? 5_000
  try {
    const resolvedPath =
      input.candidatePath ?? (await findExecutableOnPath(input.executable, environment.PATH))
    if (resolvedPath === null) {
      return {
        state: "not_detected",
        reason: `${input.displayName} is not installed or not on PATH.`,
      }
    }
    const realPath = await realpath(resolvedPath)
    const { stdout, stderr } = await execFileAsync(realPath, [...input.versionArgs], {
      timeout: timeoutMs,
      maxBuffer: 64 * 1024,
      env: { ...environment } as NodeJS.ProcessEnv,
    })
    const reportedVersion = `${stdout}${stderr}`.trim().slice(0, 256)
    if (reportedVersion.length === 0) {
      return {
        state: "not_detected",
        reason: `${input.displayName} did not report a version.`,
      }
    }
    return { state: "detected", resolvedPath, realPath, reportedVersion }
  } catch {
    return {
      state: "not_detected",
      reason: `${input.displayName} is not installed or its executable probe failed.`,
    }
  }
}
