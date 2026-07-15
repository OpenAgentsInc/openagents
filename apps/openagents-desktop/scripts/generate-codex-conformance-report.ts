import { mkdirSync, writeFileSync } from "node:fs"
import { dirname, resolve } from "node:path"

import { makeCodexConformanceReport } from "../src/codex-conformance.ts"

const output = resolve(import.meta.dirname, "../../../docs/receipts/2026-07-15-codex-app-server-conformance.json")
mkdirSync(dirname(output), { recursive: true })
writeFileSync(output, `${JSON.stringify(makeCodexConformanceReport(), null, 2)}\n`, { mode: 0o644 })
console.log(output)
