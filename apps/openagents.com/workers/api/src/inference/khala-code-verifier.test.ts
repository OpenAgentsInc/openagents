import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, test } from 'vitest'

import type { AcceptanceVerdict } from './acceptance-runner/verdict'
import { crossyRoadAcceptanceSpec } from './acceptance-spec'
import { assembleAcceptanceVerdict } from './acceptance-runner/verdict'
import {
  KHALA_CODE_CROSSY_ROAD_RUBRIC_REF,
  KHALA_CODE_HEADLESS_COMMAND_REF,
  discoverKhalaCodeVerificationCommand,
  extractSingleHtmlArtifact,
  prescreenKhalaCodeArtifact,
  verifyKhalaCodeCompletion,
} from './khala-code-verifier'
import {
  BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML,
  GOOD_CROSSY_ROAD_HTML,
} from './khala-code-verifier.fixtures'

const verify = (
  content: string,
  acceptance?: AcceptanceVerdict | undefined,
) =>
  verifyKhalaCodeCompletion({
    content,
    meteringReceiptRef: 'receipt.inference.charge.chatcmpl-fixture',
    requestId: 'chatcmpl-fixture',
    servedModel: 'openagents/khala-code',
    worker: 'fireworks',
    ...(acceptance === undefined ? {} : { acceptance }),
  })

const executedVerdict = (allPass: boolean): AcceptanceVerdict => {
  const spec = crossyRoadAcceptanceSpec()
  return assembleAcceptanceVerdict({
    checks: spec.checks.map((id, index) => ({
      detail: 'x',
      id,
      passed: allPass ? true : index === 0,
    })),
    consoleErrors: [],
    pageErrors: [],
    spec,
  })
}

describe('Khala code verifier — honest downgrade (EPIC #6017)', () => {
  test('extracts a single HTML artifact from raw or fenced assistant content', () => {
    expect(extractSingleHtmlArtifact(GOOD_CROSSY_ROAD_HTML)).toBe(
      GOOD_CROSSY_ROAD_HTML,
    )
    expect(
      extractSingleHtmlArtifact(
        `Here is the file:\n\n\`\`\`html\n${GOOD_CROSSY_ROAD_HTML}\n\`\`\``,
      ),
    ).toBe(GOOD_CROSSY_ROAD_HTML)
  })

  test('declares a reusable headless verification command contract', () => {
    expect(discoverKhalaCodeVerificationCommand()).toEqual({
      commandRef: KHALA_CODE_HEADLESS_COMMAND_REF,
      kind: 'headless_html_probe',
      rubricRef: KHALA_CODE_CROSSY_ROAD_RUBRIC_REF,
      target: 'crossy-road-single-html',
    })
  })

  test('prescreen is a gate-to-attempt, not a verdict', () => {
    const pass = prescreenKhalaCodeArtifact(GOOD_CROSSY_ROAD_HTML)
    expect(pass.attemptExecution).toBe(true)
    const fail = prescreenKhalaCodeArtifact(
      BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML,
    )
    expect(fail.attemptExecution).toBe(false)
    expect(fail.checks.find(c => c.id === 'single_html_file')?.passed).toBe(
      false,
    )
  })

  test('a prescreen-passing artifact that was NOT executed is unverified — NOT test_passed', () => {
    const verdict = verify(GOOD_CROSSY_ROAD_HTML)

    // This is the core of the honest downgrade: looks fine on paper, but we did not
    // run it, so we do NOT certify it.
    expect(verdict.verification).toBe('unverified')
    expect(verdict.verified).toBe(false)
    expect(verdict.executed).toBe(false)
    expect(verdict.scalarReward).toBe(0)
    expect(verdict.reward.scalar).toBe(0)
    expect(verdict.prescreen.attemptExecution).toBe(true)
  })

  test('an artifact that fails the prescreen is failed and not executed', () => {
    const verdict = verify(BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML)
    expect(verdict.verification).toBe('failed')
    expect(verdict.verified).toBe(false)
    expect(verdict.executed).toBe(false)
    expect(verdict.scalarReward).toBe(0)
    expect(verdict.failedChecks).toContain('single_html_file')
  })

  test('verdict is test_passed only when an EXECUTED acceptance suite fully passed', () => {
    const verdict = verify(GOOD_CROSSY_ROAD_HTML, executedVerdict(true))
    expect(verdict.executed).toBe(true)
    expect(verdict.verification).toBe('test_passed')
    expect(verdict.verified).toBe(true)
    expect(verdict.scalarReward).toBe(1)
    expect(verdict.failedChecks).toEqual([])
    expect(verdict.sourceRefs).toContain(
      'receipt.inference.charge.chatcmpl-fixture',
    )
    expect(verdict.reward.handoffRef).toContain(
      'accepted_outcome.khala_code.crossy_road.',
    )
  })

  test('an executed acceptance suite that did not fully pass is failed with a dense reward', () => {
    const verdict = verify(GOOD_CROSSY_ROAD_HTML, executedVerdict(false))
    expect(verdict.executed).toBe(true)
    expect(verdict.verification).toBe('failed')
    expect(verdict.verified).toBe(false)
    expect(verdict.scalarReward).toBeGreaterThan(0)
    expect(verdict.scalarReward).toBeLessThan(1)
    expect(verdict.failedChecks.length).toBeGreaterThan(0)
  })
})

// GAP 1 (EPIC #6017): the pre-screen must ALLOW a faithful three.js game that
// loads three.js from a pinned, well-known CDN (so it reaches the authoritative
// execution verifier), while STILL rejecting arbitrary/unknown external assets.
describe('Khala code verifier — pinned-CDN library allowance', () => {
  const threeJsGame = (cdnUrl: string): string => `<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>three.js crossy road</title>
<style>body{margin:0}#game{display:block}</style>
<script src="${cdnUrl}"></script>
</head>
<body>
  <canvas id="game" width="960" height="540"></canvas>
  <script>
    const scene = new THREE.Scene();
    function loop(){ requestAnimationFrame(loop); }
    loop();
  </script>
</body>
</html>`

  test('a three.js game from each allowlisted CDN PASSES the pre-screen (reaches execution)', () => {
    const cdnUrls = [
      'https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js',
      'https://unpkg.com/three@0.160.0/build/three.min.js',
      'https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.min.js',
      'https://esm.sh/three@0.160.0',
    ]
    for (const url of cdnUrls) {
      const prescreen = prescreenKhalaCodeArtifact(threeJsGame(url))
      expect(prescreen.attemptExecution).toBe(true)
      expect(
        prescreen.checks.find(c => c.id === 'single_html_file')?.passed,
      ).toBe(true)
      expect(prescreen.allowedCdnLibraries.map(l => l.url)).toContain(url)
      expect(prescreen.allowedCdnLibraries.every(l => l.pinned)).toBe(true)
    }
  })

  test('an ES-module import of three.js from an allowlisted CDN also PASSES', () => {
    const html = `<!doctype html>
<html><head><meta charset="utf-8"><title>g</title></head>
<body>
  <canvas id="game"></canvas>
  <script type="module">
    import * as THREE from 'https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.module.js';
    const scene = new THREE.Scene();
  </script>
</body></html>`
    const prescreen = prescreenKhalaCodeArtifact(html)
    expect(prescreen.attemptExecution).toBe(true)
    expect(prescreen.allowedCdnLibraries).toHaveLength(1)
    expect(prescreen.allowedCdnLibraries[0]?.host).toBe('cdn.jsdelivr.net')
  })

  test('an arbitrary/unknown external script is STILL rejected', () => {
    const prescreen = prescreenKhalaCodeArtifact(
      threeJsGame('https://evil.example.test/three.min.js'),
    )
    expect(prescreen.attemptExecution).toBe(false)
    expect(
      prescreen.checks.find(c => c.id === 'single_html_file')?.passed,
    ).toBe(false)
    expect(prescreen.allowedCdnLibraries).toHaveLength(0)
  })

  test('an external stylesheet/image is STILL rejected even with an allowlisted script', () => {
    const html = `<!doctype html>
<html><head>
  <link rel="stylesheet" href="https://unpkg.com/some-theme/style.css">
  <script src="https://unpkg.com/three@0.160.0/build/three.min.js"></script>
</head><body><canvas id="game"></canvas></body></html>`
    expect(prescreenKhalaCodeArtifact(html).attemptExecution).toBe(false)
  })

  test('a mix of allowlisted + one unknown script host is rejected', () => {
    const html = `<!doctype html>
<html><head>
  <script src="https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.min.js"></script>
  <script src="https://random-host.example/extra.js"></script>
</head><body><canvas id="game"></canvas></body></html>`
    expect(prescreenKhalaCodeArtifact(html).attemptExecution).toBe(false)
  })

  test('the committed passing north-star artifact now PASSES the pre-screen', () => {
    const artifactPath = fileURLToPath(
      new URL(
        '../../../../../../scripts/khala-demo/artifacts/khala-crossy-road-northstar-passing.v1.html',
        import.meta.url,
      ),
    )
    const html = readFileSync(artifactPath, 'utf8')
    const prescreen = prescreenKhalaCodeArtifact(html)
    expect(prescreen.attemptExecution).toBe(true)
    expect(
      prescreen.checks.find(c => c.id === 'single_html_file')?.passed,
    ).toBe(true)
    // It pulls three.js from cdnjs (pinned to r128) — exactly the allowance.
    expect(
      prescreen.allowedCdnLibraries.some(
        l => l.host === 'cdnjs.cloudflare.com' && l.pinned,
      ),
    ).toBe(true)
  })
})
