import { describe, expect, test } from 'vitest'

import {
  normalizeGoogleFinishReason,
  sanitizeGoogleToolSchema,
} from './google-tool-schema'

// Every keyword Vertex was MEASURED to reject with HTTP 400 for
// gemini-3.6-flash on 2026-07-31. A regression here is a live 502 for every
// tool-using client whose schema carries the keyword.
const REJECTED_KEYWORDS = [
  '$defs',
  '$ref',
  '$schema',
  'additionalProperties',
  'const',
  'definitions',
  'examples',
  'exclusiveMaximum',
  'exclusiveMinimum',
  'patternProperties',
  'uniqueItems',
] as const

const collectKeys = (value: unknown, into: Set<string>): Set<string> => {
  if (Array.isArray(value)) {
    for (const entry of value) {
      collectKeys(entry, into)
    }
    return into
  }
  if (typeof value === 'object' && value !== null) {
    for (const [key, child] of Object.entries(value)) {
      into.add(key)
      collectKeys(child, into)
    }
  }
  return into
}

describe('sanitizeGoogleToolSchema — $ref / $defs inlining', () => {
  test('inlines a $defs $ref instead of stripping it (the 502 root cause)', () => {
    // The exact shape a schemars/zod/pydantic-generated tool definition sends: a
    // nested object hoisted into `$defs` and referenced by `$ref`. Stripping
    // `$ref` leaves an EMPTY schema; leaving it in place is a hard Vertex 400.
    const sanitized = sanitizeGoogleToolSchema({
      $defs: {
        Location: {
          properties: {
            city: { type: 'string' },
            zip: { type: 'string' },
          },
          required: ['city'],
          type: 'object',
        },
      },
      $schema: 'https://json-schema.org/draft/2020-12/schema',
      additionalProperties: false,
      properties: {
        where: { $ref: '#/$defs/Location' },
      },
      required: ['where'],
      type: 'object',
    })

    expect(sanitized).toEqual({
      properties: {
        where: {
          properties: {
            city: { type: 'string' },
            zip: { type: 'string' },
          },
          required: ['city'],
          type: 'object',
        },
      },
      required: ['where'],
      type: 'object',
    })
    expect(collectKeys(sanitized, new Set()).has('$ref')).toBe(false)
    expect(collectKeys(sanitized, new Set()).has('$defs')).toBe(false)
  })

  test('inlines the legacy `definitions` pointer form as well', () => {
    expect(
      sanitizeGoogleToolSchema({
        definitions: { Name: { type: 'string' } },
        properties: { who: { $ref: '#/definitions/Name' } },
        type: 'object',
      }),
    ).toEqual({
      properties: { who: { type: 'string' } },
      type: 'object',
    })
  })

  test('sibling keywords beside $ref win over the inlined definition', () => {
    expect(
      sanitizeGoogleToolSchema({
        $defs: { Name: { description: 'generic', type: 'string' } },
        properties: {
          who: { $ref: '#/$defs/Name', description: 'the caller' },
        },
        type: 'object',
      }),
    ).toEqual({
      properties: { who: { description: 'the caller', type: 'string' } },
      type: 'object',
    })
  })

  test('a self-referential $ref terminates instead of hanging', () => {
    // A recursive tree node: `children` is an array of the node itself. Naive
    // inlining never terminates; the visiting stack collapses the cycle.
    const sanitized = sanitizeGoogleToolSchema({
      $defs: {
        Node: {
          properties: {
            children: { items: { $ref: '#/$defs/Node' }, type: 'array' },
            label: { type: 'string' },
          },
          type: 'object',
        },
      },
      properties: { root: { $ref: '#/$defs/Node' } },
      type: 'object',
    })

    expect(sanitized).toMatchObject({
      properties: {
        root: {
          properties: {
            children: { items: {}, type: 'array' },
            label: { type: 'string' },
          },
          type: 'object',
        },
      },
      type: 'object',
    })
    expect(collectKeys(sanitized, new Set()).has('$ref')).toBe(false)
  })

  test('a mutually recursive $ref pair terminates', () => {
    const sanitized = sanitizeGoogleToolSchema({
      $defs: {
        A: { properties: { b: { $ref: '#/$defs/B' } }, type: 'object' },
        B: { properties: { a: { $ref: '#/$defs/A' } }, type: 'object' },
      },
      properties: { start: { $ref: '#/$defs/A' } },
      type: 'object',
    })
    expect(collectKeys(sanitized, new Set()).has('$ref')).toBe(false)
  })

  test('an unresolvable $ref collapses to an empty schema rather than failing', () => {
    expect(
      sanitizeGoogleToolSchema({
        properties: { missing: { $ref: '#/$defs/NotThere' } },
        type: 'object',
      }),
    ).toEqual({ properties: { missing: {} }, type: 'object' })
  })
})

describe('sanitizeGoogleToolSchema — Vertex acceptance matrix', () => {
  test('removes every keyword Vertex rejects', () => {
    const sanitized = sanitizeGoogleToolSchema({
      $schema: 'https://json-schema.org/draft/2020-12/schema',
      additionalProperties: false,
      properties: {
        count: {
          exclusiveMaximum: 100,
          exclusiveMinimum: 0,
          type: 'integer',
        },
        kind: { const: 'repo', type: 'string' },
        tags: {
          items: { type: 'string' },
          type: 'array',
          uniqueItems: true,
        },
        wildcard: {
          patternProperties: { '^x-': { type: 'string' } },
          type: 'object',
        },
      },
      type: 'object',
    })

    const keys = collectKeys(sanitized, new Set())
    for (const rejected of REJECTED_KEYWORDS) {
      expect({ keyword: rejected, present: keys.has(rejected) }).toEqual({
        keyword: rejected,
        present: false,
      })
    }
  })

  test('drops `examples` while keeping the singular `example`', () => {
    expect(
      sanitizeGoogleToolSchema({
        example: 'docs/README.md',
        examples: ['a', 'b'],
        type: 'string',
      }),
    ).toEqual({ example: 'docs/README.md', type: 'string' })
  })

  test('keeps every keyword Vertex accepts', () => {
    const sanitized = sanitizeGoogleToolSchema({
      description: 'a bounded query',
      properties: {
        depth: { default: 1, minimum: 0, type: 'integer' },
        mode: { enum: ['fast', 'slow'], type: 'string' },
        when: { format: 'date-time', type: 'string' },
      },
      propertyOrdering: ['mode', 'depth'],
      required: ['mode'],
      title: 'Query',
      type: 'object',
    })

    expect(sanitized).toEqual({
      description: 'a bounded query',
      properties: {
        depth: { default: 1, minimum: 0, type: 'integer' },
        mode: { enum: ['fast', 'slow'], type: 'string' },
        when: { format: 'date-time', type: 'string' },
      },
      propertyOrdering: ['mode', 'depth'],
      required: ['mode'],
      title: 'Query',
      type: 'object',
    })
  })

  test('drops a `format` value outside the documented Google set', () => {
    expect(sanitizeGoogleToolSchema({ format: 'uri', type: 'string' })).toEqual({
      type: 'string',
    })
  })

  test('translates `const` into a single-member enum instead of dropping it', () => {
    expect(sanitizeGoogleToolSchema({ const: 'repo' })).toEqual({
      enum: ['repo'],
      type: 'string',
    })
  })

  test('renders non-string enum members as strings (Vertex enum is repeated string)', () => {
    expect(sanitizeGoogleToolSchema({ enum: [1, 2], type: 'integer' })).toEqual({
      enum: ['1', '2'],
      type: 'integer',
    })
  })

  test('never mistakes a property NAMED like a keyword for a keyword', () => {
    // A tool whose parameter is literally called `format` / `if` /
    // `additionalProperties` must keep those PROPERTY names untouched.
    expect(
      sanitizeGoogleToolSchema({
        properties: {
          additionalProperties: { type: 'boolean' },
          format: { type: 'string' },
          if: { type: 'string' },
        },
        type: 'object',
      }),
    ).toEqual({
      properties: {
        additionalProperties: { type: 'boolean' },
        format: { type: 'string' },
        if: { type: 'string' },
      },
      type: 'object',
    })
  })
})

describe('sanitizeGoogleToolSchema — type normalization', () => {
  test('array-valued `type` with null becomes a scalar type plus nullable', () => {
    expect(sanitizeGoogleToolSchema({ type: ['string', 'null'] })).toEqual({
      nullable: true,
      type: 'string',
    })
  })

  test('a bare `"null"` type becomes nullable', () => {
    expect(sanitizeGoogleToolSchema({ type: 'null' })).toEqual({
      nullable: true,
    })
  })

  test('a genuine union type becomes an anyOf of single-type schemas', () => {
    expect(sanitizeGoogleToolSchema({ type: ['string', 'number'] })).toEqual({
      anyOf: [{ type: 'string' }, { type: 'number' }],
    })
  })

  test('`oneOf` becomes `anyOf` (the Google subset has no oneOf)', () => {
    expect(
      sanitizeGoogleToolSchema({
        oneOf: [{ type: 'string' }, { type: 'integer' }],
      }),
    ).toEqual({ anyOf: [{ type: 'string' }, { type: 'integer' }] })
  })

  test('a nullable-only anyOf variant folds onto the parent', () => {
    expect(
      sanitizeGoogleToolSchema({
        anyOf: [{ type: 'string' }, { type: 'null' }],
      }),
    ).toEqual({ nullable: true, type: 'string' })
  })

  test('preserves a pre-existing anyOf when a union type is also present', () => {
    const sanitized = sanitizeGoogleToolSchema({
      anyOf: [{ type: 'string' }],
      type: ['integer', 'boolean'],
    }) as Record<string, unknown>
    expect(sanitized['anyOf']).toBeUndefined()
    expect(sanitized['allOf']).toEqual([
      { anyOf: [{ type: 'string' }] },
      { anyOf: [{ type: 'integer' }, { type: 'boolean' }] },
    ])
  })
})

describe('normalizeGoogleFinishReason', () => {
  test('maps the Google enum onto the OpenAI finish_reason vocabulary', () => {
    expect(normalizeGoogleFinishReason('STOP')).toBe('stop')
    expect(normalizeGoogleFinishReason('MAX_TOKENS')).toBe('length')
    expect(normalizeGoogleFinishReason('SAFETY')).toBe('content_filter')
    expect(normalizeGoogleFinishReason('PROHIBITED_CONTENT')).toBe(
      'content_filter',
    )
    expect(normalizeGoogleFinishReason('RECITATION')).toBe('content_filter')
  })

  test('upgrades a plain stop to tool_calls when the turn produced tool calls', () => {
    expect(normalizeGoogleFinishReason('STOP', true)).toBe('tool_calls')
    expect(normalizeGoogleFinishReason(undefined, true)).toBe('tool_calls')
  })

  test('a truncation keeps `length` even when tool calls were produced', () => {
    expect(normalizeGoogleFinishReason('MAX_TOKENS', true)).toBe('length')
  })

  test('an absent or unknown reason falls back to `stop`, never a raw enum', () => {
    expect(normalizeGoogleFinishReason(undefined)).toBe('stop')
    expect(normalizeGoogleFinishReason('')).toBe('stop')
    expect(normalizeGoogleFinishReason('SOME_FUTURE_ENUM')).toBe('stop')
  })

  test('is idempotent over its own output', () => {
    for (const value of ['stop', 'length', 'content_filter', 'tool_calls']) {
      expect(normalizeGoogleFinishReason(value)).toBe(value)
    }
  })
})
