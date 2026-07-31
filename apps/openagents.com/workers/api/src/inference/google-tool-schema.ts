// Shared Google tool-schema sanitizer for the two Google lanes (Vertex Gemini
// `vertex-gemini-adapter.ts` and Generative Language / Gemma 4
// `gemma4-adapter.ts`), plus the shared Google -> OpenAI finish-reason
// normalizer.
//
// WHY THIS EXISTS (the 2026-07-31 hosted-Gemini 502):
// Google's `FunctionDeclaration.parameters` is NOT JSON Schema — it is the
// bounded OpenAPI-3.0 `Schema` subset documented at
// https://ai.google.dev/api/caching#Schema. Anything outside that field set is a
// HARD `400 INVALID_ARGUMENT: Unknown name "<keyword>"`, which the gateway route
// then surfaces as a 502 `provider_error`. Clients that send stock JSON Schema
// (OpenAI tool definitions, schemars/zod-generated schemas, the Omega desktop
// app) routinely carry `$defs` + `$ref`, `const`, `examples`,
// `exclusiveMinimum`, `patternProperties`, `uniqueItems`, and array-valued
// `type` — every one of which 400s.
//
// Measured Vertex acceptance matrix for `gemini-3.6-flash` (2026-07-31):
//   REJECTED (400): $ref, $defs/definitions, exclusiveMinimum, examples, const,
//                   patternProperties, uniqueItems, array-valued `type`
//   ACCEPTED (200): title, description, format, minimum, default, enum, oneOf,
//                   anyOf, allOf, nullable
//
// STRATEGY: an ALLOWLIST, not a denylist. Vertex rejects UNKNOWN field names, so
// only keywords in the documented subset survive; everything else is dropped or
// translated. Dropping a keyword loses a hint, keeping an unknown one loses the
// whole request.
//
// `$ref` is INLINED, never stripped — stripping `$ref` leaves an empty schema
// and the model loses the parameter shape entirely. Recursive/self-referential
// definitions terminate via a visiting stack (a cycle collapses to an empty
// schema) plus an absolute depth bound, so a recursive schema can never spin the
// request thread.
//
// Semantics ported from the reviewed reference implementation `resolve_refs` +
// `adapt_to_json_schema_subset` in
// `omega/crates/language_model_core/src/tool_schema.rs`.
//
// This module is pure: no I/O, no credentials, no request state.

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

// Absolute recursion bound. Cycles are already caught by the visiting stack;
// this additionally bounds a pathologically deep (but acyclic) schema.
const MAX_SCHEMA_DEPTH = 64

// String keywords copied verbatim when the value is a string.
const STRING_KEYWORDS: ReadonlySet<string> = new Set([
  'description',
  'pattern',
  'title',
])

// Numeric keywords copied verbatim when the value is a finite number.
const NUMBER_KEYWORDS: ReadonlySet<string> = new Set([
  'maxItems',
  'maxLength',
  'maxProperties',
  'maximum',
  'minItems',
  'minLength',
  'minProperties',
  'minimum',
])

// The `format` values Google documents for its Schema subset. Any other format
// (`uri`, `email`, `uuid`, ...) is dropped rather than risking a 400.
const SUPPORTED_FORMATS: ReadonlySet<string> = new Set([
  'date-time',
  'double',
  'enum',
  'float',
  'int32',
  'int64',
])

// Decode the two JSON Pointer escapes so a `$defs` key containing `/` or `~`
// still resolves.
const decodePointerSegment = (segment: string): string =>
  segment.replace(/~1/gu, '/').replace(/~0/gu, '~')

// The string members of a `required` / `propertyOrdering` array, or undefined
// when there are none (an empty array is not worth emitting).
const stringList = (raw: unknown): ReadonlyArray<string> | undefined => {
  if (!Array.isArray(raw)) {
    return undefined
  }
  const names = raw.filter((name): name is string => typeof name === 'string')
  return names.length === 0 ? undefined : names
}

type RefContext = Readonly<{
  defs: Record<string, unknown> | undefined
  legacyDefs: Record<string, unknown> | undefined
}>

// Resolve a same-document `#/$defs/<name>` or `#/definitions/<name>` pointer.
// Any other pointer form (remote refs, `#/properties/...`, `#`) is unsupported
// and resolves to undefined, which collapses to an empty schema rather than
// failing the whole request.
const refTarget = (ref: string, context: RefContext): unknown => {
  if (ref.startsWith('#/$defs/')) {
    const name = decodePointerSegment(ref.slice('#/$defs/'.length))
    return context.defs?.[name]
  }
  if (ref.startsWith('#/definitions/')) {
    const name = decodePointerSegment(ref.slice('#/definitions/'.length))
    return context.legacyDefs?.[name]
  }
  return undefined
}

// Vertex `enum` is a repeated STRING. A numeric/boolean enum member would be
// rejected by the proto parser, so non-strings are rendered as their JSON text.
const enumMember = (value: unknown): string =>
  typeof value === 'string' ? value : JSON.stringify(value ?? null)

// Port of `push_any_of_constraint`: add one more `anyOf` constraint to a schema
// that may already carry `anyOf` / `allOf`, without silently dropping either.
type CompositionState = Readonly<{
  anyOf: ReadonlyArray<unknown> | undefined
  allOf: ReadonlyArray<unknown> | undefined
}>

const pushAnyOfConstraint = (
  current: CompositionState,
  added: ReadonlyArray<unknown>,
): CompositionState => {
  if (current.anyOf !== undefined) {
    return {
      allOf: [
        ...(current.allOf ?? []),
        { anyOf: current.anyOf },
        { anyOf: added },
      ],
      anyOf: undefined,
    }
  }
  if (current.allOf !== undefined) {
    return { allOf: [...current.allOf, { anyOf: added }], anyOf: undefined }
  }
  return { allOf: undefined, anyOf: added }
}

// Port of `collapse_nullable_only_any_of`: `{ "nullable": true }`-only variants
// (what a `{"type": "null"}` branch becomes) are folded onto the parent, which
// is how OpenAPI 3.0 expresses nullability. A single remaining variant is
// inlined when its keys do not collide with the parent's.
const collapseNullableOnlyAnyOf = (
  schema: Record<string, unknown>,
): Record<string, unknown> => {
  const anyOf = schema['anyOf']
  if (!Array.isArray(anyOf)) {
    return schema
  }
  const isNullableOnly = (entry: unknown): boolean =>
    isRecord(entry) &&
    Object.keys(entry).length === 1 &&
    entry['nullable'] === true
  if (!anyOf.some(isNullableOnly)) {
    return schema
  }
  const remaining = anyOf.filter(entry => !isNullableOnly(entry))
  const withNullable: Record<string, unknown> = { ...schema, nullable: true }
  delete withNullable['anyOf']
  if (remaining.length === 0) {
    return withNullable
  }
  const only = remaining[0]
  if (
    remaining.length === 1 &&
    isRecord(only) &&
    Object.keys(only).every(key => !(key in withNullable))
  ) {
    return { ...withNullable, ...only }
  }
  return { ...withNullable, anyOf: remaining }
}

// Sanitize one schema node. Returns undefined when the node is not an object
// schema (JSON Schema's boolean `true`/`false` schemas have no Google subset
// equivalent), so the caller omits the keyword entirely.
//
// The traversal is SCHEMA-AWARE: `properties` values are a map of member NAME ->
// schema, so a tool parameter legitimately named `if`, `format`, or
// `additionalProperties` is never mistaken for a keyword. `enum` / `default` /
// `example` / `const` hold arbitrary data and are never walked as schemas.
const sanitizeSchemaNode = (
  value: unknown,
  context: RefContext,
  visiting: ReadonlyArray<string>,
  depth: number,
): Record<string, unknown> | undefined => {
  if (!isRecord(value)) {
    return undefined
  }
  if (depth > MAX_SCHEMA_DEPTH) {
    return {}
  }

  // --- $ref inlining (before anything else; the target is a schema too) ---
  const ref = value['$ref']
  if (typeof ref === 'string') {
    // Cycle guard: a self-referential definition (a tree node whose children are
    // tree nodes) collapses to an empty schema instead of recursing forever.
    if (visiting.includes(ref)) {
      return {}
    }
    const target = refTarget(ref, context)
    if (!isRecord(target)) {
      return {}
    }
    const siblings: Record<string, unknown> = { ...value }
    delete siblings['$ref']
    // Sibling keywords beside `$ref` (e.g. a local `description`) win over the
    // definition's, matching the reference implementation.
    return sanitizeSchemaNode(
      { ...target, ...siblings },
      context,
      [...visiting, ref],
      depth + 1,
    )
  }

  const out: Record<string, unknown> = {}
  let anyOf: ReadonlyArray<unknown> | undefined
  let allOf: ReadonlyArray<unknown> | undefined
  let oneOf: ReadonlyArray<unknown> | undefined

  const sanitizeList = (raw: unknown): ReadonlyArray<unknown> | undefined => {
    if (!Array.isArray(raw)) {
      return undefined
    }
    const list = raw
      .map(entry => sanitizeSchemaNode(entry, context, visiting, depth + 1))
      .filter((entry): entry is Record<string, unknown> => entry !== undefined)
    return list.length === 0 ? undefined : list
  }

  for (const [key, child] of Object.entries(value)) {
    if (STRING_KEYWORDS.has(key)) {
      if (typeof child === 'string') {
        out[key] = child
      }
      continue
    }
    if (NUMBER_KEYWORDS.has(key)) {
      if (typeof child === 'number' && Number.isFinite(child)) {
        out[key] = child
      }
      continue
    }
    switch (key) {
      case 'properties': {
        if (!isRecord(child)) {
          break
        }
        const properties: Record<string, unknown> = {}
        for (const [name, member] of Object.entries(child)) {
          const sanitized = sanitizeSchemaNode(
            member,
            context,
            visiting,
            depth + 1,
          )
          if (sanitized !== undefined) {
            properties[name] = sanitized
          }
        }
        out['properties'] = properties
        break
      }
      case 'items': {
        // Draft-04 tuple form (`items: [schemaA, schemaB]`) has no Google
        // equivalent; the first entry is the closest honest approximation.
        const itemSchema = Array.isArray(child) ? child[0] : child
        const sanitized = sanitizeSchemaNode(
          itemSchema,
          context,
          visiting,
          depth + 1,
        )
        if (sanitized !== undefined) {
          out['items'] = sanitized
        }
        break
      }
      case 'anyOf': {
        anyOf = sanitizeList(child)
        break
      }
      case 'allOf': {
        allOf = sanitizeList(child)
        break
      }
      case 'oneOf': {
        oneOf = sanitizeList(child)
        break
      }
      case 'required': {
        const names = stringList(child)
        if (names !== undefined) {
          out['required'] = names
        }
        break
      }
      case 'propertyOrdering': {
        const names = stringList(child)
        if (names !== undefined) {
          out['propertyOrdering'] = names
        }
        break
      }
      case 'enum': {
        if (Array.isArray(child) && child.length > 0) {
          out['enum'] = child.map(enumMember)
        }
        break
      }
      case 'format': {
        if (
          typeof child === 'string' &&
          SUPPORTED_FORMATS.has(child.trim().toLowerCase())
        ) {
          out['format'] = child
        }
        break
      }
      case 'nullable': {
        if (child === true) {
          out['nullable'] = true
        }
        break
      }
      case 'default':
      case 'example': {
        out[key] = child
        break
      }
      // `type` and `const` are resolved after the loop (they interact with
      // nullability and with enum). Every other keyword — `$schema`, `$id`,
      // `$defs`, `definitions`, `additionalProperties`, `patternProperties`,
      // `propertyNames`, `unevaluated*`, `exclusiveMinimum`, `exclusiveMaximum`,
      // `multipleOf`, `uniqueItems`, `contains`, `if`/`then`/`else`, `not`,
      // `dependent*`, `examples`, `readOnly`, `writeOnly`, `deprecated`,
      // `contentEncoding`, ... — is outside the Google subset and is dropped.
      default:
        break
    }
  }

  // --- `type` (including array-valued and "null" forms) ---
  const rawType = value['type']
  const declaredTypes =
    typeof rawType === 'string'
      ? [rawType]
      : Array.isArray(rawType)
        ? rawType.filter((entry): entry is string => typeof entry === 'string')
        : []
  const concreteTypes = declaredTypes.filter(
    entry => entry.trim().toLowerCase() !== 'null',
  )
  if (concreteTypes.length !== declaredTypes.length) {
    // `["string", "null"]` / `"null"` -> OpenAPI 3.0 `nullable: true`.
    out['nullable'] = true
  }
  if (concreteTypes.length === 1) {
    out['type'] = concreteTypes[0]
  } else if (concreteTypes.length > 1) {
    // A genuine union type becomes an `anyOf` of single-type schemas.
    const pushed = pushAnyOfConstraint(
      { allOf, anyOf },
      concreteTypes.map(entry => ({ type: entry })),
    )
    anyOf = pushed.anyOf
    allOf = pushed.allOf
  }

  // --- `const` -> single-member `enum` (the subset has no `const`) ---
  const constValue = value['const']
  if (constValue !== undefined && out['enum'] === undefined) {
    out['enum'] = [enumMember(constValue)]
    if (out['type'] === undefined && typeof constValue === 'string') {
      out['type'] = 'string'
    }
  }

  // --- `oneOf` -> `anyOf` (the subset has no `oneOf`) ---
  if (oneOf !== undefined) {
    const pushed = pushAnyOfConstraint({ allOf, anyOf }, oneOf)
    anyOf = pushed.anyOf
    allOf = pushed.allOf
  }

  if (anyOf !== undefined) {
    out['anyOf'] = anyOf
  }
  if (allOf !== undefined) {
    out['allOf'] = allOf
  }

  // --- infer a missing `type` from unambiguous siblings ---
  if (
    out['type'] === undefined &&
    out['anyOf'] === undefined &&
    out['allOf'] === undefined
  ) {
    if (out['properties'] !== undefined || out['required'] !== undefined) {
      out['type'] = 'object'
    } else if (out['items'] !== undefined) {
      out['type'] = 'array'
    } else if (
      out['enum'] !== undefined ||
      typeof out['description'] === 'string'
    ) {
      out['type'] = 'string'
    }
  }

  return collapseNullableOnlyAnyOf(out)
}

// Sanitize an OpenAI-style `function.parameters` JSON Schema into the Google
// `FunctionDeclaration.parameters` subset. Root-level `$defs` / `definitions`
// are consumed as the inlining source and never emitted.
export const sanitizeGoogleToolSchema = (schema: unknown): unknown => {
  if (!isRecord(schema)) {
    return schema
  }
  const defs = schema['$defs']
  const legacyDefs = schema['definitions']
  const context: RefContext = {
    defs: isRecord(defs) ? defs : undefined,
    legacyDefs: isRecord(legacyDefs) ? legacyDefs : undefined,
  }
  return sanitizeSchemaNode(schema, context, [], 0) ?? {}
}

// ---------------------------------------------------------------------------
// Finish-reason normalization
// ---------------------------------------------------------------------------

// Google reports its own `FinishReason` enum ("STOP", "MAX_TOKENS", "SAFETY",
// ...). Our gateway speaks the OpenAI Chat Completions wire contract, whose
// `finish_reason` vocabulary is `stop` | `length` | `tool_calls` |
// `content_filter`. Emitting the raw Google enum breaks stock OpenAI clients, so
// both Google lanes normalize on EVERY path (complete, buffered stream, and the
// incremental SSE pass-through).
//
// `sawToolCalls` upgrades a plain `stop` to `tool_calls` — Google reports "STOP"
// even when the turn ended in a `functionCall`. A truncation (`MAX_TOKENS`) or a
// safety stop keeps its own, more informative reason.
export const normalizeGoogleFinishReason = (
  raw: string | undefined,
  sawToolCalls = false,
): string => {
  const value = (raw ?? '').trim()
  const base = ((): string => {
    if (value === '') {
      return 'stop'
    }
    switch (value.toUpperCase()) {
      case 'LENGTH':
      case 'MAX_TOKENS':
        return 'length'
      case 'BLOCKLIST':
      case 'CONTENT_FILTER':
      case 'IMAGE_SAFETY':
      case 'PROHIBITED_CONTENT':
      case 'RECITATION':
      case 'SAFETY':
      case 'SPII':
        return 'content_filter'
      case 'FUNCTION_CALL':
      case 'TOOL_CALLS':
        return 'tool_calls'
      // "STOP", "FINISH_REASON_UNSPECIFIED", "OTHER",
      // "MALFORMED_FUNCTION_CALL", and any future enum member normalize to the
      // safe default rather than leaking a raw provider token onto the wire.
      default:
        return 'stop'
    }
  })()
  return sawToolCalls && base === 'stop' ? 'tool_calls' : base
}
