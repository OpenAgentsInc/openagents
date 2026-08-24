/**
 * The skills a session can read.
 *
 * A skill is a directory holding a `SKILL.md`: YAML front matter naming it and
 * saying when it applies, then a body of instructions. The format is shared
 * with the other agents that read this repository, so the same file serves all
 * of them and none of them owns it.
 *
 * Skills are offered by name and description, not by body. Eight skills across
 * this machine and this repository are some 46 KB, more than a local model's
 * context should spend on instructions it will not use, and the total grows
 * with every skill anyone adds. So the catalog is small and constant, and a
 * body is read only when the model asks for one -- which is what the front
 * matter's `description` is for: it is the sentence the model chooses on.
 *
 * The catalog rides in the `skill` tool's description rather than in a system
 * prompt. The local lane composes its own prompt; the thread lane's is the
 * server's. A tool description reaches both.
 */

import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Where skills live, nearest first. The first to claim a name keeps it.
 *
 * The CLI ships its own last, so a repository or a person can replace one by
 * writing a skill of the same name and nothing has to be uninstalled. They are
 * ordinary skills otherwise: they appear in the catalog, and `/skills` switches
 * them off like any other.
 */
const SKILL_DIRECTORIES = (
  cwd: string,
  home: string,
  builtIn: string,
): ReadonlyArray<string> => [
  join(cwd, ".agents", "skills"),
  join(home, ".agents", "skills"),
  builtIn,
];

/** The skills packaged with this CLI, beside the compiled output. */
const builtInSkills = (): string =>
  join(dirname(fileURLToPath(import.meta.url)), "..", "skills");

/** How much of one skill body is handed back. */
const BODY_LIMIT = 32_000;

export interface CoderSkill {
  /** The name the model asks for, from the front matter. */
  readonly name: string;
  /** When to use it, from the front matter. One sentence, shown in the catalog. */
  readonly description: string;
  /** The instructions, front matter removed. */
  readonly body: string;
  /** Where it was read from, so a reader can open it. */
  readonly path: string;
}

/**
 * Read `name` and `description` out of YAML front matter.
 *
 * Deliberately not a YAML parser. These are two bounded scalar fields at the
 * top of a known file, and a dependency that can parse anchors and merge keys
 * is a dependency that can also do something surprising with a file anyone may
 * drop in a skills directory.
 */
const frontMatter = (source: string): { name?: string; description?: string } => {
  if (!source.startsWith("---")) return {};
  const end = source.indexOf("\n---", 3);
  if (end < 0) return {};

  const fields: { name?: string; description?: string } = {};
  const lines = source.slice(3, end).split("\n");

  for (const [at, line] of lines.entries()) {
    const match = /^(name|description):\s*(.*)$/.exec(line);
    if (match === null) continue;
    const key = match[1] as "name" | "description";
    const inline = match[2]!.trim();

    // `>` and `|` say the value is the indented block beneath, which is how a
    // description longer than a line is written. Taking the marker as the value
    // is how `stripe-directory` came to describe itself as ">-".
    if (inline === "" || /^[>|][-+]?$/.test(inline)) {
      const block: string[] = [];
      for (const next of lines.slice(at + 1)) {
        if (!/^\s/.test(next) || next.trim() === "") break;
        block.push(next.trim());
      }
      // A folded block is one paragraph; a literal one keeps its line breaks.
      if (block.length > 0) fields[key] = block.join(inline.startsWith("|") ? "\n" : " ");
      continue;
    }

    // A quoted scalar is the same string without its quotes.
    fields[key] = inline.replace(/^["'](.*)["']$/, "$1");
  }
  return fields;
};

/** The body after the front matter, or the whole file when there is none. */
const withoutFrontMatter = (source: string): string => {
  if (!source.startsWith("---")) return source.trim();
  const end = source.indexOf("\n---", 3);
  if (end < 0) return source.trim();
  return source.slice(source.indexOf("\n", end + 1) + 1).trim();
};

/**
 * Every skill readable from this directory, nearest source winning its name.
 *
 * A directory that is missing, unreadable, or holds no `SKILL.md` contributes
 * nothing. A skills directory is optional, and a session whose repository has
 * none is a session with no skills, not a session that failed to start.
 */
export function discoverSkills(
  cwd: string = process.cwd(),
  // Taken rather than read so a test can point at a directory it made. A
  // function that always reads the real home directory can only be tested on a
  // machine that happens to have the right skills in it, and the same goes for
  // the directory this package ships.
  home: string = homedir(),
  builtIn: string = builtInSkills(),
): ReadonlyArray<CoderSkill> {
  const found = new Map<string, CoderSkill>();

  for (const directory of SKILL_DIRECTORIES(cwd, home, builtIn)) {
    let entries: ReadonlyArray<string>;
    try {
      entries = readdirSync(directory);
    } catch {
      continue;
    }

    for (const entry of entries) {
      const path = join(directory, entry, "SKILL.md");
      let source: string;
      try {
        if (!statSync(path).isFile()) continue;
        source = readFileSync(path, "utf8");
      } catch {
        continue;
      }

      const { name, description } = frontMatter(source);
      // A skill with no name cannot be asked for, and one with no description
      // gives the model nothing to choose on. Both are required.
      if (name === undefined || description === undefined) continue;
      if (found.has(name)) continue;

      found.set(name, { name, description, body: withoutFrontMatter(source), path });
    }
  }

  // Sorting a fresh array built from the map, so nothing shared is mutated.
  // eslint-disable-next-line unicorn/no-array-sort -- the spread is the copy
  return [...found.values()].sort((left, right) => left.name.localeCompare(right.name));
}

/** The catalog line for one skill, as the model reads it. */
export const catalogEntry = (skill: CoderSkill): string =>
  `- \`${skill.name}\`: ${skill.description}`;

/** What a skill hands back when it is read. */
export const renderSkill = (skill: CoderSkill): string => {
  const body =
    skill.body.length > BODY_LIMIT
      ? `${skill.body.slice(0, BODY_LIMIT)}\n\n[truncated; the rest is in ${skill.path}]`
      : skill.body;
  return `Skill \`${skill.name}\` (${skill.path}):\n\n${body}`;
};

/**
 * Which skills this workspace offers the model.
 *
 * A skill costs context whether or not it is used: its description sits in the
 * `skill` tool for the whole session. A reader who knows a skill is irrelevant
 * to today's work should be able to take it out, and have it stay out.
 *
 * The choice is stored per workspace, keyed by path, because the reason to
 * switch a skill off is usually the repository rather than the machine. It is
 * stored under the config directory rather than in the repository so that
 * switching one off is not a change someone else has to review.
 *
 * Off is recorded, not on: a skill added later is on until someone says
 * otherwise, which is the behaviour of a session that has never been touched.
 */
export interface SkillSelection {
  /** Every skill found, switched on or off. */
  readonly all: ReadonlyArray<CoderSkill>;
  /** Whether this skill is offered to the model. */
  isOn(name: string): boolean;
  /** Switch one skill, persist the choice, and return its new state. */
  toggle(name: string): boolean;
  /** The skills the model is offered, in catalog order. */
  active(): ReadonlyArray<CoderSkill>;
}

const selectionPath = (home: string): string =>
  join(home, ".config", "openagents", "coder-skills.json");

/** Names switched off for this workspace, or none when nothing is recorded. */
const readDisabled = (path: string, workspace: string): ReadonlySet<string> => {
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as unknown;
    if (typeof parsed !== "object" || parsed === null) return new Set();
    const entry = (parsed as Record<string, unknown>)[workspace];
    if (!Array.isArray(entry)) return new Set();
    return new Set(entry.filter((name): name is string => typeof name === "string"));
  } catch {
    // No file, unreadable, or not the shape this writes. A preference nobody
    // can read is a preference nobody set, and the session opens with every
    // skill on rather than refusing to start.
    return new Set();
  }
};

const writeDisabled = (path: string, workspace: string, disabled: ReadonlySet<string>): void => {
  let all: Record<string, unknown> = {};
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as unknown;
    if (typeof parsed === "object" && parsed !== null) all = parsed as Record<string, unknown>;
  } catch {
    // Start a fresh file rather than lose the choice being made now.
  }

  // An empty list is the default, so it is removed rather than recorded. A file
  // of empty arrays is a file that says nothing.
  if (disabled.size === 0) delete all[workspace];
  else all[workspace] = [...disabled].sort();

  try {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, `${JSON.stringify(all, undefined, 2)}\n`, "utf8");
  } catch {
    // The choice still holds for this session. A preference that cannot be
    // written is not a reason to refuse the preference.
  }
};

/** Read the skills for this workspace and the choice made about them. */
export function loadSkillSelection(
  cwd: string = process.cwd(),
  home: string = homedir(),
  builtIn?: string,
): SkillSelection {
  const all = discoverSkills(cwd, home, builtIn);
  const path = selectionPath(home);
  const disabled = new Set(readDisabled(path, cwd));

  return {
    all,
    isOn: (name) => !disabled.has(name),
    toggle: (name) => {
      const on = disabled.has(name);
      if (on) disabled.delete(name);
      else disabled.add(name);
      writeDisabled(path, cwd, disabled);
      return on;
    },
    active: () => all.filter((skill) => !disabled.has(skill.name)),
  };
}
