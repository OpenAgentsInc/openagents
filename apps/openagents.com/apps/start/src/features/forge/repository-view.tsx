import type {
  ForgeFileContent,
  ForgeRepositoryProjection,
  ForgeRepositoryReadFailure,
  ForgeRepositoryReadRequest,
  ForgeRepositoryReadResult,
} from "@/features/forge/repository-read";
import { PublicHeader } from "@/components/public-header";
import {
  AlertTriangle,
  Archive,
  Binary,
  BookOpen,
  Boxes,
  ChevronRight,
  CircleDot,
  Clock3,
  Code2,
  Copy,
  File,
  FileCode2,
  FileImage,
  Folder,
  GitBranch,
  GitCommitHorizontal,
  GitCompareArrows,
  LockKeyhole,
  Network,
  RefreshCw,
  Search,
  ShieldCheck,
  Tag,
  Users,
} from "lucide-react";
import { Marked, type RendererObject } from "marked";
import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";

import "./repository-view.css";

type ForgePageProps = Readonly<{
  request: ForgeRepositoryReadRequest;
  result: ForgeRepositoryReadResult;
}>;

const dateTime = (value: string): string => {
  const parsed = new Date(value);
  return Number.isNaN(parsed.valueOf())
    ? value
    : new Intl.DateTimeFormat("en", {
        dateStyle: "medium",
        timeStyle: "short",
        timeZone: "UTC",
      }).format(parsed);
};

const relativeTime = (value: string, now = Date.now()): string => {
  const elapsed = Math.max(0, now - Date.parse(value));
  if (!Number.isFinite(elapsed)) return "Time unavailable";
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 48) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
};

const bytes = (value: number): string => {
  if (value < 1_000) return `${value} B`;
  if (value < 1_000_000) return `${(value / 1_000).toFixed(1)} KB`;
  return `${(value / 1_000_000).toFixed(1)} MB`;
};

const refCountLabel = (branches: number, tags: number): string =>
  `${branches} ${branches === 1 ? "branch" : "branches"} · ${tags} ${tags === 1 ? "tag" : "tags"}`;

const encodeSearch = (
  projection: ForgeRepositoryProjection,
  updates: {
    readonly [Key in keyof ForgeRepositoryReadRequest]?:
      | ForgeRepositoryReadRequest[Key]
      | undefined;
  },
): string => {
  const owner = updates.owner ?? projection.repository.owner;
  const repo = updates.repo ?? projection.repository.name;
  const view = updates.view ?? "code";
  const ref =
    updates.ref === undefined && !("ref" in updates) ? projection.selectedRef : updates.ref;
  const search = new URLSearchParams();
  if (view !== "code") search.set("view", view);
  if (ref !== undefined) search.set("ref", ref);
  if (updates.path !== undefined && updates.path !== "") search.set("path", updates.path);
  if (updates.commit !== undefined) search.set("commit", updates.commit);
  if (updates.base !== undefined) search.set("base", updates.base);
  const query = search.toString();
  return `/forge/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}${
    query === "" ? "" : `?${query}`
  }`;
};

const safeExternalHref = (value: string): string | undefined => {
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:" ? url.href : undefined;
  } catch {
    return undefined;
  }
};

const escapeHtml = (value: string): string =>
  value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");

const safeMarkdownHtml = (
  markdown: string,
  projection: ForgeRepositoryProjection,
  sourcePath: string,
  assets: ReadonlyArray<Readonly<{ path: string; sourceUrl: string }>>,
): string => {
  const sourceDirectory = sourcePath.includes("/")
    ? sourcePath.slice(0, sourcePath.lastIndexOf("/") + 1)
    : "";
  const renderer: RendererObject = {
    html: ({ text }: { text: string }) => escapeHtml(text),
    link(token) {
      const external = safeExternalHref(token.href);
      const href =
        external ??
        (token.href.startsWith("#")
          ? token.href
          : encodeSearch(projection, {
              path: `${sourceDirectory}${token.href}`.replace(/^\.\//, ""),
            }));
      const target = external === undefined ? "" : ' rel="noreferrer" target="_blank"';
      return `<a href="${escapeHtml(href)}"${target}>${this.parser.parseInline(token.tokens)}</a>`;
    },
    image: (token) => {
      const path = `${sourceDirectory}${token.href}`.replace(/^\.\//, "");
      const asset = assets.find((candidate) => candidate.path === path);
      if (asset !== undefined) {
        return `<img alt="${escapeHtml(token.text)}" loading="lazy" src="${escapeHtml(asset.sourceUrl)}">`;
      }
      const external = safeExternalHref(token.href);
      const href = external ?? encodeSearch(projection, { path });
      return `<a class="forge-markdown-image-fallback" href="${escapeHtml(href)}"${
        external === undefined ? "" : ' rel="noreferrer" target="_blank"'
      }>[Image: ${escapeHtml(token.text || path)}]</a>`;
    },
  };
  return new Marked({ gfm: true, renderer }).parse(markdown, {
    async: false,
  }) as string;
};

const iconForEntry = (kind: "directory" | "file" | "symlink" | "submodule"): ReactNode =>
  kind === "directory" ? (
    <Folder aria-hidden="true" />
  ) : kind === "submodule" ? (
    <Boxes aria-hidden="true" />
  ) : (
    <File aria-hidden="true" />
  );

const parentPath = (path: string): string =>
  path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";

function RefMenu({ projection }: { projection: ForgeRepositoryProjection }) {
  const branches = projection.refs.filter((ref) => ref.kind === "branch");
  const tags = projection.refs.filter((ref) => ref.kind === "tag");
  return (
    <details className="forge-ref-menu">
      <summary>
        <GitBranch aria-hidden="true" />
        <span>{projection.selectedRef.replace(/^refs\/heads\//, "")}</span>
        <small>{refCountLabel(branches.length, tags.length)}</small>
      </summary>
      <div className="forge-ref-popover">
        <strong>Branches</strong>
        {branches.length === 0 ? (
          <p>No branches are available.</p>
        ) : (
          branches.map((ref) => (
            <a
              aria-current={ref.name === projection.selectedRef ? "page" : undefined}
              href={encodeSearch(projection, { ref: ref.name, path: "" })}
              key={ref.name}
            >
              <GitBranch aria-hidden="true" />
              <span>{ref.name.replace(/^refs\/heads\//, "")}</span>
              <code>{ref.objectId.slice(0, 8)}</code>
            </a>
          ))
        )}
        <strong>Tags</strong>
        {tags.length === 0 ? (
          <p>No tags are available.</p>
        ) : (
          tags.map((ref) => (
            <a
              aria-current={ref.name === projection.selectedRef ? "page" : undefined}
              href={encodeSearch(projection, { ref: ref.name, path: "" })}
              key={ref.name}
            >
              <Tag aria-hidden="true" />
              <span>{ref.name.replace(/^refs\/tags\//, "")}</span>
              <code>{ref.objectId.slice(0, 8)}</code>
            </a>
          ))
        )}
      </div>
    </details>
  );
}

function Tree({ projection }: { projection: ForgeRepositoryProjection }) {
  const [query, setQuery] = useState("");
  const inputId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const focusFilter = (event: KeyboardEvent) => {
      if (event.key !== "/" || event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement
      )
        return;
      if (inputRef.current?.offsetParent === null) return;
      event.preventDefault();
      inputRef.current?.focus();
    };
    window.addEventListener("keydown", focusFilter);
    return () => window.removeEventListener("keydown", focusFilter);
  }, []);
  const entries = [...projection.tree]
    .filter((entry) => entry.path.toLowerCase().includes(query.trim().toLowerCase()))
    .sort((left, right) => {
      if (left.kind === "directory" && right.kind !== "directory") return -1;
      if (left.kind !== "directory" && right.kind === "directory") return 1;
      return left.name.localeCompare(right.name);
    });
  return (
    <nav aria-label="Repository files" className="forge-tree">
      <div className="forge-tree-heading">
        <span>Files</span>
        <code>{projection.selectedPath === "" ? "/" : projection.selectedPath}</code>
      </div>
      <label className="forge-tree-search" htmlFor={inputId}>
        <Search aria-hidden="true" />
        <span className="sr-only">Filter repository files</span>
        <input
          autoComplete="off"
          id={inputId}
          onChange={(event) => setQuery(event.currentTarget.value)}
          placeholder="Filter files"
          ref={inputRef}
          type="search"
          value={query}
        />
        <kbd>/</kbd>
      </label>
      {projection.selectedPath !== "" ? (
        <a
          className="forge-tree-parent"
          href={encodeSearch(projection, { path: parentPath(projection.selectedPath) })}
        >
          <Folder aria-hidden="true" />
          <span>..</span>
          <small>Parent directory</small>
        </a>
      ) : null}
      {entries.length === 0 ? (
        <div className="forge-tree-empty">
          <Archive aria-hidden="true" />
          <p>
            {query === ""
              ? "This directory has no tracked files at the selected revision."
              : `No files match “${query}”.`}
          </p>
        </div>
      ) : (
        <ul>
          {entries.map((entry) => (
            <li key={`${entry.kind}:${entry.path}`}>
              <a href={encodeSearch(projection, { path: entry.path })}>
                {iconForEntry(entry.kind)}
                <span>{entry.name}</span>
                <small>{entry.kind === "file" ? bytes(entry.size) : entry.kind}</small>
              </a>
            </li>
          ))}
        </ul>
      )}
    </nav>
  );
}

function Breadcrumbs({
  projection,
  path,
}: {
  projection: ForgeRepositoryProjection;
  path: string;
}) {
  const segments = path.split("/").filter(Boolean);
  return (
    <nav aria-label="File path" className="forge-breadcrumbs">
      <a href={encodeSearch(projection, { path: "" })}>{projection.repository.name}</a>
      {segments.map((segment, index) => {
        const target = segments.slice(0, index + 1).join("/");
        return (
          <span key={target}>
            <ChevronRight aria-hidden="true" />
            <a
              aria-current={index === segments.length - 1 ? "page" : undefined}
              href={encodeSearch(projection, { path: target })}
            >
              {segment}
            </a>
          </span>
        );
      })}
    </nav>
  );
}

function CodeFile({
  file,
  projection,
}: {
  file: Extract<ForgeFileContent, { readonly _tag: "text" }>;
  projection: ForgeRepositoryProjection;
}) {
  const lines: ReadonlyArray<
    ReadonlyArray<{
      readonly content: string;
      readonly color?: string;
      readonly fontStyle?: number;
    }>
  > = file.highlightedLines ?? file.content.split("\n").map((line) => [{ content: line }]);
  return (
    <section aria-labelledby="forge-file-title" className="forge-file">
      <header>
        <div>
          <FileCode2 aria-hidden="true" />
          <h2 id="forge-file-title">{file.path.split("/").at(-1)}</h2>
        </div>
        <span>
          {lines.length} lines · {bytes(file.byteSize)} · {file.language ?? "text"}
        </span>
      </header>
      <div className="forge-code-scroll" tabIndex={0} aria-label={`${file.path} source code`}>
        <pre>
          <code>
            {lines.map((tokens, index) => {
              const line = index + 1;
              return (
                <span className="forge-code-line" id={`L${line}`} key={line}>
                  <a
                    aria-label={`Permalink to line ${line}`}
                    className="forge-line-number"
                    href={`${encodeSearch(projection, { path: file.path })}#L${line}`}
                  >
                    {line}
                  </a>
                  <span className="forge-line-content">
                    {tokens.map((token, tokenIndex) => (
                      <span
                        key={`${line}:${tokenIndex}`}
                        style={{
                          ...(token.color === undefined ? {} : { color: token.color }),
                          ...(token.fontStyle === undefined
                            ? {}
                            : {
                                fontStyle: (token.fontStyle & 1) === 1 ? "italic" : undefined,
                                fontWeight: (token.fontStyle & 2) === 2 ? 700 : undefined,
                                textDecoration:
                                  (token.fontStyle & 4) === 4 ? "underline" : undefined,
                              }),
                        }}
                      >
                        {token.content}
                      </span>
                    ))}
                    {"\n"}
                  </span>
                </span>
              );
            })}
          </code>
        </pre>
      </div>
    </section>
  );
}

function MarkdownFile({
  file,
  projection,
  title = "README",
}: {
  file: Extract<ForgeFileContent, { readonly _tag: "markdown" }>;
  projection: ForgeRepositoryProjection;
  title?: string;
}) {
  const html = useMemo(
    () => safeMarkdownHtml(file.content, projection, file.path, file.assets ?? []),
    [file.assets, file.content, file.path, projection],
  );
  return (
    <section aria-labelledby="forge-readme-title" className="forge-readme">
      <header>
        <BookOpen aria-hidden="true" />
        <h2 id="forge-readme-title">{title}</h2>
        <span>{bytes(file.byteSize)}</span>
      </header>
      <article dangerouslySetInnerHTML={{ __html: html }} />
    </section>
  );
}

function FileRefusal({ file }: { file: Extract<ForgeFileContent, { readonly _tag: "refusal" }> }) {
  const copy =
    file.reason === "too_large"
      ? "This file is larger than the safe web-view limit. Clone the repository to inspect it."
      : file.reason === "binary"
        ? "This binary file cannot be shown as source text."
        : "This image format is not admitted for inline rendering.";
  return (
    <section className="forge-refusal" role="status">
      <Binary aria-hidden="true" />
      <h2>Preview refused</h2>
      <p>{copy}</p>
      <code>
        {file.path} · {bytes(file.byteSize)} · {file.objectId.slice(0, 12)}
      </code>
    </section>
  );
}

function FileContent({ projection }: { projection: ForgeRepositoryProjection }) {
  const file = projection.file;
  if (file === null) {
    if (projection.readme !== null) {
      return <MarkdownFile file={projection.readme} projection={projection} />;
    }
    return (
      <section className="forge-empty" role="status">
        <BookOpen aria-hidden="true" />
        <h2>No README at this revision</h2>
        <p>Select a file from the tree, or choose another branch.</p>
      </section>
    );
  }
  if (file._tag === "text") return <CodeFile file={file} projection={projection} />;
  if (file._tag === "markdown") {
    return <MarkdownFile file={file} projection={projection} title={file.path} />;
  }
  if (file._tag === "refusal") return <FileRefusal file={file} />;
  return (
    <section className="forge-image">
      <header>
        <FileImage aria-hidden="true" />
        <h2>{file.path}</h2>
        <span>{bytes(file.byteSize)}</span>
      </header>
      <div>
        <img alt={`Contents of ${file.path}`} src={file.sourceUrl} />
      </div>
    </section>
  );
}

function Commits({ projection }: { projection: ForgeRepositoryProjection }) {
  return (
    <section aria-labelledby="forge-commits-title" className="forge-commits">
      <header>
        <GitCommitHorizontal aria-hidden="true" />
        <div>
          <h2 id="forge-commits-title">Commit history</h2>
          <p>{projection.selectedRef.replace(/^refs\/heads\//, "")}</p>
        </div>
      </header>
      {projection.commits.length === 0 ? (
        <div className="forge-empty" role="status">
          <GitCommitHorizontal aria-hidden="true" />
          <h3>No commits are available</h3>
          <p>The selected reference does not have a visible commit projection.</p>
        </div>
      ) : (
        <ol>
          {projection.commits.map((commit) => (
            <li key={commit.objectId}>
              <div>
                <a
                  href={encodeSearch(projection, {
                    view: "commit",
                    commit: commit.objectId,
                  })}
                >
                  {commit.subject}
                </a>
                <p>
                  {commit.authorName} committed{" "}
                  <time dateTime={commit.authoredAt} title={dateTime(commit.authoredAt)}>
                    {relativeTime(commit.authoredAt, Date.parse(projection.servedAt))}
                  </time>
                </p>
              </div>
              <a
                className="forge-sha"
                href={encodeSearch(projection, {
                  view: "commit",
                  commit: commit.objectId,
                })}
              >
                {commit.shortId}
              </a>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

type DiffLine = Readonly<{
  kind: "add" | "remove" | "context" | "meta";
  content: string;
  oldLine?: number;
  newLine?: number;
}>;

const parseDiff = (source: string): ReadonlyArray<DiffLine> => {
  let oldLine = 0;
  let newLine = 0;
  return source.split("\n").map((content) => {
    const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(content);
    if (hunk !== null) {
      oldLine = Number(hunk[1]);
      newLine = Number(hunk[2]);
      return { kind: "meta", content };
    }
    if (content.startsWith("+") && !content.startsWith("+++")) {
      return { kind: "add", content, newLine: newLine++ };
    }
    if (content.startsWith("-") && !content.startsWith("---")) {
      return { kind: "remove", content, oldLine: oldLine++ };
    }
    if (content.startsWith(" ")) {
      return { kind: "context", content, oldLine: oldLine++, newLine: newLine++ };
    }
    return { kind: "meta", content };
  });
};

function Diff({ projection }: { projection: ForgeRepositoryProjection }) {
  if (projection.diff === null) {
    return (
      <section className="forge-empty" role="status">
        <GitCompareArrows aria-hidden="true" />
        <h2>No diff is available</h2>
        <p>Choose a commit with a parent or provide two visible revisions.</p>
      </section>
    );
  }
  const lines = parseDiff(projection.diff.unified);
  return (
    <section aria-labelledby="forge-diff-title" className="forge-diff">
      <header>
        <GitCompareArrows aria-hidden="true" />
        <div>
          <h2 id="forge-diff-title">Revision diff</h2>
          <code>
            {projection.diff.baseObjectId.slice(0, 8)} → {projection.diff.headObjectId.slice(0, 8)}
          </code>
        </div>
      </header>
      <div className="forge-diff-scroll" tabIndex={0}>
        <pre>
          <code>
            {lines.map((line, index) => (
              <span data-diff-line={line.kind} key={index}>
                <i>{line.oldLine ?? ""}</i>
                <i>{line.newLine ?? ""}</i>
                <b>{line.content}</b>
                {"\n"}
              </span>
            ))}
          </code>
        </pre>
      </div>
      {projection.diff.truncated ? (
        <p className="forge-truncated" role="status">
          The diff reached the bounded display limit.
        </p>
      ) : null}
    </section>
  );
}

function Commit({ projection }: { projection: ForgeRepositoryProjection }) {
  const commit = projection.commit;
  if (commit === null) {
    return (
      <section className="forge-empty" role="status">
        <GitCommitHorizontal aria-hidden="true" />
        <h2>Commit not available</h2>
        <p>The owned Forge service did not return this commit.</p>
      </section>
    );
  }
  return (
    <section className="forge-commit">
      <header>
        <GitCommitHorizontal aria-hidden="true" />
        <div>
          <h2>{commit.subject}</h2>
          <p>{commit.body}</p>
        </div>
      </header>
      <dl>
        <div>
          <dt>Commit</dt>
          <dd>
            <code>{commit.objectId}</code>
          </dd>
        </div>
        <div>
          <dt>Author</dt>
          <dd>{commit.authorName}</dd>
        </div>
        <div>
          <dt>Authored</dt>
          <dd>
            <time dateTime={commit.authoredAt}>{dateTime(commit.authoredAt)}</time>
          </dd>
        </div>
        <div>
          <dt>Parents</dt>
          <dd>
            {commit.parentIds.length === 0
              ? "Root commit"
              : commit.parentIds.map((parent) => <code key={parent}>{parent.slice(0, 12)}</code>)}
          </dd>
        </div>
        <div>
          <dt>Diffstat</dt>
          <dd>
            <span className="forge-add">+{commit.additions}</span>{" "}
            <span className="forge-del">−{commit.deletions}</span> across {commit.changedFiles}{" "}
            files
          </dd>
        </div>
      </dl>
      <a
        className="forge-primary-action"
        href={encodeSearch(projection, {
          view: "diff",
          commit: commit.objectId,
          ...(commit.parentIds[0] === undefined ? {} : { base: commit.parentIds[0] }),
        })}
      >
        <GitCompareArrows aria-hidden="true" /> View changes
      </a>
    </section>
  );
}

function Failure({
  failure,
  request,
}: {
  failure: ForgeRepositoryReadFailure;
  request: ForgeRepositoryReadRequest;
}) {
  const auth = failure._tag === "authentication_required";
  const missing = failure._tag === "not_found";
  return (
    <div className="forge-shell forge-state-page">
      <div className="forge-state-mark">
        {auth ? (
          <LockKeyhole aria-hidden="true" />
        ) : missing ? (
          <Archive aria-hidden="true" />
        ) : (
          <AlertTriangle aria-hidden="true" />
        )}
      </div>
      <p className="forge-state-path">
        openagents / forge / {request.owner} / {request.repo}
      </p>
      <h1>
        {auth ? "Invitation required" : missing ? "Repository not found" : "Forge read unavailable"}
      </h1>
      <p>{failure.detail}</p>
      <div className="forge-state-actions">
        {auth ? (
          <a className="forge-primary-action" href="/login">
            Sign in
          </a>
        ) : null}
        <a href={`/forge/${encodeURIComponent(request.owner)}/${encodeURIComponent(request.repo)}`}>
          <RefreshCw aria-hidden="true" /> Try again
        </a>
        <a href="/">OpenAgents home</a>
      </div>
      <small>
        Repository content is read only from the owned Forge service. No GitHub fallback is used.
      </small>
    </div>
  );
}

function Repository({
  projection,
  request,
}: {
  projection: ForgeRepositoryProjection;
  request: ForgeRepositoryReadRequest;
}) {
  const [copied, setCopied] = useState(false);
  const branches = projection.refs.filter((ref) => ref.kind === "branch").length;
  const tags = projection.refs.filter((ref) => ref.kind === "tag").length;
  const publicRead = projection.access.mode === "public_web_read";
  const copyClone = async () => {
    await navigator.clipboard.writeText(`git clone ${projection.repository.canonicalCloneUrl}`);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1800);
  };
  return (
    <div className="forge-shell">
      <header className="forge-repo-header">
        <div className="forge-repo-title">
          <Network aria-hidden="true" />
          <div>
            <nav aria-label="Repository breadcrumb">
              <a href="/forge">Forge</a>
              <ChevronRight aria-hidden="true" />
              <span>{projection.repository.owner}</span>
              <ChevronRight aria-hidden="true" />
              <strong>{projection.repository.name}</strong>
            </nav>
            <h1>
              <span className="forge-repo-owner">{projection.repository.owner}</span>
              <span className="forge-repo-separator">/</span>
              <span className="forge-repo-name">{projection.repository.name}</span>
            </h1>
            <p>{projection.repository.description || "No repository description is available."}</p>
          </div>
        </div>
        <div className="forge-repo-status">
          <span>
            <ShieldCheck aria-hidden="true" />{" "}
            {projection.repository.authorityMode === "openagents_git_authoritative"
              ? "OpenAgents Git authority"
              : "Migration pending"}
          </span>
          <span>
            <Clock3 aria-hidden="true" /> {projection.repository.projectionFreshness}
          </span>
          <span>
            <CircleDot aria-hidden="true" /> {publicRead ? "Public web read" : "Member read"}
          </span>
        </div>
      </header>

      <section aria-label="Repository facts" className="forge-facts">
        <div>
          <span>NIP-34 coordinate</span>
          <code>{projection.repository.nip34Coordinate}</code>
        </div>
        <div>
          <span>Maintainers</span>
          <strong>
            {publicRead
              ? "Hidden in public view"
              : `${projection.repository.maintainers.length} visible`}
          </strong>
        </div>
        <div>
          <span>References</span>
          <strong>{refCountLabel(branches, tags)}</strong>
        </div>
        <div className="forge-clone">
          <span>Read-only clone</span>
          <code>git clone {projection.repository.canonicalCloneUrl}</code>
          <button aria-label="Copy clone command" onClick={() => void copyClone()} type="button">
            <Copy aria-hidden="true" /> {copied ? "Copied" : "Copy"}
          </button>
        </div>
      </section>

      <div className="forge-toolbar">
        <RefMenu projection={projection} />
        <nav aria-label="Repository views" className="forge-tabs">
          <a
            aria-current={request.view === "code" ? "page" : undefined}
            href={encodeSearch(projection, { view: "code", commit: undefined, base: undefined })}
          >
            <Code2 aria-hidden="true" /> Code
          </a>
          <a
            aria-current={request.view === "commits" ? "page" : undefined}
            href={encodeSearch(projection, {
              view: "commits",
              path: undefined,
              commit: undefined,
              base: undefined,
            })}
          >
            <GitCommitHorizontal aria-hidden="true" /> Commits
          </a>
        </nav>
      </div>

      {request.view === "code" ? (
        <>
          {projection.selectedPath !== "" ? (
            <Breadcrumbs path={projection.selectedPath} projection={projection} />
          ) : null}
          <details className="forge-mobile-tree">
            <summary>
              <Folder aria-hidden="true" /> Browse files
            </summary>
            <Tree projection={projection} />
          </details>
          <div className="forge-browser">
            <Tree projection={projection} />
            <main className="forge-content">
              <FileContent projection={projection} />
            </main>
          </div>
        </>
      ) : request.view === "commits" ? (
        <Commits projection={projection} />
      ) : request.view === "commit" ? (
        <Commit projection={projection} />
      ) : (
        <Diff projection={projection} />
      )}

      <footer className="forge-footer">
        <span>
          Served at <time dateTime={projection.servedAt}>{dateTime(projection.servedAt)}</time>
        </span>
        <code>{projection.repository.repositoryRef}</code>
        {publicRead ? (
          <span>
            <Users aria-hidden="true" /> Member data and write actions are omitted.
          </span>
        ) : null}
      </footer>
    </div>
  );
}

export function ForgeRepositoryPage({ request, result }: ForgePageProps) {
  return (
    <>
      <PublicHeader />
      <div className="forge-page">
        <a className="forge-skip-link" href="#forge-main">
          Skip to repository content
        </a>
        <div id="forge-main">
          {result._tag === "failed" ? (
            <Failure failure={result.failure} request={request} />
          ) : (
            <Repository projection={result.projection} request={request} />
          )}
        </div>
      </div>
    </>
  );
}

export function ForgeRepositorySkeleton() {
  return (
    <div
      aria-busy="true"
      aria-label="Loading repository"
      className="forge-page forge-shell forge-skeleton"
    >
      <div className="forge-skeleton-title" />
      <div className="forge-skeleton-line forge-skeleton-line-wide" />
      <div className="forge-skeleton-facts">
        {Array.from({ length: 4 }, (_, index) => (
          <i key={index} />
        ))}
      </div>
      <div className="forge-skeleton-browser">
        <i />
        <b />
      </div>
      <span className="sr-only">Loading repository</span>
    </div>
  );
}
