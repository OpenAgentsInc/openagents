// Generate `src/shared/khala-crossy-road-game.generated.ts` from the committed,
// 6/6-verified Khala-built crossy-road artifact.
//
// WHY GENERATE: the in-Verse game screen runs the game in a same-origin srcdoc
// iframe (so its global `window`/`document`/`THREE`/keydown listeners stay fully
// isolated from the Verse), and textures the iframe's live canvas onto an
// in-world board. The browser bundle therefore needs the game HTML as a STRING.
// Rather than hand-copy (and drift from) the committed artifact, we derive the
// string from it here, recording the source path + sha256 so the generated file
// is auditable and regenerable.
//
// The ONLY transform is dropping the CDN `<script src=.../three.min.js>` tag:
// the iframe gets `THREE` injected from the parent app's bundled `three`
// (version-stable across the basic APIs the game uses) BEFORE the game script
// runs, so the game has NO network dependency and works headless + offline.
//
// Run: bun apps/autopilot-desktop/scripts/generate-khala-crossy-road-game.ts

import { createHash } from "node:crypto"
import { readFileSync, writeFileSync } from "node:fs"
import { join, resolve } from "node:path"

const APP_ROOT = resolve(import.meta.dir, "..")
const REPO_ROOT = resolve(APP_ROOT, "..", "..")
const ARTIFACT = join(
  REPO_ROOT,
  "scripts",
  "khala-demo",
  "artifacts",
  "khala-crossy-road-northstar-passing.v1.html",
)
const OUT = join(APP_ROOT, "src", "shared", "khala-crossy-road-game.generated.ts")

const raw = readFileSync(ARTIFACT, "utf8")
const sha = createHash("sha256").update(raw).digest("hex")

// Drop the CDN three.js <script src=...> — the iframe gets THREE from the parent.
const stripped = raw.replace(
  /<script\s+src="https:\/\/cdnjs\.cloudflare\.com\/[^"]*three[^"]*"><\/script>\s*/i,
  "",
)
if (stripped === raw) {
  throw new Error("expected to strip the CDN three.js <script src> but found none")
}

// Force `preserveDrawingBuffer: true` on the game's WebGLRenderer. WHY: the
// in-Verse screen samples the game's WebGL canvas as a THREE.CanvasTexture from a
// SEPARATE renderer/frame. With the default `preserveDrawingBuffer: false` the
// drawing buffer is cleared right after the browser composites the game frame, so
// an external texImage2D read of the canvas yields a BLANK (black) image. Keeping
// the drawing buffer lets the Verse host re-read the last drawn game frame each
// time it dirties the texture. This is a rendering-transport tweak ONLY; it does
// not change game logic, so the artifact stays the verified game.
const withPreserve = stripped.replace(
  /new\s+THREE\.WebGLRenderer\(\s*\{([^}]*)\}\s*\)/,
  (match, inner: string) =>
    /preserveDrawingBuffer/.test(inner)
      ? match
      : `new THREE.WebGLRenderer({${inner}, preserveDrawingBuffer: true })`,
)
if (withPreserve === stripped) {
  throw new Error(
    "expected to inject preserveDrawingBuffer into the game's WebGLRenderer but found none",
  )
}

// Escape for a TS template literal.
const escaped = withPreserve.replace(/\\/g, "\\\\").replace(/`/g, "\\`").replace(/\$\{/g, "\\${")

const relArtifact = "scripts/khala-demo/artifacts/khala-crossy-road-northstar-passing.v1.html"
const out = `// GENERATED — do not edit by hand.
//
// Source: ${relArtifact}
// sha256: ${sha}
// Regenerate: bun apps/autopilot-desktop/scripts/generate-khala-crossy-road-game.ts
//
// The committed, 6/6-verified Khala-built crossy-road game, with the CDN
// three.js <script src> removed. The in-Verse game screen runs this inside a
// same-origin srcdoc iframe with THREE injected from the parent's bundled three.

export const KHALA_CROSSY_ROAD_ARTIFACT_SHA256 = ${JSON.stringify(sha)} as const

export const KHALA_CROSSY_ROAD_GAME_HTML = \`${escaped}\` as const
`

writeFileSync(OUT, out)
console.log(`wrote ${OUT}\n  source sha256 ${sha}\n  html bytes ${withPreserve.length}`)
