import type { BehaviorContractRegistryDocument } from "./contract"

const stateBadge = (state: string): string =>
  state === "enforced" ? "ENFORCED" : state.toUpperCase()

/**
 * Render the registry as a markdown section for the human contract doc.
 * The registry TypeScript module stays the machine source of truth; docs
 * embed this rendering so a doc/test drift check can pin them together.
 */
export const renderBehaviorContractMarkdown = (
  document: BehaviorContractRegistryDocument,
): string => {
  const lines: string[] = []
  lines.push(`Registry version: \`${document.version}\` (schema \`${document.schemaVersion}\`)`)
  lines.push("")
  for (const contract of document.contracts) {
    lines.push(`### \`${contract.contractId}\` — ${stateBadge(contract.state)}`)
    lines.push("")
    lines.push(`- **Surface:** ${contract.surface} (${contract.productArea})`)
    lines.push(
      `- **Stated by:** ${contract.source.statedBy} via ${contract.source.channel} on ${contract.source.statedOn}`,
    )
    lines.push(`- **Statement:** ${contract.statement}`)
    lines.push(`- **Enforcement tier:** ${contract.enforcementTier}`)
    for (const oracle of contract.oracles) {
      lines.push(
        `- **Oracle** \`${oracle.id}\` (${oracle.kind}, ${oracle.mode}): ${oracle.description} — \`${oracle.ref}\``,
      )
    }
    lines.push(`- **Verification:** ${contract.verification}`)
    if (contract.blockerRefs.length > 0) {
      lines.push(`- **Blockers:** ${contract.blockerRefs.map(ref => `\`${ref}\``).join(", ")}`)
    }
    if (contract.authorityBoundary !== undefined) {
      lines.push(`- **Authority boundary:** ${contract.authorityBoundary}`)
    }
    lines.push("")
  }
  return lines.join("\n")
}
