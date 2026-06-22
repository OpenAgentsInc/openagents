#!/usr/bin/env bun

import { readFile } from 'node:fs/promises'
import { chromium } from 'playwright'

import { verifyKhalaCodeCompletion } from '../workers/api/src/inference/khala-code-verifier.ts'

const usage =
  'usage: bun scripts/khala-code-headless-harness.mjs <artifact.html>'
const artifactPath = process.argv[2]

if (artifactPath === undefined || artifactPath.trim() === '') {
  console.error(usage)
  process.exit(2)
}

const html = await readFile(artifactPath, 'utf8')
const pageErrors = []
const consoleErrors = []

const browser = await chromium.launch({ headless: true })
try {
  const page = await browser.newPage({ viewport: { height: 720, width: 1280 } })
  page.on('pageerror', error => pageErrors.push(String(error)))
  page.on('console', message => {
    if (message.type() === 'error') {
      consoleErrors.push(message.text())
    }
  })

  await page.setContent(html, { timeout: 5_000, waitUntil: 'load' })
  await page.waitForTimeout(100)

  const before = await page.evaluate(() => {
    const probe = globalThis.__openagentsCrossyRoadState
    return typeof probe === 'function' ? probe() : null
  })

  for (const key of [
    'ArrowUp',
    'ArrowLeft',
    'ArrowRight',
    'ArrowDown',
    'w',
    'a',
    's',
    'd',
  ]) {
    await page.keyboard.press(key)
  }

  const afterKeys = await page.evaluate(() => {
    const probe = globalThis.__openagentsCrossyRoadState
    return typeof probe === 'function' ? probe() : null
  })

  await page.evaluate(() => {
    const restart = globalThis.__openagentsCrossyRoadRestart
    if (typeof restart === 'function') restart()
  })

  const afterRestart = await page.evaluate(() => {
    const probe = globalThis.__openagentsCrossyRoadState
    return typeof probe === 'function' ? probe() : null
  })

  const verifier = verifyKhalaCodeCompletion({
    content: html,
    requestId: `headless.${Date.now()}`,
    servedModel: 'openagents/khala-code',
    worker: 'khala-code-headless-harness',
  })

  const headlessChecks = {
    consoleErrors,
    hasProbe: before !== null && afterKeys !== null && afterRestart !== null,
    pageErrors,
    progressAdvanced:
      before !== null &&
      afterKeys !== null &&
      Number(afterKeys.progress) > Number(before.progress),
    restartReset:
      afterRestart !== null &&
      Number(afterRestart.progress) === 0 &&
      Number(afterRestart.player?.x) === 0 &&
      Number(afterRestart.player?.z) === 0,
  }
  const headlessPassed =
    headlessChecks.hasProbe &&
    headlessChecks.progressAdvanced &&
    headlessChecks.restartReset &&
    pageErrors.length === 0 &&
    consoleErrors.length === 0

  const output = {
    headless: {
      checks: headlessChecks,
      passed: headlessPassed,
      runner: 'playwright-chromium',
    },
    verifier,
  }

  console.log(JSON.stringify(output, null, 2))
  process.exit(verifier.verified && headlessPassed ? 0 : 1)
} finally {
  await browser.close()
}
