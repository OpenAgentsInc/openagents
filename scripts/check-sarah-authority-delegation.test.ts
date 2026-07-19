import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vite-plus/test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = readFileSync(path.join(repoRoot, "docs/authority/SARAH_AUTHORITY.md"), "utf8");

const frontmatter = /^---\n([\s\S]*?)\n---/.exec(source)?.[1] ?? "";
const readFrontmatter = (key: string): string => {
  const value = new RegExp(`^${key}:\\s+["']?([^"'\\n]+)["']?$`, "m").exec(frontmatter)?.[1];
  if (value === undefined) throw new Error(`missing ${key}`);
  return value.trim();
};

const readBlock = <A>(label: string): A => {
  const value = new RegExp("```" + label + "\\n([\\s\\S]*?)\\n```").exec(source)?.[1];
  if (value === undefined) throw new Error(`missing ${label}`);
  return JSON.parse(value) as A;
};

type Grant = Readonly<{
  id: string;
  actions: ReadonlyArray<string>;
  resources: ReadonlyArray<string>;
  condition_refs: ReadonlyArray<string>;
}>;

describe("Sarah authority revision 4", () => {
  test("composes with root revision 6 and preserves a closed sandbox grant", () => {
    expect(readFrontmatter("authority_revision")).toBe("4");
    expect(source).toContain("AUTHORITY.md_revision_6");

    const grants = readBlock<ReadonlyArray<Grant>>("authority-delegation-grants");
    const grant = grants.find(({ id }) => id === "grant.sarah.managed_sandbox");
    expect(grant?.resources).toEqual(["authenticated_owner_openagents_managed_sandboxes"]);
    expect(grant?.actions).toEqual([
      "create_managed_sandbox",
      "list_managed_sandboxes",
      "inspect_managed_sandbox",
      "dispatch_managed_sandbox_work",
      "interrupt_managed_sandbox_turn",
      "stop_managed_sandbox",
      "resume_managed_sandbox",
      "delete_managed_sandbox",
    ]);
    expect(grant?.actions).not.toContain("run_gcloud");
    expect(grant?.actions).not.toContain("start_full_auto_run");
    expect(grant?.condition_refs).toEqual(
      expect.arrayContaining([
        "condition.managed_sandbox_scope",
        "condition.managed_sandbox_budget",
        "condition.managed_sandbox_runtime_admission",
      ]),
    );
  });
});
