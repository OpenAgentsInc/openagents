// Single-instance coordination for Khala Code desktop (#8442). A launched
// process first tries to CONNECT to a fixed local Unix domain socket. If that
// succeeds, another instance is already running (the socket only exists while
// something is listening on it), so this launch forwards its payload (any
// deep-link URL it was opened with) and becomes a "secondary" -- the caller
// should then exit without opening a window. If the connect fails, this
// launch removes any stale socket file left behind by a crashed previous
// instance and binds the socket itself, becoming the "primary" that stays
// alive for the lifetime of the app and receives forwarded payloads from any
// later secondary launches (used to focus the existing window and route a
// warm deep link).
//
// This uses a real Unix domain socket (no mocked transport) so tests exercise
// actual connect/listen/forward behavior against temporary socket paths.

import { unlink } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"

export const KHALA_CODE_DESKTOP_SINGLE_INSTANCE_ENV = "KHALA_CODE_DESKTOP_SINGLE_INSTANCE"
export const KHALA_CODE_DESKTOP_SINGLE_INSTANCE_SOCKET_ENV =
  "KHALA_CODE_DESKTOP_SINGLE_INSTANCE_SOCKET"

type SingleInstanceEnv = Readonly<Record<string, string | undefined>>

export const khalaCodeDesktopSingleInstanceEnabled = (env: SingleInstanceEnv): boolean => {
  const value = env[KHALA_CODE_DESKTOP_SINGLE_INSTANCE_ENV]?.trim().toLowerCase()
  return value !== "0" && value !== "false" && value !== "off"
}

export const resolveKhalaCodeDesktopSingleInstanceSocketPath = (
  env: SingleInstanceEnv,
): string =>
  env[KHALA_CODE_DESKTOP_SINGLE_INSTANCE_SOCKET_ENV]?.trim() ||
  join(env.HOME?.trim() || homedir(), ".khala-code", "desktop-single-instance.sock")

export type KhalaCodeSingleInstanceResult =
  | Readonly<{ role: "primary"; close: () => void }>
  | Readonly<{ role: "secondary"; forwarded: boolean }>

export type KhalaCodeSingleInstanceOptions = Readonly<{
  socketPath: string
  /** Payload to send to an already-running primary instance, if any (e.g. a
   * deep-link URL this launch was opened with). Omit for a plain focus-only
   * relaunch with nothing to forward. */
  forwardPayload?: string
  /** Invoked on the primary instance for every payload line a later
   * secondary instance forwards to it. */
  onIncomingPayload: (payload: string) => void
  /** Invoked if this process could neither reach an existing primary nor
   * bind the socket itself (e.g. a permissions problem, or a narrow race
   * where the previous primary's socket file disappeared mid-handoff). */
  onListenError?: (error: unknown) => void
}>

// AF_UNIX socket paths are kernel-limited to a small fixed `sun_path` buffer
// -- 104 bytes on macOS/BSD, 108 on Linux. A path at or past that limit does
// not fail cleanly: it can connect to (or bind) whatever the OS silently
// truncates the path down to, which risks talking to an unrelated socket
// instead of just failing loudly. `~/.khala-code/desktop-single-instance.sock`
// is always short in practice, but stay well clear of the limit rather than
// depend on that.
const MAX_SAFE_UNIX_SOCKET_PATH_BYTES = 100

/**
 * Acquires (or defers to) the single-instance lock for Khala Code desktop.
 * Never throws -- every failure path resolves to a typed result instead.
 */
export const acquireKhalaCodeDesktopSingleInstanceLock = async (
  options: KhalaCodeSingleInstanceOptions,
): Promise<KhalaCodeSingleInstanceResult> => {
  const { socketPath } = options

  if (Buffer.byteLength(socketPath, "utf8") > MAX_SAFE_UNIX_SOCKET_PATH_BYTES) {
    options.onListenError?.(
      new Error(
        `Single-instance socket path is too long for a safe AF_UNIX bind (${
          Buffer.byteLength(socketPath, "utf8")
        } bytes, max ${MAX_SAFE_UNIX_SOCKET_PATH_BYTES}): ${socketPath}`,
      ),
    )
    return { forwarded: false, role: "secondary" }
  }

  const tryForwardToExistingPrimary = (): Promise<boolean> =>
    new Promise(resolve => {
      let settled = false
      const settle = (value: boolean): void => {
        if (settled) return
        settled = true
        resolve(value)
      }
      Bun.connect({
        socket: {
          // A `close` with no prior `open` means the connection attempt
          // itself failed (e.g. nothing listening yet) -- that is a failed
          // forward, not a successful one. Only `open` (we reached a real
          // listener) settles this true.
          close: () => settle(false),
          data: () => undefined,
          error: () => settle(false),
          open: socket => {
            // Settle true the instant we have a real connection, BEFORE
            // calling `.end()` -- ending the socket can synchronously
            // re-enter the `close` callback above for a fast local unix
            // socket, and that must not be allowed to win the race and
            // flip an already-successful forward back to false.
            settle(true)
            try {
              if (options.forwardPayload !== undefined) {
                socket.write(`${options.forwardPayload}\n`)
              }
            } finally {
              socket.end()
            }
          },
        },
        unix: socketPath,
      }).catch(() => settle(false))
    })

  if (await tryForwardToExistingPrimary()) {
    return { forwarded: true, role: "secondary" }
  }

  // No primary is listening (or the socket file is stale from a crashed
  // previous run). Clear any stale file, then try to become the primary.
  try {
    await unlink(socketPath)
  } catch {
    // Nothing to remove, or a permissions issue that the listen below will
    // surface via onListenError.
  }

  try {
    const server = Bun.listen({
      socket: {
        close: () => undefined,
        data: (_socket, data) => {
          const text = Buffer.from(data).toString("utf8")
          for (const line of text.split("\n")) {
            const trimmed = line.trim()
            if (trimmed.length > 0) options.onIncomingPayload(trimmed)
          }
        },
        error: () => undefined,
        open: () => undefined,
      },
      unix: socketPath,
    })
    return {
      close: () => {
        try {
          server.stop(true)
        } catch {
          // Already stopped.
        }
      },
      role: "primary",
    }
  } catch (error) {
    options.onListenError?.(error)
    // Could not forward AND could not bind (a narrow race, or a permissions
    // problem writing under the state directory). Report as an unforwarded
    // secondary rather than silently pretending to be primary with no
    // listener -- the caller decides whether to still open a window.
    return { forwarded: false, role: "secondary" }
  }
}
