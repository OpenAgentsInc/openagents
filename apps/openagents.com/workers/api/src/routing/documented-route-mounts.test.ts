/**
 * The guard for #9306: a path this repository documents or promises must be
 * one the Worker actually serves.
 *
 * #9306 shipped a live `yellow` promise whose stated verification method was
 * `GET /api/public/free-tier-data-sharing` while the route answered 404,
 * because its handler was exported and imported by nothing. Nothing in the
 * test suite could tell, so it was found by accident.
 *
 * Each ledger below is asserted by EQUALITY, not by containment. A newly
 * orphaned path fails, and a ledger entry that has since been fixed or removed
 * also fails. A silencing list that only ever grows is how the original defect
 * survived: the free-tier handler sat in
 * `scripts/uncalled-production-symbol-baseline.json` the whole time.
 */
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { Effect } from 'effect'
import { describe, expect, test } from 'vite-plus/test'

import { exactRoutePathManifest } from '../index'
import { isRetiredMoneySurfaceRequest } from '../money-surface-retirement'
import { openAgentsCapabilityManifest } from '../openagents-capability-manifest'
import { openAgentsOpenApiDocument } from '../openagents-openapi'
import { publicProductPromisesDocument } from '../product-promises'
import { gatewayLegacyPathname } from '../worker-routes'

/**
 * ## Resolver
 *
 * Resolve whether a documented or promised path is actually served by the
 * Worker.
 *
 * #9306 shipped a live `yellow` product promise whose stated verification
 * method was `GET /api/public/free-tier-data-sharing`. The handler was
 * exported, the OpenAPI document declared the path, and nothing imported the
 * module — so the route answered 404 in production while the promise claimed
 * it was checkable. It was found by accident during unrelated work.
 *
 * An earlier prototype compared the OpenAPI document against path literals in
 * `index.ts` and flagged 29 of 71 public paths, including
 * `/api/public/product-promises`, which is live. That route is registered
 * through the imported `PublicProductPromisesEndpoint` constant rather than a
 * literal, so the naive comparison could not see it. A guard with false
 * positives gets ignored, which is how the original blindness survived, so
 * this resolver is built to be believed rather than to be exhaustive.
 *
 * WHAT COUNTS AS MOUNTED
 *   1. The path is in the live exact-route manifest. That manifest is the
 *      registry the production dispatcher is built from, read at runtime, so
 *      constant-referenced paths and the retired-capability filter are both
 *      already applied to it.
 *   2. A module reachable from the Worker entry point by static import
 *      declares a matching path predicate. Reachability is the discriminator
 *      that catches #9306: an exported handler in a module nobody imports can
 *      never answer a request, however complete the module looks.
 *
 * The four predicate idioms this reads are the four the Worker actually uses:
 * a pathname equality, a `pathname.startsWith` prefix, an anchored route
 * regex, and a path constant or `:param` template resolved across the import
 * that carries it. When a path resolves through none of them the resolver says
 * so and names nothing, because "unresolved" is the honest word for a static
 * reader that found no evidence.
 *
 * A missed mount (a route this reader cannot see) makes the guard silent, not
 * wrong. That is the failure direction to prefer.
 */


type RouteMountEvidence = Readonly<{
  kind:
    | 'exact-route-table'
    | 'pathname-equality'
    | 'pathname-prefix'
    | 'route-regex'
    | 'path-template'
    | 'retired-money-surface'
  detail: string
  source?: string
}>

type DocumentedRouteResolver = Readonly<{
  reachableModuleCount: number
  predicateCount: number
  /** Evidence that the path is served, or `undefined` when none was found. */
  resolve: (documentedPath: string) => RouteMountEvidence | undefined
}>

const SOURCE_EXTENSIONS = ['.ts', '.tsx'] as const

const resolveRelativeImport = (
  fromFile: string,
  specifier: string,
): string | undefined => {
  if (!specifier.startsWith('.')) return undefined
  const base = resolve(dirname(fromFile), specifier)
  for (const extension of SOURCE_EXTENSIONS) {
    const candidate = `${base}${extension}`
    if (existsSync(candidate)) return candidate
  }
  for (const extension of SOURCE_EXTENSIONS) {
    const candidate = join(base, `index${extension}`)
    if (existsSync(candidate)) return candidate
  }
  return undefined
}

const IMPORT_SPECIFIER_PATTERNS: ReadonlyArray<RegExp> = [
  /(?:^|\n)\s*(?:import|export)[\s\S]{0,4000}?from\s+['"]([^'"]+)['"]/g,
  /(?:^|\n)\s*import\s+['"]([^'"]+)['"]/g,
  /import\(\s*['"]([^'"]+)['"]\s*\)/g,
]

const collectReachableModules = (entry: string): ReadonlyArray<string> => {
  const reachable = new Set<string>()
  const pending = [entry]
  while (pending.length > 0) {
    const file = pending.pop()
    if (file === undefined || reachable.has(file)) continue
    reachable.add(file)
    const source = readFileSync(file, 'utf8')
    for (const pattern of IMPORT_SPECIFIER_PATTERNS) {
      pattern.lastIndex = 0
      for (const match of source.matchAll(pattern)) {
        const target = resolveRelativeImport(file, match[1] ?? '')
        if (
          target !== undefined &&
          !target.includes(`${'node_modules'}/`) &&
          !/\.test\.tsx?$/.test(target)
        ) {
          pending.push(target)
        }
      }
    }
  }
  return [...reachable]
}

/**
 * Strip comments before reading predicates. Route modules carry JSDoc blocks
 * that list the paths they serve, and a documented path must not be able to
 * prove itself mounted from a comment.
 */
const stripComments = (source: string): string =>
  source
    .replace(/\/\*[\s\S]*?\*\//g, ' ')
    .split('\n')
    .map(line => line.replace(/(^|[^:'"`\\])\/\/.*$/, '$1'))
    .join('\n')

/**
 * Scan one line for anchored route regex literals.
 *
 * A character-class-aware scan is required: every path-param pattern in this
 * Worker is written `([^/]+)`, and the bare `/` inside that class ends a naive
 * regex-literal match three characters early.
 */
const scanRouteRegexLiterals = (line: string): ReadonlyArray<string> => {
  const found: Array<string> = []
  for (let index = 0; index < line.length - 3; index += 1) {
    if (
      line[index] !== '/' ||
      line[index + 1] !== '^' ||
      line[index + 2] !== '\\' ||
      line[index + 3] !== '/'
    ) {
      continue
    }
    let cursor = index + 1
    let inCharacterClass = false
    let closingSlash = -1
    for (; cursor < line.length; cursor += 1) {
      const character = line[cursor]
      if (character === '\\') {
        cursor += 1
        continue
      }
      if (inCharacterClass) {
        if (character === ']') inCharacterClass = false
        continue
      }
      if (character === '[') {
        inCharacterClass = true
        continue
      }
      if (character === '/') {
        closingSlash = cursor
        break
      }
    }
    if (closingSlash === -1) continue
    const body = line.slice(index + 1, closingSlash)
    // A route pattern is anchored at both ends and starts with a literal
    // segment character. `/^\/+/` (a leading-slash trim) fails both tests.
    if (!body.endsWith('$')) continue
    if (!/^\^\\\/[A-Za-z0-9]/.test(body)) continue
    found.push(body)
    index = closingSlash
  }
  return found
}

const PATH_LITERAL = /^\/[A-Za-z0-9][A-Za-z0-9._\-/:{}]*$/
const ROUTE_KEY_NAME = /(?:^|[a-z])(?:path|route|endpoint|url)s?$/i

const CONST_PATH_DECLARATION =
  /(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=\n]*)?=\s*['"`](\/[^'"`\s]*)['"`]/g
const OBJECT_PATH_PROPERTY = /([A-Za-z_$][\w$]*)\s*:\s*['"`](\/[^'"`\s]*)['"`]/g
const PATHNAME_EQUALITY =
  /pathname\s*(?:===|!==)\s*['"`](\/[^'"`]*)['"`]|['"`](\/[^'"`]*)['"`]\s*(?:===|!==)\s*[\w.]*pathname/g
const PATHNAME_PREFIX = /pathname\s*\.startsWith\(\s*['"`](\/[^'"`]*)['"`]/g
const IMPORTED_BINDING =
  /(?:^|\n)\s*import\s+(?:type\s+)?\{([^}]*)\}\s*from\s*['"][^'"]*['"]/g

/**
 * Split a route regex body into segments so a documented template can be
 * compared structurally.
 *
 * Substituting a sample value into `{contextKind}` and testing the result
 * against `^\/api\/forum\/contexts\/(site|workroom)\/([^\/]+)\/activity$` fails
 * for every sample that is not one of the two enum members, which reads a live
 * route as missing. Comparing segment shapes does not have that problem.
 */
const routeRegexSegments = (body: string): ReadonlyArray<string> | undefined => {
  const inner = body.slice(1, body.length - 1)
  if (!inner.startsWith('\\/')) return undefined
  const segments: Array<string> = []
  let current = ''
  let depth = 0
  let inCharacterClass = false
  for (let index = 2; index < inner.length; index += 1) {
    const character = inner[index]
    if (character === '\\') {
      if (inner[index + 1] === '/' && depth === 0 && !inCharacterClass) {
        segments.push(current)
        current = ''
        index += 1
        continue
      }
      current += character + (inner[index + 1] ?? '')
      index += 1
      continue
    }
    if (inCharacterClass) {
      if (character === ']') inCharacterClass = false
      current += character
      continue
    }
    if (character === '[') {
      inCharacterClass = true
      current += character
      continue
    }
    if (character === '(') depth += 1
    if (character === ')') depth -= 1
    current += character
  }
  segments.push(current)
  return segments
}

/**
 * Expand optional trailing segment groups into concrete variants.
 *
 * `^\/api\/agents\/claims\/([^\/]+)(?:\/(approve|reject))?$` serves three
 * documented paths at two different segment counts. Segment-shape comparison
 * needs each length as its own pattern.
 */
const expandOptionalSegmentGroups = (body: string): ReadonlyArray<string> => {
  const optional = /\(\?:\\\/[^()]*(?:\([^()]*\))?[^()]*\)\?/
  const match = optional.exec(body)
  if (match === null) return [body]
  const withGroup = body.slice(0, match.index) + match[0].slice(3, -2) + body.slice(match.index + match[0].length)
  const withoutGroup = body.slice(0, match.index) + body.slice(match.index + match[0].length)
  return [
    ...expandOptionalSegmentGroups(withGroup),
    ...expandOptionalSegmentGroups(withoutGroup),
  ]
}

const isLiteralRouteSegment = (segment: string): boolean =>
  /^[A-Za-z0-9._-]+$/.test(segment.replace(/\\(.)/g, '$1'))

const unescapeRouteSegment = (segment: string): string =>
  segment.replace(/\\(.)/g, '$1')

const isDocumentedParameterSegment = (segment: string): boolean =>
  /^\{.+\}$/.test(segment) || segment.startsWith(':')

/**
 * A documented path matches a route pattern when they have the same segment
 * count, every literal segment is identical, and every documented parameter
 * segment lines up with a non-literal (capturing or wildcard) route segment.
 */
const segmentsMatch = (
  documentedSegments: ReadonlyArray<string>,
  routeSegments: ReadonlyArray<string>,
): boolean => {
  if (documentedSegments.length !== routeSegments.length) return false
  return documentedSegments.every((documented, index) => {
    const route = routeSegments[index] ?? ''
    if (isDocumentedParameterSegment(documented)) {
      return !isLiteralRouteSegment(route) || route.includes('|')
    }
    if (isLiteralRouteSegment(route)) return unescapeRouteSegment(route) === documented
    // A documented literal may still be served by a pattern segment, e.g. an
    // enum alternation. Accept only when the alternation names it.
    return route.includes(documented)
  })
}

const templateSegments = (template: string): ReadonlyArray<string> =>
  template.replace(/^\//, '').split('/')

const templateMatches = (
  documentedSegments: ReadonlyArray<string>,
  template: string,
): boolean => {
  const segments = templateSegments(template)
  if (segments.length !== documentedSegments.length) return false
  return documentedSegments.every((documented, index) => {
    const candidate = segments[index] ?? ''
    if (isDocumentedParameterSegment(candidate)) return true
    if (isDocumentedParameterSegment(documented)) return false
    return candidate === documented
  })
}

type DocumentedRouteResolverOptions = Readonly<{
  entryFile: string
  /**
   * The live exact-route manifest (`exactRoutePathManifest`). Passed in rather
   * than imported so this module stays a pure reader and the caller supplies
   * runtime truth.
   */
  exactRoutePaths: ReadonlyArray<string>
  repoRoot: string
  /**
   * The canonical `/api` gateway alias rewrite the dispatcher applies before
   * matching (`gatewayLegacyPathname`).
   */
  legacyPathname: (pathname: string) => string | undefined
  /**
   * The retirement interception the entry point applies ahead of the route
   * table (`isRetiredMoneySurfaceRequest`). A retired money surface answers a
   * typed 410 rather than falling through, so it is served, not missing.
   */
  isRetiredSurface: (pathname: string) => boolean
}>

const makeDocumentedRouteResolver = (
  options: DocumentedRouteResolverOptions,
): DocumentedRouteResolver => {
  const reachable = collectReachableModules(options.entryFile)
  const exact = new Set(options.exactRoutePaths)

  const equalities = new Map<string, string>()
  const prefixes = new Map<string, string>()
  const routeRegexes: Array<{
    segments: ReadonlyArray<string>
    body: string
    compiled: RegExp | undefined
    file: string
  }> = []
  const templates = new Map<string, string>()

  // Path constants are frequently declared in a module that never touches a
  // pathname and consumed by the router that does, so collect declarations
  // everywhere and admit one only when a pathname-handling module imports its
  // binding.
  const declaredPaths = new Map<string, { literal: string; file: string }>()
  const pathnameModuleBindings = new Set<string>()

  for (const file of reachable) {
    const source = stripComments(readFileSync(file, 'utf8'))
    const handlesPathname = source.includes('pathname')

    for (const match of source.matchAll(CONST_PATH_DECLARATION)) {
      const [, identifier, literal] = match
      if (identifier === undefined || literal === undefined) continue
      if (!PATH_LITERAL.test(literal)) continue
      declaredPaths.set(identifier, { file, literal })
      if (handlesPathname) templates.set(literal, file)
    }

    for (const match of source.matchAll(OBJECT_PATH_PROPERTY)) {
      const [, key, literal] = match
      if (key === undefined || literal === undefined) continue
      if (!ROUTE_KEY_NAME.test(key) || !PATH_LITERAL.test(literal)) continue
      declaredPaths.set(key, { file, literal })
      if (handlesPathname) templates.set(literal, file)
    }

    if (!handlesPathname) continue

    for (const match of source.matchAll(PATHNAME_EQUALITY)) {
      const literal = match[1] ?? match[2]
      if (literal !== undefined) equalities.set(literal, file)
    }
    for (const match of source.matchAll(PATHNAME_PREFIX)) {
      const literal = match[1]
      if (literal !== undefined) prefixes.set(literal, file)
    }
    for (const line of source.split('\n')) {
      for (const body of scanRouteRegexLiterals(line)) {
        let compiled: RegExp | undefined
        try {
          compiled = new RegExp(body)
        } catch {
          compiled = undefined
        }
        for (const variant of expandOptionalSegmentGroups(body)) {
          const segments = routeRegexSegments(variant)
          if (segments !== undefined) {
            routeRegexes.push({ body, compiled, file, segments })
          }
        }
      }
    }
    for (const match of source.matchAll(IMPORTED_BINDING)) {
      for (const binding of (match[1] ?? '').split(',')) {
        const name = binding.split(/\s+as\s+/).pop()?.trim()
        if (name !== undefined && name !== '') pathnameModuleBindings.add(name)
      }
    }
    // A property access such as `PublicAgentProposalRecoveryRoute.previewPath`
    // reaches a declared path without importing that member by name.
    for (const match of source.matchAll(/\.([A-Za-z_$][\w$]*)/g)) {
      const name = match[1]
      if (name !== undefined) pathnameModuleBindings.add(name)
    }
  }

  for (const [identifier, declaration] of declaredPaths) {
    if (pathnameModuleBindings.has(identifier)) {
      templates.set(declaration.literal, declaration.file)
    }
  }

  // A constant that ends in `/` is a `startsWith` prefix, not a whole path.
  // Receipt readers all declare one (`/api/public/cloud/receipts/`) and slice
  // the ref off the tail, so reading it as a complete path finds no mount.
  for (const [template, file] of [...templates]) {
    if (template.endsWith('/')) {
      templates.delete(template)
      prefixes.set(template, file)
    }
  }

  const source = (file: string) => relative(options.repoRoot, file)

  const resolveDocumentedPath = (documentedPath: string): RouteMountEvidence | undefined => {
    // A promise may cite a family rather than one path
    // (`route:/api/training/leaderboards/*`). It is answered when any served
    // path sits under the prefix.
    if (documentedPath.endsWith('/*')) {
      const prefix = documentedPath.slice(0, -1)
      const served = [...exact, ...equalities.keys(), ...templates.keys()].find(
        path => path.startsWith(prefix) && path.length > prefix.length,
      )
      return served === undefined
        ? undefined
        : { detail: served, kind: 'pathname-equality' }
    }

    const candidates = [documentedPath]
    const legacy = options.legacyPathname(documentedPath)
    if (legacy !== undefined) candidates.push(legacy)

    for (const candidate of candidates) {
      if (exact.has(candidate)) {
        return { detail: candidate, kind: 'exact-route-table' }
      }
      if (options.isRetiredSurface(candidate.replace(/\{[^}]+\}/g, 'x'))) {
        return { detail: candidate, kind: 'retired-money-surface' }
      }
      const equalityFile = equalities.get(candidate)
      if (equalityFile !== undefined) {
        return { detail: candidate, kind: 'pathname-equality', source: source(equalityFile) }
      }
      const documentedSegments = templateSegments(candidate)
      for (const [prefix, file] of prefixes) {
        // A one-segment prefix such as `/api/` is the dispatcher's terminal
        // fallback, not a mount, so it may never resolve a documented path.
        if (prefix.split('/').filter(Boolean).length < 2) continue
        if (candidate.startsWith(prefix)) {
          return { detail: prefix, kind: 'pathname-prefix', source: source(file) }
        }
      }
      for (const [template, file] of templates) {
        if (templateMatches(documentedSegments, template)) {
          return { detail: template, kind: 'path-template', source: source(file) }
        }
      }
      // A documented path with no parameters is a concrete request line, so
      // test it directly. Segment-shape comparison is only needed for a
      // template, whose `{param}` cannot be substituted with any single value
      // that satisfies an enum alternation.
      const isConcrete = !/[{:]/.test(candidate)
      for (const regex of routeRegexes) {
        const matched = isConcrete
          ? regex.compiled?.test(candidate) === true
          : segmentsMatch(documentedSegments, regex.segments)
        if (matched) {
          return { detail: regex.body, kind: 'route-regex', source: source(regex.file) }
        }
      }
    }
    return undefined
  }

  return {
    predicateCount: equalities.size + prefixes.size + routeRegexes.length + templates.size,
    reachableModuleCount: reachable.length,
    resolve: resolveDocumentedPath,
  }
}

const repoRoot = resolve(import.meta.dirname, '../../../../../..')
const workerEntry = join(
  repoRoot,
  'apps/openagents.com/workers/api/src/index.ts',
)

const resolver = makeDocumentedRouteResolver({
  entryFile: workerEntry,
  exactRoutePaths: exactRoutePathManifest,
  isRetiredSurface: pathname => isRetiredMoneySurfaceRequest('GET', pathname),
  legacyPathname: gatewayLegacyPathname,
  repoRoot,
})

/**
 * Paths the served OpenAPI document declares that no handler answers. Empty,
 * and it must stay that way: the eight the guard found on 2026-08-03 were all
 * routes the VP-1 retirement wave deleted on 2026-07-14 without retracting
 * their documentation, and their declarations were removed rather than
 * silenced. See `docs/refactor/2026-08-03-orphaned-public-route-audit.md`.
 */
const unservedDocumentedPaths: ReadonlyArray<string> = []

/**
 * `route:` evidence refs on promises that are still live (any state except
 * `withdrawn`) and name a path nothing serves. These are promise-registry
 * decisions for the owner, not code defects this guard may fix, so they are
 * pinned here to stop the set from growing quietly.
 */
const unservedPromiseRouteRefs: ReadonlyArray<string> = [
  'data.free_tier_capture_disclosure.v1 -> /api/keys/free',
  'inference.khala_free_openai_compatible_api.v1 -> /api/keys/free',
  'metrics.khala_tokens_served_public.v1 -> /api/public/khala-token-history',
  'pylon.install_without_wallet_knowledge.v1 -> /api/public/nexus-pylon/receipts/{receiptRef}',
]

/**
 * `routeExact` compares a path to `url.pathname` with `===`, so an exact-route
 * entry containing a `:param` segment answers only a request for that literal
 * text. Four of these are deliberate tombstones whose handler is a bare
 * `notFound()`. The fifth,
 * `/api/public/inference/privacy-receipts/:receiptRef`, is not: it carries a
 * real handler that no live receipt ref can ever reach (#9307).
 */
const parameterShapedExactRoutes: ReadonlyArray<string> = [
  '/api/public/inference/privacy-receipts/:receiptRef',
  '/api/public/khala-code/outside-user-runs/:receiptRef',
  '/api/public/khala-code/trace-plugin-revenue-share-precedents/:receiptRef',
  '/api/public/qa-swarm/first-engagements/:receiptRef',
  '/api/public/revenue-loop/first-dollar-evidence/:bundleRef',
]

const servedOpenApiPaths = (): ReadonlyArray<string> =>
  Object.keys(
    Effect.runSync(openAgentsOpenApiDocument()).paths as Record<
      string,
      unknown
    >,
  )

const normalizeRouteRef = (raw: string): string => raw.replace(/[.,)]+$/, '')

/**
 * The capability manifest writes one entry for a pair of sibling routes as
 * `/api/forum/{topics|posts}/{targetId}/reports`. Expand the alternation so
 * both real paths are checked.
 */
const expandAlternations = (path: string): ReadonlyArray<string> => {
  const match = /\{([A-Za-z0-9-]+(?:\|[A-Za-z0-9-]+)+)\}/.exec(path)
  if (match === null) return [path]
  return (match[1] ?? '')
    .split('|')
    .flatMap(option =>
      expandAlternations(
        path.slice(0, match.index) +
          option +
          path.slice(match.index + match[0].length),
      ),
    )
}

/** Every API endpoint the live `/.well-known/openagents.json` advertises. */
const capabilityManifestApiPaths = (): ReadonlyArray<string> => {
  const advertised = new Set<string>()
  for (const match of JSON.stringify(
    Effect.runSync(openAgentsCapabilityManifest()),
  ).matchAll(/https:\/\/openagents\.com(\/api\/[A-Za-z0-9._\-{}|/:]*)/g)) {
    for (const path of expandAlternations(
      normalizeRouteRef(match[1] ?? ''),
    )) {
      advertised.add(path)
    }
  }
  return [...advertised]
}

const promiseRouteRefs = (): ReadonlyArray<
  Readonly<{ promiseId: string; ref: string; state: string }>
> => {
  const document = publicProductPromisesDocument() as unknown as Readonly<{
    promises: ReadonlyArray<Readonly<{ promiseId: string; state: string }>>
  }>
  const refs: Array<{ promiseId: string; ref: string; state: string }> = []
  const seen = new Set<string>()
  for (const promise of document.promises) {
    for (const match of JSON.stringify(promise).matchAll(
      /route:(\/[A-Za-z0-9._\-{}/:*]*)/g,
    )) {
      const ref = normalizeRouteRef(match[1] ?? '')
      const key = `${promise.promiseId}|${ref}`
      if (ref === '' || seen.has(key)) continue
      seen.add(key)
      refs.push({ promiseId: promise.promiseId, ref, state: promise.state })
    }
  }
  return refs
}

describe('documented route mounts', () => {
  test('resolves a route registered through an imported path constant', () => {
    // The earlier prototype flagged this live route because `index.ts`
    // registers it as `PublicProductPromisesEndpoint`, never as a literal.
    // That single false positive is why the prototype was not landed.
    expect(resolver.resolve('/api/public/product-promises')).toMatchObject({
      kind: 'exact-route-table',
    })
  })

  test('resolves the #9306 route now that it is mounted', () => {
    expect(
      resolver.resolve('/api/public/free-tier-data-sharing'),
    ).toMatchObject({ kind: 'exact-route-table' })
  })

  test('resolves a dynamic route whose pattern uses an enum segment', () => {
    // `^\/api\/forum\/contexts\/(site|workroom)\/([^/]+)\/activity$` cannot be
    // matched by substituting a sample value into `{contextKind}`.
    expect(
      resolver.resolve('/api/forum/contexts/{contextKind}/{contextId}/activity'),
    ).toMatchObject({ kind: 'route-regex' })
  })

  test('resolves a route whose path lives in a separate module constant', () => {
    expect(
      resolver.resolve('/api/public/business/case-studies'),
    ).toMatchObject({ kind: 'path-template' })
  })

  test('does not resolve a path nothing declares', () => {
    expect(
      resolver.resolve('/api/public/definitely-not-a-route-9306'),
    ).toBeUndefined()
  })

  test('reads a path predicate from code, never from a comment', () => {
    // `config.ts` mentions `POST /api/keys/free` in a comment and serves
    // nothing. A resolver that read comments would call it mounted.
    const configSource = readFileSync(
      join(repoRoot, 'apps/openagents.com/workers/api/src/config.ts'),
      'utf8',
    )
    expect(configSource).toContain('/api/keys/free')
    expect(resolver.resolve('/api/keys/free')).toBeUndefined()
  })

  test('every path the OpenAPI document serves is answered by a handler', () => {
    const unresolved = servedOpenApiPaths()
      .filter(path => resolver.resolve(path) === undefined)
      .sort()

    expect(unresolved).toEqual([...unservedDocumentedPaths].sort())
  })

  test('every endpoint the capability manifest advertises is answered', () => {
    // `/.well-known/openagents.json` is what an outside agent reads to learn
    // what this service can do, so a dead href there is a claim, not a note.
    const unresolved = capabilityManifestApiPaths()
      .filter(path => resolver.resolve(path) === undefined)
      .sort()

    expect(unresolved).toEqual([])
  })

  test('every live promise route ref is answered by a handler', () => {
    const unresolved = promiseRouteRefs()
      .filter(
        entry =>
          entry.state !== 'withdrawn' &&
          resolver.resolve(entry.ref) === undefined,
      )
      .map(entry => `${entry.promiseId} -> ${entry.ref}`)
      .sort()

    expect(unresolved).toEqual([...unservedPromiseRouteRefs].sort())
  })

  test('exact routes carrying a param-shaped segment stay enumerated', () => {
    const parameterShaped = exactRoutePathManifest
      .filter(path =>
        path
          .split('/')
          .some(segment => segment.startsWith(':') || /^\{.+\}$/.test(segment)),
      )
      .sort()

    expect(parameterShaped).toEqual([...parameterShapedExactRoutes].sort())
  })
})
