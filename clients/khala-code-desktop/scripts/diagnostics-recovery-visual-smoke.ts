#!/usr/bin/env bun
/**
 * Diagnostics recovery visual smoke (issue #8441 acceptance criterion:
 * "Load failure and unresponsive recovery states are visible in a visual
 * smoke").
 *
 * The real triggers for these states are native-only (a dom-ready timeout
 * and the renderer-heartbeat watchdog in src/bun/index.ts), so this smoke
 * drives the mounted recovery overlay directly through the test-only hook
 * `window.__khalaCodeRecoveryOverlayTestHook` installed in src/ui/main.ts —
 * the same `.show()`/`.hide()` calls a real push message would trigger, just
 * invoked without a real native watchdog/window in the loop. It asserts the
 * overlay's DOM shape (role, action buttons, choice sets) for both states
 * and saves a screenshot of each as visual evidence.
 */
import { mkdir, rm, writeFile } from "node:fs/promises"
import { basename, join } from "node:path"

import { chromium, type Browser } from "playwright"
import {
  findKhalaQaAvailablePort as findAvailablePort,
  installKhalaQaConsoleErrorOracle,
  startKhalaQaViteServer as startViteServer,
  waitForKhalaQaHttp as waitForHttp,
} from "@openagentsinc/khala-qa-harness/desktop-smoke-helpers"
import {
  assertKhalaCodePagePublicSafe,
  assertKhalaCodePublicSafeValue,
} from "./public-safety-oracle"
import { installKhalaCodeVisualSmokeRpcMocks } from "./visual-smoke-rpc-mocks"

export const DIAGNOSTICS_RECOVERY_VISUAL_SMOKE_HARNESS =
  "khala_code_diagnostics_recovery_visual_smoke"

type RecoveryKindCase = Readonly<{
  detail: string
  expectedActions: readonly string[]
  kind: "load_failure" | "unresponsive"
}>

const recoveryKindCases: readonly RecoveryKindCase[] = [
  {
    detail: "Khala Code's window failed to finish loading.",
    expectedActions: ["relaunch", "export_logs", "quit"],
    kind: "load_failure",
  },
  {
    detail: "Khala Code hasn't responded recently.",
    expectedActions: ["relaunch", "export_logs", "keep_waiting", "quit"],
    kind: "unresponsive",
  },
]

export type DiagnosticsRecoveryVisualCaptureResult = Readonly<{
  actionButtons: readonly string[]
  kind: RecoveryKindCase["kind"]
  role: string | null
  screenshot: string
}>

const khalaPreviewFallbackPorts = (preferredPort: number): ReadonlyArray<number> =>
  Array.from({ length: 10 }, (_, index) => 50021 + index)
    .filter(port => port !== preferredPort)

export async function runDiagnosticsRecoveryVisualSmoke(
  options: Readonly<{ outDir: string }>,
): Promise<readonly DiagnosticsRecoveryVisualCaptureResult[]> {
  await rm(options.outDir, { force: true, recursive: true })
  await mkdir(options.outDir, { recursive: true })

  const repoRoot = new URL("../../..", import.meta.url).pathname
  const port = await findAvailablePort(50028, khalaPreviewFallbackPorts(50028))
  const server = startViteServer({
    cwd: join(repoRoot, "clients/khala-code-desktop"),
    label: "khala-code-desktop-diagnostics-recovery",
    port,
  })
  let browser: Browser | null = null
  try {
    await waitForHttp(`http://127.0.0.1:${port}/`)
    browser = await chromium.launch({ headless: true })
    const page = await browser.newPage({
      colorScheme: "dark",
      reducedMotion: "no-preference",
      viewport: { height: 900, width: 1280 },
    })
    const consoleOracle = installKhalaQaConsoleErrorOracle(page, {
      label: DIAGNOSTICS_RECOVERY_VISUAL_SMOKE_HARNESS,
    })
    const results: DiagnosticsRecoveryVisualCaptureResult[] = []
    try {
      await installKhalaCodeVisualSmokeRpcMocks(page)
      await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: "domcontentloaded" })
      await page.waitForFunction(
        () => (globalThis as unknown as {
          __khalaCodeRecoveryOverlayTestHook?: unknown
        }).__khalaCodeRecoveryOverlayTestHook !== undefined,
      )

      // Baseline: nothing shown before any state is forced.
      const hiddenBefore = await page.locator("[data-khala-code-recovery-overlay]").count()
      if (hiddenBefore !== 0) {
        throw new Error("Recovery overlay rendered before any state was forced.")
      }

      for (const testCase of recoveryKindCases) {
        await page.evaluate(({ detail, kind }) => {
          const hook = (globalThis as unknown as {
            __khalaCodeRecoveryOverlayTestHook: {
              show: (state: { detail: string; kind: string; since: string }) => void
            }
          }).__khalaCodeRecoveryOverlayTestHook
          hook.show({ detail, kind, since: new Date().toISOString() })
        }, { detail: testCase.detail, kind: testCase.kind })

        const overlay = page.locator("[data-khala-code-recovery-overlay]")
        await overlay.waitFor({ state: "visible" })
        const observedKind = await overlay.getAttribute("data-khala-code-recovery-kind")
        if (observedKind !== testCase.kind) {
          throw new Error(`Expected recovery kind ${testCase.kind}, observed ${String(observedKind)}`)
        }
        const role = await page.locator("[role='alertdialog']").getAttribute("role")
        const actionButtons = await page.locator("[data-khala-code-recovery-action]").evaluateAll(
          elements => elements.map(element => element.getAttribute("data-khala-code-recovery-action") ?? ""),
        )
        if (JSON.stringify(actionButtons) !== JSON.stringify(testCase.expectedActions)) {
          throw new Error(
            `Expected actions ${JSON.stringify(testCase.expectedActions)} for ${testCase.kind}, ` +
            `observed ${JSON.stringify(actionButtons)}`,
          )
        }

        const screenshotPath = join(options.outDir, `${testCase.kind}.png`)
        await page.screenshot({ path: screenshotPath })
        await assertKhalaCodePagePublicSafe(page, `diagnostics recovery overlay (${testCase.kind})`)

        results.push({
          actionButtons,
          kind: testCase.kind,
          role,
          screenshot: basename(screenshotPath),
        })

        await page.evaluate(() => {
          const hook = (globalThis as unknown as {
            __khalaCodeRecoveryOverlayTestHook: { hide: () => void }
          }).__khalaCodeRecoveryOverlayTestHook
          hook.hide()
        })
        await overlay.waitFor({ state: "detached" })
      }

      consoleOracle.assertNoUnexpected()
    } catch (error) {
      consoleOracle.assertNoUnexpected()
      throw error
    } finally {
      await page.close()
    }

    const summary = { harness: DIAGNOSTICS_RECOVERY_VISUAL_SMOKE_HARNESS, results }
    assertKhalaCodePublicSafeValue(summary, "Diagnostics recovery visual smoke summary")
    await writeFile(join(options.outDir, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`)
    return results
  } finally {
    if (browser !== null) await browser.close()
    server.kill()
  }
}

if (import.meta.main) {
  const outDir = new URL("../var/diagnostics-recovery-visual-smoke", import.meta.url).pathname
  const results = await runDiagnosticsRecoveryVisualSmoke({ outDir })
  for (const result of results) {
    console.log(
      `[diagnostics-recovery-visual-smoke] ${result.kind}: role=${result.role} ` +
      `actions=${result.actionButtons.join(",")} screenshot=${result.screenshot}`,
    )
  }
  console.log(`[diagnostics-recovery-visual-smoke] PASS (${results.length} states captured)`)
}
