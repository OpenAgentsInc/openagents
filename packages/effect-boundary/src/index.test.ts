import { describe, test } from 'bun:test'
import { Config, ConfigProvider, Effect, Schema as S } from 'effect'

import {
  decodeRowEffect,
  expectBoundaryParseError,
  parseJsonEffect,
  readConfigEffect,
} from './index.js'

const Example = S.Struct({
  id: S.String,
})

describe('Effect boundary helpers', () => {
  test('returns a typed error for malformed JSON', async () => {
    const error = await Effect.runPromise(
      parseJsonEffect(Example, '{', 'test.json').pipe(Effect.flip),
    )

    expectBoundaryParseError(error, {
      operation: 'test.json',
      reasonRef: 'boundary.json.malformed',
    })
  })

  test('returns a typed error for missing fields', async () => {
    const error = await Effect.runPromise(
      parseJsonEffect(Example, '{}', 'test.body').pipe(Effect.flip),
    )

    expectBoundaryParseError(error, {
      operation: 'test.body',
      reasonRef: 'boundary.schema.invalid',
    })
  })

  test('returns a typed error for wrong row shape', async () => {
    const error = await Effect.runPromise(
      decodeRowEffect(Example, { id: 123 }, 'rows.example').pipe(Effect.flip),
    )

    expectBoundaryParseError(error, {
      operation: 'rows.example',
      reasonRef: 'boundary.schema.invalid',
    })
  })

  test('redacts config failures to a reason ref', async () => {
    const effect = readConfigEffect(
      Config.string('SECRET_TOKEN'),
      'config.test.secret_token',
    ).pipe(
      Effect.provide(
        ConfigProvider.layer(ConfigProvider.fromUnknown({})),
      ),
    )
    const error = await Effect.runPromise(effect.pipe(Effect.flip))

    expectBoundaryParseError(error, {
      operation: 'config.test.secret_token',
      reasonRef: 'boundary.config.invalid',
    })
  })
})
