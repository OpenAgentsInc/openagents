import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { discoverSkills, loadSkillSelection, standingContext } from "../src/coder-skills.js";
import { skillTool } from "../src/coder-tools.js";

/** A repository with the given skills under `.agents/skills`. */
const workspace = (skills: Record<string, string>): string => {
  const root = mkdtempSync(join(tmpdir(), "coder-skills-"));
  for (const [name, source] of Object.entries(skills)) {
    const directory = join(root, ".agents", "skills", name);
    mkdirSync(directory, { recursive: true });
    writeFileSync(join(directory, "SKILL.md"), source);
  }
  return root;
};

/** A home directory with no skills in it, so a test reads only its workspace. */
const EMPTY_HOME = mkdtempSync(join(tmpdir(), "coder-skills-home-"));

/** No packaged skills either, so a test reads only what it made. */
const NO_BUILT_INS = mkdtempSync(join(tmpdir(), "coder-skills-builtin-"));

const SKILL = `---
name: house-style
description: How this repository writes prose.
---

# House style

Use sentence case.
`;

describe("discovering skills", () => {
  it("reads the name, the description, and the body without its front matter", () => {
    const [skill] = discoverSkills(workspace({ "house-style": SKILL }), EMPTY_HOME, NO_BUILT_INS);

    expect(skill).toMatchObject({
      name: "house-style",
      description: "How this repository writes prose.",
    });
    expect(skill?.body).toBe("# House style\n\nUse sentence case.");
    // The body is what the front matter is not, so neither leaks into the other.
    expect(skill?.body).not.toContain("description:");
  });

  it("takes the name from the front matter, not from the directory", () => {
    const [skill] = discoverSkills(workspace({ "some-folder": SKILL }), EMPTY_HOME, NO_BUILT_INS);

    expect(skill?.name).toBe("house-style");
  });

  it("skips a directory with no SKILL.md rather than failing", () => {
    const root = workspace({ "house-style": SKILL });
    mkdirSync(join(root, ".agents", "skills", "empty"), { recursive: true });

    expect(discoverSkills(root, EMPTY_HOME, NO_BUILT_INS).map((skill) => skill.name)).toEqual(["house-style"]);
  });

  it("skips a skill missing a name or a description", () => {
    const root = workspace({
      nameless: "---\ndescription: No name.\n---\n\nBody.",
      quiet: "---\nname: quiet\n---\n\nBody.",
      "house-style": SKILL,
    });

    // One cannot be asked for and the other gives nothing to choose on.
    expect(discoverSkills(root, EMPTY_HOME, NO_BUILT_INS).map((skill) => skill.name)).toEqual(["house-style"]);
  });


  it("reads a description written as a folded block, not the block marker", () => {
    const root = workspace({
      folded: [
        "---",
        "name: folded",
        "description: >-",
        "  Use when the reader wants one sentence",
        "  spread over two lines.",
        "---",
        "",
        "Body.",
      ].join("\n"),
    });

    // Taking `>-` as the value is how a skill came to describe itself as ">-".
    expect(discoverSkills(root, EMPTY_HOME, NO_BUILT_INS)[0]?.description).toBe(
      "Use when the reader wants one sentence spread over two lines.",
    );
  });

  it("keeps the line breaks of a literal block", () => {
    const root = workspace({
      literal: ["---", "name: literal", "description: |", "  One.", "  Two.", "---", "", "Body."].join(
        "\n",
      ),
    });

    expect(discoverSkills(root, EMPTY_HOME, NO_BUILT_INS)[0]?.description).toBe("One.\nTwo.");
  });

  it("is empty for a repository with no skills directory", () => {
    expect(discoverSkills(mkdtempSync(join(tmpdir(), "coder-skills-")), EMPTY_HOME, NO_BUILT_INS)).toEqual([]);
  });

  it("strips the quotes from a quoted description", () => {
    const root = workspace({
      quoted: '---\nname: quoted\ndescription: "Quoted, with a comma."\n---\n\nBody.',
    });

    expect(discoverSkills(root, EMPTY_HOME, NO_BUILT_INS)[0]?.description).toBe("Quoted, with a comma.");
  });
});

describe("the skill tool", () => {
  const skills = discoverSkills(workspace({ "house-style": SKILL }), EMPTY_HOME, NO_BUILT_INS);
  const tool = skillTool(skills);

  it("carries the catalog in its description, so both lanes read it", () => {
    // The local lane writes its own prompt and the thread lane's is the
    // server's. A tool description is the one place that reaches both.
    expect(tool.description).toContain("`house-style`: How this repository writes prose.");
    // The body is not in the catalog. That is the point of the catalog.
    expect(tool.description).not.toContain("Use sentence case.");
  });

  it("offers only the names it found, so a model cannot ask for another", () => {
    expect((tool.parameters["properties"] as Record<string, { enum: string[] }>)["name"]?.enum).toEqual([
      "house-style",
    ]);
  });

  it("hands back the body when asked", async () => {
    const output = await tool.run({ name: "house-style" }, new AbortController().signal);

    expect(output).toContain("Use sentence case.");
    expect(output).toContain("house-style");
  });

  it("answers an unknown name with what it does have, rather than throwing", async () => {
    const output = await tool.run({ name: "nope" }, new AbortController().signal);

    // A model that misremembers can correct itself; it cannot correct a turn
    // that died.
    expect(output).toContain("There is no `nope` skill");
    expect(output).toContain("`house-style`");
  });
});

describe("choosing which skills the model is offered", () => {
  /** A home the test owns, so the choice is written where it can be read back. */
  const home = () => mkdtempSync(join(tmpdir(), "coder-skills-home-"));

  const two = {
    alpha: "---\nname: alpha\ndescription: First.\n---\n\nBody.",
    beta: "---\nname: beta\ndescription: Second.\n---\n\nBody.",
  };

  it("offers every skill until one is switched off", () => {
    const selection = loadSkillSelection(workspace(two), home(), NO_BUILT_INS);

    expect(selection.active().map((skill) => skill.name)).toEqual(["alpha", "beta"]);
    expect(selection.isOn("alpha")).toBe(true);
  });

  it("drops a switched-off skill from what the model is offered", () => {
    const selection = loadSkillSelection(workspace(two), home(), NO_BUILT_INS);

    selection.toggle("alpha");

    expect(selection.isOn("alpha")).toBe(false);
    expect(selection.active().map((skill) => skill.name)).toEqual(["beta"]);
    // Still found, so the screen can offer to switch it back on.
    expect(selection.all).toHaveLength(2);
  });

  it("remembers the choice for the next session", () => {
    const root = workspace(two);
    const where = home();

    loadSkillSelection(root, where, NO_BUILT_INS).toggle("beta");

    expect(loadSkillSelection(root, where, NO_BUILT_INS).active().map((skill) => skill.name)).toEqual(["alpha"]);
  });

  it("switches one back on", () => {
    const root = workspace(two);
    const where = home();

    loadSkillSelection(root, where, NO_BUILT_INS).toggle("beta");
    loadSkillSelection(root, where, NO_BUILT_INS).toggle("beta");

    expect(loadSkillSelection(root, where, NO_BUILT_INS).active().map((skill) => skill.name)).toEqual([
      "alpha",
      "beta",
    ]);
  });

  it("keeps the choice to the workspace it was made in", () => {
    const where = home();
    const one = workspace(two);
    const other = workspace(two);

    loadSkillSelection(one, where, NO_BUILT_INS).toggle("alpha");

    // A skill switched off for one repository is not switched off everywhere.
    expect(loadSkillSelection(other, where, NO_BUILT_INS).active().map((skill) => skill.name)).toEqual([
      "alpha",
      "beta",
    ]);
  });

  it("offers a skill added after the choice was made", () => {
    const root = workspace(two);
    const where = home();
    loadSkillSelection(root, where, NO_BUILT_INS).toggle("alpha");

    mkdirSync(join(root, ".agents", "skills", "gamma"), { recursive: true });
    writeFileSync(
      join(root, ".agents", "skills", "gamma", "SKILL.md"),
      "---\nname: gamma\ndescription: Third.\n---\n\nBody.",
    );

    // Off is what is recorded, so something nobody has ruled on is on.
    expect(loadSkillSelection(root, where, NO_BUILT_INS).active().map((skill) => skill.name)).toEqual([
      "beta",
      "gamma",
    ]);
  });
});

describe("the skills the CLI ships with", () => {
  it("offers its own, so a session knows the CLI it is part of", () => {
    const found = discoverSkills(mkdtempSync(join(tmpdir(), "coder-skills-")), EMPTY_HOME);

    expect(found.map((skill) => skill.name)).toContain("openagents-cli");
  });

  it("lets a repository replace one by name, with nothing to uninstall", () => {
    const root = workspace({
      "openagents-cli": "---\nname: openagents-cli\ndescription: Ours.\n---\n\nOur version.",
    });

    const found = discoverSkills(root, EMPTY_HOME);
    const ours = found.find((skill) => skill.name === "openagents-cli");

    // Nearest wins, and the packaged one is furthest.
    expect(ours?.description).toBe("Ours.");
    expect(found.filter((skill) => skill.name === "openagents-cli")).toHaveLength(1);
  });

  it("says what works with no credential, which help output does not", () => {
    const found = discoverSkills(mkdtempSync(join(tmpdir(), "coder-skills-")), EMPTY_HOME);
    const cli = found.find((skill) => skill.name === "openagents-cli");

    // The auth boundary is the part a model cannot discover by asking --help.
    expect(cli?.body).toContain("What works with no credential");
    expect(cli?.body).toContain("chat:account");
    expect(cli?.body).toContain("forge:write");
    expect(cli?.body).toContain("auth login --headless");
  });
});

describe("what a session is told without asking", () => {
  const AUTO = "---\nname: method\ndescription: How to work.\nauto: true\n---\n\nWork this way.";
  const NORMAL = "---\nname: other\ndescription: Something else.\n---\n\nRead me on request.";

  it("marks a skill that loads itself", () => {
    const found = discoverSkills(workspace({ method: AUTO, other: NORMAL }), EMPTY_HOME, NO_BUILT_INS);

    expect(found.find((skill) => skill.name === "method")?.auto).toBe(true);
    expect(found.find((skill) => skill.name === "other")?.auto).toBe(false);
  });

  it("carries an auto-loaded body and leaves the rest in the catalog", () => {
    const found = discoverSkills(workspace({ method: AUTO, other: NORMAL }), EMPTY_HOME, NO_BUILT_INS);

    const standing = standingContext(found, "/somewhere/else") ?? "";
    expect(standing).toContain("Work this way.");
    // The whole point of the catalog is that a body is read when it is wanted.
    expect(standing).not.toContain("Read me on request.");
  });

  it("says nothing when nothing loads itself", () => {
    const found = discoverSkills(workspace({ other: NORMAL }), EMPTY_HOME, NO_BUILT_INS);

    expect(standingContext(found, "/somewhere/else")).toBeUndefined();
  });

  it("tells a session inside OpenAgents which repository is which", () => {
    // Both were worked out from scratch, repeatedly, by sessions that had no
    // way to know: one is Phoenix, the other holds the CLI.
    const standing = standingContext([], "/Users/x/work/openagents.com") ?? "";

    expect(standing).toContain("Phoenix");
    expect(standing).toContain("packages/openagents-cli");
  });

  it("says nothing about repositories anywhere else", () => {
    expect(standingContext([], "/Users/x/work/something")).toBeUndefined();
  });
});

describe("finding the other OpenAgents repository", () => {
  const workspace = () => {
    const root = mkdtempSync(join(tmpdir(), "oa-work-"));
    for (const name of ["openagents", "openagents.com"]) {
      mkdirSync(join(root, name, ".git"), { recursive: true });
    }
    return root;
  };

  it("says where the sibling is, and how to reach it", () => {
    // The gap this closes: a session was told the two repositories exist and
    // not where the other one was. It guessed the path right, ran
    // `git grep <pattern> ../openagents`, and git refused it for being outside
    // the repository — then fell back to grepping the workspace root, which
    // holds every read-only reference clone, and spent the tool's whole
    // 120-second budget before being stopped.
    const root = workspace();
    const standing = standingContext([], join(root, "openagents.com")) ?? "";

    expect(standing).toContain(join(root, "openagents"));
    expect(standing).toContain("cd ");
    expect(standing).toContain("outside the repository");
  });

  it("warns off the workspace root by name", () => {
    const root = workspace();
    const standing = standingContext([], join(root, "openagents")) ?? "";

    expect(standing).toContain(join(root, "openagents.com"));
    expect(standing).toContain(`Do not search \`${root}\``);
  });

  it("says nothing about a sibling that is not checked out", () => {
    // Worse than no instruction: one telling the reader to `cd` somewhere that
    // does not exist.
    const root = mkdtempSync(join(tmpdir(), "oa-lone-"));
    mkdirSync(join(root, "openagents.com", ".git"), { recursive: true });

    const standing = standingContext([], join(root, "openagents.com")) ?? "";

    expect(standing).toContain("Two repositories carry");
    expect(standing).not.toContain("cd ");
    expect(standing).not.toContain("Do not search");
  });

  it("says nothing about a sibling for a directory that is neither", () => {
    const root = workspace();
    mkdirSync(join(root, "openagents-notes"), { recursive: true });

    const standing = standingContext([], join(root, "openagents-notes")) ?? "";

    expect(standing).toContain("Two repositories carry");
    expect(standing).not.toContain("cd ");
  });
});
