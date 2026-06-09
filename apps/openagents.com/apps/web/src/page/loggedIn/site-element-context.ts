import { Schema as S } from 'effect'

export const SiteElementAttribute = S.Struct({
  name: S.String,
  value: S.String,
})
export type SiteElementAttribute = typeof SiteElementAttribute.Type

export const SiteElementContext = S.Struct({
  selector: S.String,
  tag: S.String,
  text: S.NullOr(S.String),
  attributes: S.Array(SiteElementAttribute),
  htmlSnippet: S.String,
})
export type SiteElementContext = typeof SiteElementContext.Type

export type SiteElementContextInput = Readonly<{
  selector: string
  tag: string
  text?: string | null
  attributes?: ReadonlyArray<Readonly<{ name: string; value: string }>>
}>

const MAX_SELECTOR_LENGTH = 160
const MAX_TEXT_LENGTH = 180
const MAX_ATTRIBUTE_VALUE_LENGTH = 120
const MAX_SNIPPET_LENGTH = 260
const safeAttributeNames = new Set([
  'aria-label',
  'class',
  'href',
  'id',
  'name',
  'role',
  'title',
])
const secretShapedPattern =
  /\b(?:api[_-]?key|bearer|client[_-]?secret|password|provider[_-]?account|secret|token)\b/i

const compactText = (value: string, maxLength: number): string =>
  value.replace(/\s+/g, ' ').trim().slice(0, maxLength).trim()

const safeText = (value: string | null | undefined, maxLength: number) => {
  if (value === null || value === undefined) {
    return null
  }

  const compacted = compactText(value, maxLength)

  return compacted === '' || secretShapedPattern.test(compacted)
    ? null
    : compacted
}

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('"', '&quot;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')

const safeHref = (value: string): boolean =>
  value.startsWith('#') ||
  value.startsWith('/') ||
  value.startsWith('https://') ||
  value.startsWith('http://')

const safeAttributes = (
  attributes: SiteElementContextInput['attributes'],
): ReadonlyArray<SiteElementAttribute> =>
  (attributes ?? [])
    .map(attribute => ({
      name: attribute.name.toLowerCase().trim(),
      value: compactText(attribute.value, MAX_ATTRIBUTE_VALUE_LENGTH),
    }))
    .filter(attribute => {
      if (!safeAttributeNames.has(attribute.name) || attribute.value === '') {
        return false
      }

      if (secretShapedPattern.test(attribute.value)) {
        return false
      }

      return attribute.name !== 'href' || safeHref(attribute.value)
    })
    .slice(0, 6)

const buildSnippet = (
  tag: string,
  attributes: ReadonlyArray<SiteElementAttribute>,
  text: string | null,
): string => {
  const attrs = attributes
    .map(attribute => `${attribute.name}="${escapeHtml(attribute.value)}"`)
    .join(' ')
  const open = attrs === '' ? `<${tag}>` : `<${tag} ${attrs}>`
  const snippet = `${open}${escapeHtml(text ?? '')}</${tag}>`

  return snippet.length <= MAX_SNIPPET_LENGTH
    ? snippet
    : `${snippet.slice(0, MAX_SNIPPET_LENGTH - 4)}...</${tag}>`
}

export const safeSiteElementContext = (
  input: SiteElementContextInput,
): SiteElementContext | null => {
  const tag = input.tag.toLowerCase().trim()

  if (!/^[a-z][a-z0-9-]{0,31}$/.test(tag)) {
    return null
  }

  const selector = safeText(input.selector, MAX_SELECTOR_LENGTH)

  if (selector === null) {
    return null
  }

  const text = safeText(input.text, MAX_TEXT_LENGTH)
  const attributes = safeAttributes(input.attributes)

  return {
    selector,
    tag,
    text,
    attributes,
    htmlSnippet: buildSnippet(tag, attributes, text),
  }
}

export const siteElementContextDraft = (
  context: SiteElementContext,
): string =>
  [
    `Target element: ${context.htmlSnippet}`,
    `Selector: ${context.selector}`,
    'Requested change: ',
  ].join('\n')
