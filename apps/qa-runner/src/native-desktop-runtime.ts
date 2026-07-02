// Native-desktop runtime seam for the native-desktop execution backend.
//
// The native-desktop backend (native-desktop-backend.ts) drives a REAL desktop
// app: focus it, read its OS accessibility (AX) tree, click/type, and capture
// screenshots — the desktop-surface sibling of the browser runner (runner.ts)
// and the terminal runner (terminal-backend.ts). The actual native automation
// engine is abstracted behind `NativeDesktopRuntime` so that:
//   - the real macOS path shells out to native, dependency-free OS tools
//     (`osascript` System Events for the AX tree + click/type, `screencapture`
//     for screenshots, optionally `cliclick` for synthesized clicks), and
//   - unit tests inject a deterministic fake (no real desktop, no Accessibility
//     permission, no network) and still prove the full focus -> read-AX ->
//     click/type -> screenshot -> teardown contract.
//
// HONESTY RULES (mirroring container-runtime.ts):
//   - `macosNativeDesktopRuntime().available()` reports the TRUE state of the
//     host: are the native helper binaries on PATH AND has the controlling
//     process been granted macOS Accessibility permission. The backend refuses
//     to run when the helper/permission is absent; it NEVER fakes a green.
//   - every method that shells out throws an explicit, typed error on failure;
//     a non-zero helper exit is surfaced, not swallowed.
//
// WHY OSASCRIPT/SCREENCAPTURE/CLICLICK (the macOS driver choice):
//   trycua/cua (`cua-driver`, the backend droid-control uses) is the eventual
//   cross-OS option, but it is a Python/agent stack that must be installed and,
//   for the local host path, would STILL require the same macOS Accessibility
//   permission to read the AX tree. For a dependency-free, locally-runnable
//   macOS-first driver we shell out to the OS's own tools instead:
//     - `osascript` + AppleScript "System Events" reads the live AX tree and
//       synthesizes AX clicks / keystrokes through the SAME Accessibility API
//       cua uses under the hood (it is gated by the same TCC permission);
//     - `screencapture` (always present on macOS) writes a PNG of the screen;
//     - `cliclick` (optional, Homebrew) synthesizes pointer clicks when an AX
//       click target is not addressable.
//   This keeps the seam injectable, so a trycua/cua adapter can replace
//   `macosNativeDesktopRuntime` later without touching the backend. The choice +
//   its Accessibility-permission requirement is documented in
//   docs/qa-runner/native-desktop-backend.md.
//
// REQUIREMENT (owner-grantable, not code-fixable): the process driving these
// tools must hold macOS Accessibility permission
// (System Settings -> Privacy & Security -> Accessibility). Without it, System
// Events AX reads/synthesized input fail; `available()` returns false and the
// backend refuses honestly.

import { spawn } from "node:child_process";

export type NativeDesktopOs = "macos" | "windows";

/** A serializable, public-safe accessibility node. */
export interface AxNode {
  /** AX role (e.g. "AXButton", "AXWindow", "AXStaticText"). */
  readonly role: string;
  /** AX title / name when present (public-safe label text only). */
  readonly title?: string;
  /** AX value when present + safe to expose (e.g. a label; never a secret). */
  readonly value?: string;
  /** Child nodes (bounded depth). */
  readonly children?: ReadonlyArray<AxNode>;
}

/** A snapshot of an app's accessibility tree at a moment in time. */
export interface AxTreeSnapshot {
  /** The application name the tree was read from. */
  readonly app: string;
  /** The root nodes (typically the app's windows). */
  readonly nodes: ReadonlyArray<AxNode>;
}

/** Options when launching/focusing an app. */
export interface NativeAppTarget {
  /** Application name as the OS knows it (e.g. "TextEdit", "Finder"). */
  readonly app: string;
  /** Optional OS process id when multiple app instances share the same name. */
  readonly pid?: number;
}

/**
 * The native-desktop automation seam. A real implementation shells out to the
 * OS's native tools; the fake in tests is fully deterministic. The shape is
 * deliberately small — just what the backend needs to focus an app, read its AX
 * tree, synthesize input, and screenshot.
 */
export interface NativeDesktopRuntime {
  /** Engine name, surfaced in errors + the result backend label detail. */
  readonly name: string;
  /** The OS tier this runtime drives. */
  readonly os: NativeDesktopOs;
  /**
   * True only when the runtime is actually usable on this host: helper binaries
   * present AND (on macOS) Accessibility permission granted. Honest: a false
   * here makes the backend refuse, never fake. Async because the permission
   * probe needs to actually attempt an AX read.
   */
  readonly available: () => Promise<boolean>;
  /** Launch (if needed) and bring `target.app` to the foreground. */
  readonly focus: (target: NativeAppTarget) => Promise<void>;
  /** Snapshot the accessibility tree of `target.app` (bounded depth). */
  readonly accessibilityTree: (target: NativeAppTarget) => Promise<AxTreeSnapshot>;
  /**
   * Click an AX node of `target.app` addressed by a role+name intent
   * (e.g. `AXButton:OK`) or a synthesized pointer fallback (`point:x,y`).
   * Throws if the target cannot be addressed (never silently no-ops).
   */
  readonly click: (target: NativeAppTarget, selector: string) => Promise<void>;
  /** Type text into the focused element of `target.app`. */
  readonly type: (target: NativeAppTarget, text: string) => Promise<void>;
  /** Capture a screenshot to `path` (PNG). Returns the written path. */
  readonly screenshot: (target: NativeAppTarget, path: string) => Promise<string>;
  /** Release any per-app resources (best-effort). */
  readonly teardown: (target: NativeAppTarget) => Promise<void>;
}

export class NativeDesktopRuntimeError extends Error {
  constructor(
    message: string,
    readonly detail?: { readonly code?: number; readonly output?: string },
  ) {
    super(message);
    this.name = "NativeDesktopRuntimeError";
  }
}

/** Run a helper CLI, capturing combined output. Never shell-interpolates. */
function execHelper(
  bin: string,
  args: ReadonlyArray<string>,
): Promise<{ readonly code: number; readonly output: string }> {
  return new Promise((resolve, reject) => {
    let child: ReturnType<typeof spawn>;
    try {
      child = spawn(bin, [...args], { stdio: ["ignore", "pipe", "pipe"] });
    } catch (error) {
      reject(
        new NativeDesktopRuntimeError(
          `failed to spawn "${bin}": ${error instanceof Error ? error.message : String(error)}`,
        ),
      );
      return;
    }
    let output = "";
    child.stdout?.on("data", (c) => {
      output += c.toString();
    });
    child.stderr?.on("data", (c) => {
      output += c.toString();
    });
    child.on("error", (error) => {
      // ENOENT: the helper binary is not installed.
      reject(new NativeDesktopRuntimeError(`"${bin}" not available: ${error.message}`));
    });
    child.on("close", (code) => {
      resolve({ code: code ?? -1, output });
    });
  });
}

/** Run an AppleScript via `osascript -e`, returning trimmed stdout. Throws on a non-zero exit. */
async function runOsa(script: string): Promise<string> {
  const r = await execHelper("osascript", ["-e", script]);
  if (r.code !== 0) {
    throw new NativeDesktopRuntimeError(`osascript failed`, { code: r.code, output: r.output });
  }
  return r.output.trim();
}

/**
 * The bounded AppleScript that walks an app process's AX tree. System Events
 * exposes UI elements with `role`, `title`/`name`, and `value`. We keep the walk
 * SHALLOW (windows -> their direct UI elements) so a deep UI cannot make the
 * read unbounded, and we emit a compact tab/return-delimited line format that
 * `parseAxDump` turns into `AxNode`s.
 *
 * Public-safe: we read role/title/value LABELS only. macOS does not expose the
 * plaintext value of a secure-text field via System Events, and the backend's
 * tripwire (`assertPublicSafeResult`) re-checks the serialized result.
 */
const processRef = (target: NativeAppTarget): string =>
  target.pid === undefined
    ? `process ${JSON.stringify(target.app)}`
    : `(first process whose unix id is ${target.pid})`;

export function axTreeScript(target: NativeAppTarget): string {
  const proc = processRef(target);
  // Lines emitted, tab-separated: depth<TAB>role<TAB>title<TAB>value
  return [
    `set out to ""`,
    `tell application "System Events"`,
    `  if not (exists ${proc}) then return "ERR\tno-process"`,
    `  tell ${proc}`,
    `    repeat with w in windows`,
    `      set wTitle to ""`,
    `      try`,
    `        set wTitle to (title of w as string)`,
    `      end try`,
    `      set out to out & "1\tAXWindow\t" & wTitle & "\t" & return`,
    `      try`,
    `        repeat with e in (UI elements of w)`,
    `          set eRole to ""`,
    `          set eTitle to ""`,
    `          set eValue to ""`,
    `          try`,
    `            set eRole to (role of e as string)`,
    `          end try`,
    `          try`,
    `            set eTitle to (title of e as string)`,
    `          end try`,
    `          try`,
    `            set eValue to (value of e as string)`,
    `          end try`,
    `          set out to out & "2\t" & eRole & "\t" & eTitle & "\t" & eValue & return`,
    `        end repeat`,
    `      end try`,
    `    end repeat`,
    `  end tell`,
    `end tell`,
    `return out`,
  ].join("\n");
}

/** Parse the tab/return-delimited AX dump into a bounded `AxTreeSnapshot`. */
export function parseAxDump(app: string, dump: string): AxTreeSnapshot {
  const lines = dump
    .split(/\r?\n|\r/)
    .map((l) => l.replace(/\s+$/, ""))
    .filter((l) => l.length > 0);
  if (lines[0]?.startsWith("ERR")) {
    throw new NativeDesktopRuntimeError(`accessibilityTree: ${lines[0].split("\t")[1] ?? "error"}`);
  }
  // AppleScript renders an absent property as the literal string "missing
  // value"; normalize that to absent so the snapshot is clean.
  const clean = (s: string): string => (s === "missing value" ? "" : s);
  const windows: AxNode[] = [];
  let current: { role: string; title?: string; children: AxNode[] } | null = null;
  for (const line of lines) {
    const [depthStr, roleRaw = "", titleRaw = "", valueRaw = ""] = line.split("\t");
    const role = clean(roleRaw);
    const title = clean(titleRaw);
    const value = clean(valueRaw);
    if (depthStr === "1") {
      if (current) windows.push(finalizeWindow(current));
      current = { role: role || "AXWindow", children: [], ...(title ? { title } : {}) };
    } else if (depthStr === "2" && current) {
      const node: AxNode = {
        role: role || "AXUnknown",
        ...(title ? { title } : {}),
        ...(value ? { value } : {}),
      };
      current.children.push(node);
    }
  }
  if (current) windows.push(finalizeWindow(current));
  return { app, nodes: windows };
}

function finalizeWindow(w: { role: string; title?: string; children: AxNode[] }): AxNode {
  return {
    role: w.role,
    ...(w.title ? { title: w.title } : {}),
    ...(w.children.length > 0 ? { children: w.children } : {}),
  };
}

export interface MacosNativeDesktopRuntimeOptions {
  /** Override the cliclick binary (default "cliclick"; optional fallback). */
  readonly cliclickBin?: string;
}

/**
 * The REAL macOS native-desktop runtime: shells out to `osascript`
 * (System Events AX), `screencapture`, and optionally `cliclick`. `available()`
 * is honest — it returns false when a required helper is missing OR when the
 * controlling process lacks Accessibility permission (a System Events AX read
 * fails), so the backend can refuse instead of faking.
 */
export function macosNativeDesktopRuntime(
  options: MacosNativeDesktopRuntimeOptions = {},
): NativeDesktopRuntime {
  const cliclickBin = options.cliclickBin ?? "cliclick";
  return {
    name: "macos-osascript",
    os: "macos",
    available: async () => {
      try {
        // The honest probe: an actual System Events AX read. This both checks
        // that `osascript` exists AND that Accessibility permission is granted
        // (without permission, System Events raises error -1719/-25211 and the
        // command exits non-zero). `screencapture` is part of macOS.
        const r = await execHelper("osascript", [
          "-e",
          'tell application "System Events" to return (count of processes)',
        ]);
        return r.code === 0 && /^\d+$/.test(r.output.trim());
      } catch {
        return false;
      }
    },
    focus: async (target) => {
      if (target.pid !== undefined) {
        const out = await runOsa(
          [
            `tell application "System Events"`,
            `  if not (exists ${processRef(target)}) then return "ERR\tno-process"`,
            `  set frontmost of ${processRef(target)} to true`,
            `  return "OK"`,
            `end tell`,
          ].join("\n"),
        );
        if (out.startsWith("ERR")) {
          throw new NativeDesktopRuntimeError(`focus: ${out.split("\t")[1] ?? "error"}`);
        }
        return;
      }
      // `activate` launches the app if needed and brings it to the foreground.
      await runOsa(`tell application ${JSON.stringify(target.app)} to activate`);
    },
    accessibilityTree: async (target) => {
      const dump = await runOsa(axTreeScript(target));
      return parseAxDump(target.app, dump);
    },
    click: async (target, selector) => {
      // selector forms:
      //   "AXRole:Name"  -> AX press of the first matching element by role+title
      //   "point:x,y"    -> synthesized pointer click via cliclick (fallback)
      if (selector.startsWith("point:")) {
        const [x, y] = selector.slice("point:".length).split(",");
        const r = await execHelper(cliclickBin, [`c:${x},${y}`]);
        if (r.code !== 0) {
          throw new NativeDesktopRuntimeError(`cliclick click failed`, { code: r.code, output: r.output });
        }
        return;
      }
      const sep = selector.indexOf(":");
      if (sep < 0) {
        throw new NativeDesktopRuntimeError(
          `click: selector must be "AXRole:Name" or "point:x,y" (got ${JSON.stringify(selector)})`,
        );
      }
      const role = selector.slice(0, sep);
      const name = selector.slice(sep + 1);
      const proc = processRef(target);
      const script = [
        `tell application "System Events"`,
        `  tell ${proc}`,
        `    set theEl to missing value`,
        `    repeat with w in windows`,
        `      try`,
        `        set theEl to (first UI element of w whose role is ${JSON.stringify(role)} and (title is ${JSON.stringify(name)} or name is ${JSON.stringify(name)}))`,
        `      end try`,
        `      if theEl is not missing value then exit repeat`,
        `    end repeat`,
        `    if theEl is missing value then return "ERR\tnot-found"`,
        `    perform action "AXPress" of theEl`,
        `    return "OK"`,
        `  end tell`,
        `end tell`,
      ].join("\n");
      const out = await runOsa(script);
      if (out.startsWith("ERR")) {
        throw new NativeDesktopRuntimeError(`click: AX element not found for selector ${JSON.stringify(selector)}`);
      }
    },
    type: async (target, text) => {
      // keystroke into the focused element of the (re-activated) app.
      const script =
        target.pid === undefined
          ? [
              `tell application ${JSON.stringify(target.app)} to activate`,
              `tell application "System Events" to keystroke ${JSON.stringify(text)}`,
            ].join("\n")
          : [
              `tell application "System Events"`,
              `  if not (exists ${processRef(target)}) then return "ERR\tno-process"`,
              `  set frontmost of ${processRef(target)} to true`,
              `  keystroke ${JSON.stringify(text)}`,
              `  return "OK"`,
              `end tell`,
            ].join("\n");
      const out = await runOsa(script);
      if (out.startsWith("ERR")) {
        throw new NativeDesktopRuntimeError(`type: ${out.split("\t")[1] ?? "error"}`);
      }
    },
    screenshot: async (_target, path) => {
      // `screencapture -x` = silent, `-o` = omit window shadow. We capture the
      // whole screen (always succeeds) as the honest "what the desktop showed"
      // artifact rather than guessing an unreliable per-app window id.
      const r = await execHelper("screencapture", ["-x", "-o", path]);
      if (r.code !== 0) {
        throw new NativeDesktopRuntimeError(`screencapture failed`, { code: r.code, output: r.output });
      }
      return path;
    },
    teardown: async () => {
      // No persistent per-app resource to release for the osascript path.
      return undefined;
    },
  };
}

/**
 * Windows tier — SPEC ONLY. The contract is identical to the macOS runtime, but
 * the engine (UI Automation via PowerShell / a trycua/cua adapter) is not
 * implemented here. `available()` is honestly false and every action throws,
 * so a Windows host refuses rather than faking. A second pass implements this.
 */
export function windowsNativeDesktopRuntime(): NativeDesktopRuntime {
  const fail = (): never => {
    throw new NativeDesktopRuntimeError(
      "windows native-desktop runtime is spec-only: the Windows UI Automation " +
        "engine (PowerShell UIA / trycua-cua adapter) is not implemented yet. " +
        "macOS is the implemented tier; Windows lands in a second pass.",
    );
  };
  return {
    name: "windows-uia (spec-only)",
    os: "windows",
    available: async () => false,
    focus: async () => fail(),
    accessibilityTree: async () => fail(),
    click: async () => fail(),
    type: async () => fail(),
    screenshot: async () => fail(),
    teardown: async () => undefined,
  };
}
