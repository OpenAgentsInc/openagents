#!/usr/bin/env node

import { spawnSync } from "node:child_process"
import { mkdir, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { homedir, tmpdir } from "node:os"
import { fileURLToPath, pathToFileURL } from "node:url"

import { EVIDENCE_SCHEMA_VERSION } from "./ui-gap.mjs"

const scriptDirectory = dirname(fileURLToPath(import.meta.url))

function parseArguments(argv) {
  const result = {}
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index]
    const value = argv[index + 1]
    if (!key?.startsWith("--") || value === undefined) throw new Error(`Invalid option near ${key ?? "end"}`)
    result[key.slice(2)] = value
  }
  return result
}

async function domNodes(page) {
  return page.evaluate(() => {
    const inferredRole = (element) => {
      const explicit = element.getAttribute("role")
      if (explicit) return explicit
      const tag = element.tagName.toLowerCase()
      if (tag === "button") return "button"
      if (tag === "a") return "link"
      if (tag === "select") return "combobox"
      if (tag === "textarea") return "textbox"
      if (tag === "input") return element.getAttribute("type") === "checkbox" ? "checkbox" : "textbox"
      if (/^h[1-6]$/.test(tag)) return "heading"
      if (tag === "main" || tag === "nav" || tag === "aside" || tag === "section") return tag
      return "group"
    }
    const referencedText = (element, attribute) => {
      const ids = (element.getAttribute(attribute) ?? "").split(/\s+/).filter(Boolean)
      return ids.map((id) => document.getElementById(id)?.textContent?.trim() ?? "").filter(Boolean).join(" ")
    }
    const label = (element) => {
      const explicit = element.getAttribute("aria-label")
      if (explicit) return explicit
      const referenced = referencedText(element, "aria-labelledby")
      if (referenced) return referenced
      const title = element.getAttribute("title")
      if (title) return title
      const text = element.textContent?.trim().replace(/\s+/g, " ") ?? ""
      return text.slice(0, 160) || null
    }
    const selectors = [
      "button", "a[href]", "input", "select", "textarea", "[contenteditable='true']",
      "[role]", "main", "nav", "aside", "section[aria-label]", "h1", "h2", "h3",
    ].join(",")
    return [...document.querySelectorAll(selectors)]
      .filter((element) => {
        const box = element.getBoundingClientRect()
        const style = getComputedStyle(element)
        return box.width > 0 && box.height > 0 && style.visibility !== "hidden" && style.display !== "none"
      })
      .map((element, index) => {
        const box = element.getBoundingClientRect()
        const role = inferredRole(element)
        const canPress = ["button", "link", "checkbox"].includes(role)
        const canSet = ["textbox", "combobox"].includes(role)
        return {
          path: `dom.${index}`,
          depth: 0,
          role: `ARIA:${role}`,
          subrole: null,
          title: label(element),
          description: referencedText(element, "aria-describedby") || null,
          identifier: (element.getAttribute("data-en-key") ?? element.id) || null,
          value: role === "checkbox" ? element.getAttribute("aria-checked") === "true" : null,
          enabled: !("disabled" in element && element.disabled) && element.getAttribute("aria-disabled") !== "true",
          focused: document.activeElement === element,
          frame: { x: box.x, y: box.y, width: box.width, height: box.height },
          actions: [...(canPress ? ["press"] : []), ...(canSet ? ["setValue"] : [])],
          children: element.children.length,
        }
      })
  })
}

function sanitizeRuntimeText(value, replacements) {
  let result = value
  for (const [privateValue, publicValue] of replacements) {
    if (privateValue) result = result.split(privateValue).join(publicValue)
  }
  return result
    .replace(/\/(?:private\/)?tmp\/[A-Za-z0-9._/-]+/g, "<TEMP_PATH>")
    .replace(/\/var\/folders\/[A-Za-z0-9._/-]+/g, "<TEMP_PATH>")
}

function sanitizeRuntimeValue(value, replacements) {
  if (typeof value === "string") return sanitizeRuntimeText(value, replacements)
  if (Array.isArray(value)) return value.map((item) => sanitizeRuntimeValue(item, replacements))
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, sanitizeRuntimeValue(item, replacements)]),
    )
  }
  return value
}

async function main() {
  const options = parseArguments(process.argv.slice(2))
  const repoRoot = resolve(options["repo-root"])
  const workspace = resolve(options.workspace)
  const outputDirectory = resolve(options["output-dir"])
  const output = resolve(options.out)
  await mkdir(outputDirectory, { recursive: true })
  const launcherPath = join(repoRoot, "apps/openagents-desktop/scripts/ui-harness/launch-isolated-app.ts")
  const { launchIsolatedDesktopApp } = await import(pathToFileURL(launcherPath).href)
  const desktop = await launchIsolatedDesktopApp({ launchCwd: workspace })
  try {
    await desktop.page.waitForSelector('[data-en-key], [class*="oa-react"]', { timeout: 60_000 })
    await desktop.page.waitForTimeout(1_000)
    const nodes = await domNodes(desktop.page)
    const native = spawnSync(
      "swift",
      [join(scriptDirectory, "macos-ui-capture.swift"), "--pid", String(desktop.app.process().pid), "--output-dir", outputDirectory],
      { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    )
    if (native.status !== 0) throw new Error(`Native capture failed: ${(native.stderr || native.stdout).trim()}`)
    const replacements = [
      [workspace, "<TEST_WORKSPACE>"],
      [repoRoot, "<OPENAGENTS_ROOT>"],
      [homedir(), "<HOME>"],
      [tmpdir(), "<TEMP>"],
    ]
    const captured = sanitizeRuntimeValue(JSON.parse(native.stdout), replacements)
    const { limitations = [], ...runtime } = captured
    runtime.accessibility = {
      provider: "browser-dom",
      trusted: true,
      truncated: false,
      nodes: sanitizeRuntimeValue(nodes, replacements),
    }
    const evidence = {
      schemaVersion: EVIDENCE_SCHEMA_VERSION,
      kind: "runtime-capture",
      generatedAt: new Date().toISOString(),
      analysisId: options["analysis-id"],
      targetId: options["target-id"] ?? "openagents",
      runtime,
      limitations: [
        "The accessibility inventory uses the live Electron DOM. It is not a VoiceOver or macOS AX conformance result.",
        ...limitations,
      ],
    }
    await writeFile(output, `${JSON.stringify(evidence, null, 2)}\n`)
  } finally {
    await desktop.close()
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack ?? error.message}\n`)
  process.exitCode = 1
})
