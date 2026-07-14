import type { ESTree } from "@oxlint/plugins"

export type AstNode = ESTree.Node

export const nodeOf = (value: unknown): AstNode | undefined =>
  typeof value === "object" && value !== null && "type" in value && typeof value.type === "string"
    ? value as AstNode
    : undefined

export const unwrap = (value: unknown): AstNode | undefined => {
  let node = nodeOf(value)
  while (
    node?.type === "ChainExpression" ||
    node?.type === "ParenthesizedExpression" ||
    node?.type === "TSNonNullExpression" ||
    node?.type === "TSAsExpression" ||
    node?.type === "TSTypeAssertion"
  ) {
    node = nodeOf(node.expression)
  }
  return node
}

export const propertyName = (value: unknown): string | undefined => {
  const node = unwrap(value)
  if (node?.type === "Identifier" || node?.type === "PrivateIdentifier") return node.name
  return node?.type === "Literal" && typeof node.value === "string" ? node.value : undefined
}

export const identifierIs = (value: unknown, name: string): boolean => {
  const node = unwrap(value)
  return node?.type === "Identifier" && node.name === name
}

export const importSource = (node: unknown): string | undefined => {
  const value = nodeOf(node)
  return value?.type === "ImportDeclaration" && typeof value.source.value === "string"
    ? value.source.value
    : undefined
}
