import type { ProtocolManifest } from "./parity.ts";

const key = (member: ProtocolManifest["members"][number]) => `${member.direction}:${member.method}`;

export function renderProtocolDiff(before: ProtocolManifest, after: ProtocolManifest): string {
  const beforeByKey = new Map(before.members.map((member) => [key(member), member]));
  const afterByKey = new Map(after.members.map((member) => [key(member), member]));
  const lines: Array<string> = [];
  for (const name of [...afterByKey.keys()].filter((name) => !beforeByKey.has(name)).toSorted()) {
    lines.push(`+ ${name}`);
  }
  for (const name of [...beforeByKey.keys()].filter((name) => !afterByKey.has(name)).toSorted()) {
    lines.push(`- ${name}`);
  }
  for (const name of [...beforeByKey.keys()].filter((name) => afterByKey.has(name)).toSorted()) {
    if (JSON.stringify(beforeByKey.get(name)) !== JSON.stringify(afterByKey.get(name))) {
      lines.push(`~ ${name}`);
    }
  }
  return lines.length === 0 ? "No protocol drift." : lines.join("\n");
}
