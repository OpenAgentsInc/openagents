// Computer-use tools, exposed to Khala (`openagents/khala`) through Probe's LLM
// tool surface.
//
// Each tool is a `ProbeLlmTool` (Effect Schema input, Effect execute). Tools are
// black-box product drivers: they drive a real browser/terminal/fs, never poke
// the app's internals or DB. Destructive or outbound actions (terminal `run`,
// filesystem `write`) ride Probe's existing permission model via
// `getPermissionHandler()` — denied requests fail the tool rather than acting.

import { Effect, Schema as S } from "effect";
import { getPermissionHandler } from "../permission";
import { ProbeLlmToolFailure, defineProbeLlmTool, type ProbeLlmTools } from "../llm/tool";
import type { BrowserSurface } from "./browser";
import type { FilesystemSurface } from "./filesystem";
import type { TerminalSurface } from "./terminal";
import type { WaitForCondition } from "./page";

// ── Input schemas ───────────────────────────────────────────────────────────

const NavigateInput = S.Struct({ url: S.String });
const ClickInput = S.Struct({ selector: S.String, label: S.optional(S.String) });
const TypeInput = S.Struct({ selector: S.String, text: S.String, label: S.optional(S.String) });
const ReadInput = S.Struct({ selector: S.optional(S.String) });
const WaitForInput = S.Struct({
  kind: S.Literals(["url-includes", "text-visible", "selector-visible"]),
  value: S.optional(S.String),
  selector: S.optional(S.String),
  timeoutMs: S.optional(S.Number),
});
const ScreenshotInput = S.Struct({ label: S.String });
const RunInput = S.Struct({ command: S.String, args: S.optional(S.Array(S.String)) });
const FsReadInput = S.Struct({ path: S.String });
const FsWriteInput = S.Struct({ path: S.String, contents: S.String });

// Probe's `ProbeLlmToolDefinition.inputSchema` is a plain JSON-Schema object
// (`Record<string, unknown>`), so the public tool schema is hand-written here
// while the Effect Schemas above validate the input at execute time. Keeping the
// two coupled by hand is acceptable for this small, stable surface.
const str = { type: "string" } as const;
const obj = (
  properties: Record<string, unknown>,
  required: ReadonlyArray<string>,
): Record<string, unknown> => ({ type: "object", properties, required, additionalProperties: false });

const fail = (message: string) => new ProbeLlmToolFailure({ message });

function decodeInput<A, I>(
  schema: S.Schema<A, I>,
  input: Readonly<Record<string, unknown>>,
): Effect.Effect<A, ProbeLlmToolFailure> {
  return S.decodeUnknownEffect(schema)(input).pipe(
    Effect.mapError((error) => fail(`invalid_tool_input: ${String(error)}`)),
  );
}

/** Gate an outbound/destructive action through Probe's permission model. */
function gate(
  request: { action: "edit" | "write" | "delete"; filePath: string; diff: string },
): Effect.Effect<void, ProbeLlmToolFailure> {
  return getPermissionHandler()
    .ask(request)
    .pipe(
      Effect.flatMap((decision) =>
        decision === "deny"
          ? Effect.fail(fail(`permission_denied: ${request.action} ${request.filePath}`))
          : Effect.void,
      ),
    );
}

export interface ComputerUseSurfaces {
  readonly browser?: BrowserSurface;
  readonly terminal?: TerminalSurface;
  readonly filesystem?: FilesystemSurface;
}

/**
 * Build the computer-use tool set from whatever surfaces are wired. A surface
 * that is absent simply omits its tools (capability-as-presence). The terminal
 * and filesystem-write tools are permission-gated.
 */
export function makeComputerUseTools(surfaces: ComputerUseSurfaces): ProbeLlmTools {
  const tools: Record<string, ReturnType<typeof defineProbeLlmTool>> = {};
  const browser = surfaces.browser;
  if (browser) {
    tools.browser_navigate = defineProbeLlmTool({
      name: "browser_navigate",
      description: "Navigate the browser to a URL (relative to the target base URL or absolute).",
      inputSchema: obj({ url: str }, ["url"]),
      execute: (input) =>
        decodeInput(NavigateInput, input).pipe(
          Effect.flatMap((args) =>
            Effect.tryPromise({
              try: () => browser.navigate(args.url).then(() => browser.page.url()),
              catch: (e) => fail(`navigate_failed: ${e instanceof Error ? e.message : String(e)}`),
            }),
          ),
          Effect.map((url) => ({ url })),
        ),
    });
    tools.browser_click = defineProbeLlmTool({
      name: "browser_click",
      description: "Click an element by role-or-CSS selector.",
      inputSchema: obj({ selector: str, label: str }, ["selector"]),
      execute: (input) =>
        decodeInput(ClickInput, input).pipe(
          Effect.flatMap((args) =>
            Effect.tryPromise({
              try: () => browser.click(args.selector, args.label),
              catch: (e) => fail(`click_failed: ${e instanceof Error ? e.message : String(e)}`),
            }),
          ),
          Effect.as({ ok: true }),
        ),
    });
    tools.browser_type = defineProbeLlmTool({
      name: "browser_type",
      description: "Type text into an element by selector.",
      inputSchema: obj({ selector: str, text: str, label: str }, ["selector", "text"]),
      execute: (input) =>
        decodeInput(TypeInput, input).pipe(
          Effect.flatMap((args) =>
            Effect.tryPromise({
              try: () => browser.type(args.selector, args.text, args.label),
              catch: (e) => fail(`type_failed: ${e instanceof Error ? e.message : String(e)}`),
            }),
          ),
          Effect.as({ ok: true }),
        ),
    });
    tools.browser_read_text = defineProbeLlmTool({
      name: "browser_read_text",
      description: "Read visible text content, optionally scoped to a selector.",
      inputSchema: obj({ selector: str }, []),
      execute: (input) =>
        decodeInput(ReadInput, input).pipe(
          Effect.flatMap((args) =>
            Effect.tryPromise({
              try: () => browser.readText(args.selector),
              catch: (e) => fail(`read_text_failed: ${e instanceof Error ? e.message : String(e)}`),
            }),
          ),
          Effect.map((text) => ({ text })),
        ),
    });
    tools.browser_read_dom = defineProbeLlmTool({
      name: "browser_read_dom",
      description: "Read the DOM HTML, optionally scoped to a selector.",
      inputSchema: obj({ selector: str }, []),
      execute: (input) =>
        decodeInput(ReadInput, input).pipe(
          Effect.flatMap((args) =>
            Effect.tryPromise({
              try: () => browser.readDom(args.selector),
              catch: (e) => fail(`read_dom_failed: ${e instanceof Error ? e.message : String(e)}`),
            }),
          ),
          Effect.map((html) => ({ html })),
        ),
    });
    tools.browser_wait_for = defineProbeLlmTool({
      name: "browser_wait_for",
      description:
        "Wait until a condition holds (url-includes / text-visible / selector-visible). Never sleeps.",
      inputSchema: obj({ kind: { type: "string", enum: ["url-includes", "text-visible", "selector-visible"] }, value: str, selector: str, timeoutMs: { type: "number" } }, ["kind"]),
      execute: (input) =>
        decodeInput(WaitForInput, input).pipe(
          Effect.flatMap((args) => {
            let condition: WaitForCondition;
            if (args.kind === "selector-visible") {
              if (!args.selector) return Effect.fail(fail("wait_for_requires_selector"));
              condition = { kind: "selector-visible", selector: args.selector };
            } else {
              if (!args.value) return Effect.fail(fail("wait_for_requires_value"));
              condition =
                args.kind === "url-includes"
                  ? { kind: "url-includes", value: args.value }
                  : { kind: "text-visible", value: args.value };
            }
            return Effect.tryPromise({
              try: () =>
                browser.waitFor(condition, args.timeoutMs ? { timeoutMs: args.timeoutMs } : undefined),
              catch: (e) => fail(`wait_for_failed: ${e instanceof Error ? e.message : String(e)}`),
            });
          }),
          Effect.map((met) => ({ met })),
        ),
    });
    tools.browser_screenshot = defineProbeLlmTool({
      name: "browser_screenshot",
      description: "Capture a screenshot of the current page.",
      inputSchema: obj({ label: str }, ["label"]),
      execute: (input) =>
        decodeInput(ScreenshotInput, input).pipe(
          Effect.flatMap((args) =>
            Effect.tryPromise({
              try: () => browser.screenshot(args.label),
              catch: (e) => fail(`screenshot_failed: ${e instanceof Error ? e.message : String(e)}`),
            }),
          ),
          Effect.map((path) => ({ path })),
        ),
    });
  }

  const terminal = surfaces.terminal;
  if (terminal) {
    tools.terminal_run = defineProbeLlmTool({
      name: "terminal_run",
      description:
        "Run a shell command in the run workspace. Permission-gated (outbound/destructive).",
      inputSchema: obj({ command: str, args: { type: "array", items: str } }, ["command"]),
      execute: (input) =>
        decodeInput(RunInput, input).pipe(
          Effect.flatMap((args) =>
            gate({ action: "write", filePath: "<terminal>", diff: args.command }).pipe(
              Effect.flatMap(() =>
                Effect.tryPromise({
                  try: () => terminal.run(args.command, args.args),
                  catch: (e) => fail(`terminal_run_failed: ${e instanceof Error ? e.message : String(e)}`),
                }),
              ),
            ),
          ),
          Effect.map((result) => ({ exitCode: result.code, output: result.output })),
        ),
    });
  }

  const filesystem = surfaces.filesystem;
  if (filesystem) {
    tools.fs_read = defineProbeLlmTool({
      name: "fs_read",
      description: "Read a file relative to the run workspace (scoped — no escape).",
      inputSchema: obj({ path: str }, ["path"]),
      execute: (input) =>
        decodeInput(FsReadInput, input).pipe(
          Effect.flatMap((args) =>
            Effect.try({
              try: () => filesystem.read(args.path),
              catch: (e) => fail(`fs_read_failed: ${e instanceof Error ? e.message : String(e)}`),
            }),
          ),
          Effect.map((contents) => ({ contents })),
        ),
    });
    tools.fs_write = defineProbeLlmTool({
      name: "fs_write",
      description: "Write a file relative to the run workspace (scoped). Permission-gated.",
      inputSchema: obj({ path: str, contents: str }, ["path", "contents"]),
      execute: (input) =>
        decodeInput(FsWriteInput, input).pipe(
          Effect.flatMap((args) =>
            gate({ action: "write", filePath: args.path, diff: `${args.contents.length} bytes` }).pipe(
              Effect.flatMap(() =>
                Effect.try({
                  try: () => filesystem.write(args.path, args.contents),
                  catch: (e) => fail(`fs_write_failed: ${e instanceof Error ? e.message : String(e)}`),
                }),
              ),
            ),
          ),
          Effect.as({ ok: true }),
        ),
    });
  }

  return tools;
}
