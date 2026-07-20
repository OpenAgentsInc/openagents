import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, normalize } from "node:path";

import { compareStrings } from "./schema.ts";

/**
 * AR-4 (issue #9060): drift oracles for the repository's own documented claims.
 *
 * The repository's governing documents assert file paths, commands, and states
 * that nothing checks — the unverified-operational-directive failure class
 * applied to the codebase itself. These oracles give those claims a compiler.
 * All execution is SIDE-EFFECT-FREE: only fs existence, root package.json
 * scripts, and the committed live-roadmap snapshot are read. No command is
 * ever executed; no live production is probed.
 */

export type DriftClaimKind = "path" | "command" | "link";
export type DriftVerdict = "ok" | "broken" | "unverifiable";

export type DriftFinding = {
  readonly file: string;
  readonly line: number;
  readonly kind: DriftClaimKind;
  readonly claim: string;
  readonly verdict: DriftVerdict;
  readonly detail: string;
};

const lineOf = (text: string, index: number): number => text.slice(0, index).split("\n").length;

// A backtick span shaped like a repo-relative path: segments joined by `/`,
// no spaces/globs/urls, at least one slash.
const PATH_SPAN = /`([A-Za-z0-9._@-]+(?:\/[A-Za-z0-9._@-]+)+)`/g;
const HAS_EXTENSION = /\.[A-Za-z0-9]+$/;
const PNPM_RUN = /`?\bpnpm run ([a-z0-9:_-]+)`?/g;
// An inline markdown link `[text](target)`. The target is captured; reference
// definitions (`[ref]: …`) and reference links (`[text][ref]`) are not matched.
const MARKDOWN_LINK = /\[[^\]]*\]\(([^)\s]+)\)/g;

/**
 * Resolve a markdown-link target that points inside the repository, relative to
 * the linking file's own directory (root-anchored when it starts with `/`).
 * Returns the repo-relative resolved path, or null when the target is external
 * or not a repo-relative file reference (protocol URLs, mail, pure anchors, or
 * bare words with neither a slash nor an extension).
 */
const resolveLinkTarget = (file: string, rawTarget: string): string | null => {
  if (/^(?:[a-z][a-z0-9+.-]*:|#|\/\/)/i.test(rawTarget)) return null;
  const target = rawTarget.split("#")[0]!.split("?")[0]!;
  if (!target) return null;
  if (!target.includes("/") && !HAS_EXTENSION.test(target)) return null;
  return target.startsWith("/")
    ? normalize(target.slice(1))
    : normalize(join(dirname(file), target));
};

/**
 * Whether a path is gitignored. A gitignored path (e.g. a `.secrets/*.env`
 * file) is legitimately absent in a fresh worktree or CI, so its non-existence
 * is environment-specific, not a documentation error. Side-effect-free
 * (`git check-ignore -q` only reports).
 */
const isGitIgnored = (root: string, path: string): boolean => {
  try {
    execFileSync("git", ["-C", root, "check-ignore", "-q", "--", path], { stdio: "pipe" });
    return true;
  } catch {
    return false;
  }
};

/** Repo-root package.json scripts (side-effect-free read). */
const rootScripts = (root: string): ReadonlyArray<string> => {
  try {
    const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8")) as {
      scripts?: Record<string, string>;
    };
    return Object.keys(pkg.scripts ?? {});
  } catch {
    return [];
  }
};

/**
 * Check one governed document's checkable claims. `scripts` and `issueNumbers`
 * are passed in so a batch run reads them once.
 */
// npm package specifier: `@scope/name` or `name@1.2.3`, not a repo path.
const NPM_SPECIFIER = /^@|@\d/;

// GitHub organizations these docs reference as `owner/repo` slugs. There is no
// such top-level directory in the repository, so a span like `OpenAgentsInc/cloud`
// is unambiguously a GitHub reference, not a context-relative repo path.
const GITHUB_ORG_SLUGS = new Set(["OpenAgentsInc", "AtlantisPleb"]);

/**
 * A backtick span that is a well-known non-path reference rather than a
 * repo-relative file path: a git ref (`origin/main`, `upstream/foo`, `HEAD/…`)
 * or a GitHub `owner/repo` slug for a known organization. These are excluded
 * from path drift entirely rather than recorded as context-relative
 * unverifiables, which is a false classification for them.
 */
const isNonPathReference = (claim: string): boolean => {
  const first = claim.split("/")[0]!;
  if (first === "origin" || first === "upstream" || first === "HEAD") return true;
  const segments = claim.split("/");
  return segments.length === 2 && GITHUB_ORG_SLUGS.has(first);
};

export const checkDocumentClaims = (
  root: string,
  file: string,
  source: string,
  scripts: ReadonlyArray<string>,
  topLevel: ReadonlySet<string>,
): ReadonlyArray<DriftFinding> => {
  const findings: DriftFinding[] = [];

  for (const match of source.matchAll(PATH_SPAN)) {
    const claim = match[1]!;
    if (claim.startsWith("http") || claim.includes("*") || NPM_SPECIFIER.test(claim)) continue;
    if (isNonPathReference(claim)) continue;
    const line = lineOf(source, match.index);
    if (existsSync(join(root, claim))) continue;
    const firstSegment = claim.split("/")[0]!;
    if (!topLevel.has(firstSegment)) {
      // First segment is not a real top-level repo entry: the path is written
      // relative to some section's context, not the repo root. Not checkable
      // as a repo-root claim, so record as unverifiable rather than broken.
      findings.push({
        file,
        line,
        kind: "path",
        claim,
        verdict: "unverifiable",
        detail: "context-relative path (first segment is not a repo-root entry)",
      });
    } else if (isGitIgnored(root, claim)) {
      // Gitignored path (e.g. a secret file): legitimately absent in a fresh
      // worktree or CI, so its non-existence is environment-specific.
      findings.push({
        file,
        line,
        kind: "path",
        claim,
        verdict: "unverifiable",
        detail: "gitignored path (absence is environment-specific, not a doc error)",
      });
    } else if (HAS_EXTENSION.test(claim)) {
      findings.push({
        file,
        line,
        kind: "path",
        claim,
        verdict: "broken",
        detail: "referenced file does not exist",
      });
    } else {
      findings.push({
        file,
        line,
        kind: "path",
        claim,
        verdict: "unverifiable",
        detail: "directory-shaped path under a real root not found; may be conceptual",
      });
    }
  }

  for (const match of source.matchAll(PNPM_RUN)) {
    const claim = `pnpm run ${match[1]!}`;
    const line = lineOf(source, match.index);
    if (scripts.includes(match[1]!)) continue;
    findings.push({
      file,
      line,
      kind: "command",
      claim,
      verdict: "broken",
      detail: "root package.json has no such script",
    });
  }

  for (const match of source.matchAll(MARKDOWN_LINK)) {
    const rawTarget = match[1]!;
    const resolved = resolveLinkTarget(file, rawTarget);
    if (resolved === null) continue;
    const line = lineOf(source, match.index);
    if (existsSync(join(root, resolved))) continue;
    if (HAS_EXTENSION.test(resolved)) {
      findings.push({
        file,
        line,
        kind: "link",
        claim: rawTarget,
        verdict: "broken",
        detail: `linked file does not exist (resolves to ${resolved})`,
      });
    } else {
      findings.push({
        file,
        line,
        kind: "link",
        claim: rawTarget,
        verdict: "unverifiable",
        detail: `directory-shaped link target not found (resolves to ${resolved}); may be conceptual`,
      });
    }
  }

  return findings.sort((a, b) =>
    compareStrings(
      `${a.line.toString().padStart(6, "0")}:${a.kind}:${a.claim}`,
      `${b.line.toString().padStart(6, "0")}:${b.kind}:${b.claim}`,
    ),
  );
};

export type DriftReport = {
  readonly governedDocuments: ReadonlyArray<string>;
  readonly findings: ReadonlyArray<DriftFinding>;
  readonly summary: {
    readonly broken: number;
    readonly brokenUndispositioned: number;
    readonly dispositioned: number;
    readonly unverifiable: number;
    readonly documentsChecked: number;
  };
};

/** Stable disposition key for a finding: `<file>:<claim>` (line-independent). */
export const driftDispositionKey = (finding: Pick<DriftFinding, "file" | "claim">): string =>
  `${finding.file}:${finding.claim}`;

const isFile = (root: string, path: string): boolean => {
  try {
    return statSync(join(root, path)).isFile();
  } catch {
    return false;
  }
};

/** Run drift oracles over a governed-document set. */
const topLevelEntries = (root: string): ReadonlySet<string> => {
  try {
    return new Set(readdirSync(root));
  } catch {
    return new Set();
  }
};

export const runDriftOracles = (
  root: string,
  governedDocuments: ReadonlyArray<string>,
  dispositions: Record<string, string> = {},
): DriftReport => {
  const scripts = rootScripts(root);
  const topLevel = topLevelEntries(root);
  const docs = [...governedDocuments].filter((path) => isFile(root, path)).sort(compareStrings);
  const findings: DriftFinding[] = [];
  for (const file of docs) {
    findings.push(
      ...checkDocumentClaims(root, file, readFileSync(join(root, file), "utf8"), scripts, topLevel),
    );
  }
  const brokenFindings = findings.filter((f) => f.verdict === "broken");
  const dispositioned = brokenFindings.filter(
    (f) => dispositions[driftDispositionKey(f)] !== undefined,
  ).length;
  const unverifiable = findings.filter((f) => f.verdict === "unverifiable").length;
  return {
    governedDocuments: docs,
    findings,
    summary: {
      broken: brokenFindings.length,
      brokenUndispositioned: brokenFindings.length - dispositioned,
      dispositioned,
      unverifiable,
      documentsChecked: docs.length,
    },
  };
};
