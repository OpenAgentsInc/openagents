import { createHash } from "node:crypto"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Effect } from "effect"
import { afterEach, describe, expect, test } from "vite-plus/test"

import { openWorkspaceService, type DesktopWorkspaceService } from "../workspace-service.ts"
import {
  IdeCursorAuthorityFailure,
  IdeCursorDocumentAuthority,
} from "./cursor-service.ts"
import {
  ideCursorFixtureAnchor,
  ideCursorFixtureCandidate,
  ideCursorFixtureRequest,
} from "./cursor-fixture.ts"
import {
  IdeDocumentGenerationSchema,
} from "./project-contract.ts"
import {
  ideCursorAcceptedText,
  ideCursorWorkspaceDocumentRef,
  ideCursorWorkspaceFileRef,
  makeIdeCursorWorkspaceAuthorityLayer,
} from "./cursor-workspace-authority.ts"

const roots: string[] = []
const services: DesktopWorkspaceService[] = []

afterEach(() => {
  for (const service of services.splice(0)) service.dispose()
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const sha256 = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const workspace = (content: string): DesktopWorkspaceService => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-cursor-authority-"))
  roots.push(root)
  mkdirSync(path.join(root, "src"))
  writeFileSync(path.join(root, "src", "app.ts"), content)
  const service = openWorkspaceService(root, { grantRef: "workspace.grant.cursor-authority" })
  services.push(service)
  return service
}

const anchorFor = (content: string, selection = { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } }) =>
  ideCursorFixtureAnchor({
    pathRef: "src/app.ts",
    fileRef: ideCursorWorkspaceFileRef("src/app.ts"),
    documentRef: ideCursorWorkspaceDocumentRef("src/app.ts"),
    documentGeneration: IdeDocumentGenerationSchema.make(1),
    selection,
    contentDigest: sha256(content),
  })

const candidateFor = (
  content: string,
  text: string,
  replace: ReturnType<typeof anchorFor>["selection"],
  suffix: string,
) => {
  const anchor = anchorFor(content, replace)
  const request = ideCursorFixtureRequest(suffix, 1, { anchor })
  return ideCursorFixtureCandidate(request, { replace, text })
}

const run = <A, E>(service: DesktopWorkspaceService, effect: Effect.Effect<A, E, IdeCursorDocumentAuthority>) =>
  Effect.runPromise(effect.pipe(Effect.provide(makeIdeCursorWorkspaceAuthorityLayer(service))))

const document = (service: DesktopWorkspaceService) => {
  const result = service.openDocument({ grantRef: service.grantRef, pathRef: "src/app.ts" })
  if (result.state !== "available") throw new Error(`document unavailable: ${result.state}`)
  return result.document
}

describe("IdeCursorWorkspaceAuthority", () => {
  test("verifies canonical path-derived refs, digest, and range", async () => {
    const content = "export const answer = 41\n"
    const service = workspace(content)
    await run(service, Effect.gen(function* () {
      const authority = yield* IdeCursorDocumentAuthority
      yield* authority.validate(anchorFor(content, {
        start: { line: 1, column: 14 },
        end: { line: 1, column: 20 },
      }))
      const stale = yield* authority.validate(anchorFor("not the canonical content")).pipe(Effect.flip)
      expect(stale).toBeInstanceOf(IdeCursorAuthorityFailure)
      expect(stale.reason).toBe("stale")

      const wrongRef = anchorFor(content, { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } })
      const refFailure = yield* authority.validate({
        ...wrongRef,
        fileRef: ideCursorWorkspaceFileRef("src/other.ts"),
      }).pipe(Effect.flip)
      expect(refFailure.reason).toBe("stale")
    }))
  })

  test("computes deterministic word, line, and full acceptance prefixes", () => {
    expect(ideCursorAcceptedText("  answer + more\nnext()", "word")).toBe("  answer")
    expect(ideCursorAcceptedText("answer + more\r\nnext()", "line")).toBe("answer + more\r\n")
    expect(ideCursorAcceptedText("answer + more\nnext()", "all")).toBe("answer + more\nnext()")
  })

  test("accepts through a canonical revision CAS and restores one exact undo boundary", async () => {
    const content = "const value = \n"
    const range = { start: { line: 1, column: 15 }, end: { line: 1, column: 15 } }
    const service = workspace(content)
    const candidate = candidateFor(content, "answer + more\nnext()", range, "accept-undo")
    const result = await run(service, Effect.gen(function* () {
      const authority = yield* IdeCursorDocumentAuthority
      const accepted = yield* authority.accept(candidate, "word")
      expect(document(service).content).toBe("const value = answer\n")
      const undone = yield* authority.undo(candidate)
      expect(document(service).content).toBe(content)

      yield* authority.accept(candidate, "line")
      expect(document(service).content).toBe("const value = answer + more\n\n")
      yield* authority.undo(candidate)

      yield* authority.accept(candidate, "all")
      expect(document(service).content).toBe("const value = answer + more\nnext()\n")
      yield* authority.undo(candidate)
      const duplicateUndo = yield* authority.undo(candidate).pipe(Effect.flip)
      return { accepted, undone, duplicateUndo }
    }))
    expect(result.accepted.previousContentDigest).toBe(sha256(content))
    expect(result.accepted.resultContentDigest).toBe(sha256("const value = answer\n"))
    expect(result.undone.resultContentDigest).toBe(sha256(content))
    expect(result.duplicateUndo.reason).toBe("conflict")
  })

  test("refuses undo after any external revision changes the accepted post-image", async () => {
    const content = "const value = 1\n"
    const range = { start: { line: 1, column: 15 }, end: { line: 1, column: 16 } }
    const service = workspace(content)
    const candidate = candidateFor(content, "2", range, "external-conflict")
    const failure = await run(service, Effect.gen(function* () {
      const authority = yield* IdeCursorDocumentAuthority
      yield* authority.accept(candidate, "all")
      const accepted = document(service)
      const external = service.saveDocument({
        grantRef: service.grantRef,
        pathRef: accepted.pathRef,
        content: "const value = 3\n",
        expectedRevisionRef: accepted.revisionRef,
      })
      expect(external.state).toBe("saved")
      return yield* authority.undo(candidate).pipe(Effect.flip)
    }))
    expect(failure.reason).toBe("conflict")
    expect(document(service).content).toBe("const value = 3\n")
  })

  test("refuses cross-file next edits without a target-document anchor", async () => {
    const content = "const value = 1\n"
    const service = workspace(content)
    const anchor = anchorFor(content)
    const request = ideCursorFixtureRequest("cross-file", 1, {
      anchor,
      intent: { _tag: "NextEdit" },
    })
    const completion = ideCursorFixtureCandidate(request)
    if (completion._tag !== "Completion") throw new Error("fixture did not produce a completion")
    const candidate = {
      ...completion,
      _tag: "NextEdit" as const,
      targetPathRef: "src/other.ts",
      explanation: "Edit the next file.",
    }
    const result = await run(service, Effect.gen(function* () {
      const authority = yield* IdeCursorDocumentAuthority
      return yield* authority.accept(candidate, "all").pipe(Effect.flip)
    }))
    expect(result.reason).toBe("stale")
    expect(document(service).content).toBe(content)
  })
})
