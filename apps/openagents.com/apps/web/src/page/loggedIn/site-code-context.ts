import type { SiteElementContext } from './site-element-context'

export type SiteCodeViewerContext = Readonly<{
  path: string
  versionRef: string
  language: string
  source: string
}>

export type SiteCodeViewerContextInput = Readonly<{
  path: string
  versionRef: string
  language: string
  source: string
}>

const MAX_PATH_LENGTH = 160
const MAX_VERSION_REF_LENGTH = 180
const MAX_LANGUAGE_LENGTH = 40
const MAX_SOURCE_LENGTH = 1200
const secretShapedPattern =
  /\b(?:api[_-]?key|bearer|client[_-]?secret|password|provider[_-]?account|secret|token)\b/i

const compact = (value: string, maxLength: number): string =>
  value.replace(/\s+/g, ' ').trim().slice(0, maxLength).trim()

export const safeSiteCodeViewerContext = (
  input: SiteCodeViewerContextInput,
): SiteCodeViewerContext | null => {
  const path = compact(input.path, MAX_PATH_LENGTH)
  const versionRef = compact(input.versionRef, MAX_VERSION_REF_LENGTH)
  const language = compact(input.language.toLowerCase(), MAX_LANGUAGE_LENGTH)
  const source = input.source.trim().slice(0, MAX_SOURCE_LENGTH)

  if (
    path === '' ||
    versionRef === '' ||
    language === '' ||
    source === '' ||
    secretShapedPattern.test(path) ||
    secretShapedPattern.test(versionRef) ||
    secretShapedPattern.test(source)
  ) {
    return null
  }

  return {
    path,
    versionRef,
    language,
    source,
  }
}

export const siteCodeViewerContextFromElement = (
  context: SiteElementContext,
  versionRef: string,
): SiteCodeViewerContext | null =>
  safeSiteCodeViewerContext({
    language: 'html',
    path: `selected-element/${context.tag}.html`,
    source: context.htmlSnippet,
    versionRef,
  })
