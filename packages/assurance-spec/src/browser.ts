/**
 * Browser-safe AssuranceSpec surface.
 *
 * Repository inspection and proposal generation deliberately stay on the
 * package root because they require Node. Editors and other renderers only
 * need the document grammar, parser, serializer, and adequacy assessment.
 */
export * from "./graph.ts"
export * from "./parser.ts"
export * from "./schema.ts"
export * from "./serializer.ts"
export * from "./validator.ts"
