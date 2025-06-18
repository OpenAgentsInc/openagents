import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import { InvalidRequestError, SecurityError } from "../src/Claude/errors.js"
import { validateRequest, validateSecurity } from "../src/Claude/Integration.js"

describe("Claude Integration", () => {
  describe("validateRequest", () => {
    it.effect("should validate valid request", () =>
      Effect.gen(function*() {
        const request = {
          url: "http://localhost:3000",
          fullPage: true,
          interactions: [
            { action: "click" as const, selector: "#button" }
          ]
        }

        const validated = yield* validateRequest(request)
        expect(validated.url).toBe("http://localhost:3000")
        expect(validated.fullPage).toBe(true)
      }))

    it.effect("should reject invalid request", () =>
      Effect.gen(function*() {
        const request = {
          url: 123, // Invalid type
          fullPage: "yes" // Invalid type
        }

        yield* validateRequest(request)
      }).pipe(
        Effect.flip,
        Effect.map((error) => {
          expect(error).toBeInstanceOf(InvalidRequestError)
        })
      ))
  })

  describe("validateSecurity", () => {
    it.effect("should allow localhost URLs", () =>
      Effect.gen(function*() {
        const urls = [
          "http://localhost:3000",
          "http://127.0.0.1:8080",
          "http://0.0.0.0:4000"
        ]

        for (const url of urls) {
          const result = yield* validateSecurity(url, undefined)
          // URL constructor normalizes URLs, adding trailing slash to bare domain URLs
          const expectedUrl = new URL(url).toString()
          expect(result.url).toBe(expectedUrl)
        }
      }))

    it.effect("should reject non-localhost URLs when restricted", () =>
      Effect.gen(function*() {
        const url = "http://example.com"
        const options = { allowedHosts: ["localhost", "127.0.0.1", "0.0.0.0"] }
        yield* validateSecurity(url, undefined, options)
      }).pipe(
        Effect.flip,
        Effect.map((error) => {
          expect(error).toBeInstanceOf(SecurityError)
          expect(error.message).toContain("not in allowed list")
        })
      ))

    it.effect("should validate output path", () =>
      Effect.gen(function*() {
        const url = "http://localhost:3000"
        const validPath = ".autotest/screenshots/test.png"

        const result = yield* validateSecurity(url, validPath)
        expect(result.outputPath).toBe(validPath)
      }))

    it.effect("should reject paths outside output directory", () =>
      Effect.gen(function*() {
        const url = "http://localhost:3000"
        const invalidPath = "../../../etc/passwd"

        yield* validateSecurity(url, invalidPath)
      }).pipe(
        Effect.flip,
        Effect.map((error) => {
          expect(error).toBeInstanceOf(SecurityError)
          expect(error.message).toContain("must be within")
        })
      ))
  })
})
