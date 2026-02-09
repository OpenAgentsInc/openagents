/**
 * Fallback LanguageModel: tries primary then fallback on error.
 * Used to run OpenRouter (primary) with Cloudflare Workers AI (fallback).
 */
import type { LanguageModel } from "@effect/ai"
import { Effect, Stream } from "effect"

/**
 * Returns an Effect that resolves to a LanguageModel Service that tries primary first,
 * then fallback on any error (per generateText/generateObject call, or per streamText stream).
 */
export const makeFallbackLanguageModel = (
  primary: LanguageModel.Service,
  fallback: LanguageModel.Service,
): Effect.Effect<LanguageModel.Service, never> =>
  Effect.succeed({
    generateText: (options) =>
      primary.generateText(options).pipe(Effect.catchAll(() => fallback.generateText(options))),
    generateObject: (options) =>
      primary.generateObject(options).pipe(Effect.catchAll(() => fallback.generateObject(options))),
    streamText: (options) =>
      primary.streamText(options).pipe(Stream.catchAll(() => fallback.streamText(options))),
  })
