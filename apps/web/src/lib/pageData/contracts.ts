// Shared page data contracts between Effect atoms/loaders and UI templates.
// Keep this file UI-free so `src/effect/**` can depend on it without pulling in `effuse-pages/**`.

export type ModuleItem = {
  readonly moduleId: string
  readonly description: string
  readonly signatureIdsJson: string
}

export type ModulesPageData = {
  readonly errorText: string | null
  readonly sorted: ReadonlyArray<ModuleItem> | null
}

export type SignatureItem = {
  readonly signatureId: string
  readonly promptSummary: string
  readonly inputSchemaJson: string
  readonly outputSchemaJson: string
  readonly promptIrJson: string
  readonly defaultsJson: string
}

export type SignaturesPageData = {
  readonly errorText: string | null
  readonly sorted: ReadonlyArray<SignatureItem> | null
}

export type ToolItem = {
  readonly name: string
  readonly description: string
  readonly usage: string | null
  readonly inputSchemaJson: string
  readonly outputSchemaJson: string
}

export type ToolsPageData = {
  readonly errorText: string | null
  readonly sorted: ReadonlyArray<ToolItem> | null
}

