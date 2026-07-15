import { Schema } from 'effect'

const DocsSidebarSchema = Schema.Struct({
  label: Schema.optionalKey(Schema.String),
  order: Schema.Number,
})

export const DocsFrontmatterSchema = Schema.Struct({
  title: Schema.String,
  description: Schema.String,
  lastModified: Schema.String,
  sidebar: DocsSidebarSchema,
})

export const decodeDocsFrontmatter = Schema.decodeUnknownSync(
  DocsFrontmatterSchema,
)

export type DocsHeading = Readonly<{
  depth: 2 | 3
  id: string
  text: string
}>

export type DocsPageLink = Readonly<{
  path: string
  title: string
}>

export type DocsPageManifestEntry = Readonly<{
  description: string
  editUrl: string
  group: string
  headings: ReadonlyArray<DocsHeading>
  lastModified: string
  next?: DocsPageLink
  path: string
  previous?: DocsPageLink
  rawMarkdownUrl: string
  sidebarLabel: string
  slug: string
  title: string
}>

export type DocsPage = DocsPageManifestEntry &
  Readonly<{
    html: string
  }>

export type DocsNavigationGroupDefinition = Readonly<{
  collapsed: boolean
  label: string
  slugs: ReadonlyArray<string>
}>

export type DocsSearchRecord = Readonly<{
  body: string
  description: string
  headings: string
  id: string
  path: string
  title: string
}>

const DocsSearchRecordSchema = Schema.Struct({
  body: Schema.String,
  description: Schema.String,
  headings: Schema.String,
  id: Schema.String,
  path: Schema.String,
  title: Schema.String,
})

export const decodeDocsSearchIndex = Schema.decodeUnknownSync(
  Schema.Array(DocsSearchRecordSchema),
)
