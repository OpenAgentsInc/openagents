import { readFileSync } from "node:fs";
import path from "node:path";
import {
  ForgeRepositoryReadFailure,
  ForgeRepositoryReadResult,
  type ForgeRepositoryReadRequest,
} from "@/features/forge/repository-read";
import { ForgeRepositoryPage, ForgeRepositorySkeleton } from "@/features/forge/repository-view";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { forgeProjection } from "./repository-fixture";

const request: ForgeRepositoryReadRequest = {
  owner: "OpenAgentsInc",
  repo: "omega",
  view: "code",
  ref: "refs/heads/main",
  path: "src/index.ts",
};

const loaded = (projection = forgeProjection()) =>
  ForgeRepositoryReadResult.cases.loaded.make({ projection });

describe("Forge repository viewer", () => {
  test("renders identity, refs, tree, highlighted code, and line permalinks", () => {
    const html = renderToStaticMarkup(<ForgeRepositoryPage request={request} result={loaded()} />);

    expect(html).toContain(
      '<span class="forge-repo-owner">OpenAgentsInc</span><span class="forge-repo-separator">/</span><span class="forge-repo-name">omega</span>',
    );
    expect(html).toContain("OpenAgents Git authority");
    expect(html).toContain("NIP-34 coordinate");
    expect(html).toContain("1 branch · 1 tag");
    expect(html).toContain("path=src%2Fcomponents");
    expect(html).toContain('aria-label="src/index.ts source code"');
    expect(html).toContain('id="L1"');
    expect(html).toContain("Permalink to line 1");
    expect(html).toContain("color:#A0A0A0");
    expect(html).not.toContain("Push");
    expect(html).not.toContain("Edit file");
  });

  test("renders commit, diff, empty, refusal, and invitation-required states", () => {
    const commit = renderToStaticMarkup(
      <ForgeRepositoryPage
        request={{ ...request, view: "commit", commit: "a".repeat(40) }}
        result={loaded()}
      />,
    );
    expect(commit).toContain("Use the owned Forge read service.");
    expect(commit).toContain("+21");
    expect(commit).toContain("View changes");

    const diff = renderToStaticMarkup(
      <ForgeRepositoryPage
        request={{ ...request, view: "diff", commit: "a".repeat(40) }}
        result={loaded()}
      />,
    );
    expect(diff).toContain('data-diff-line="add"');
    expect(diff).toContain("Revision diff");

    const refusal = renderToStaticMarkup(
      <ForgeRepositoryPage
        request={request}
        result={loaded({
          ...forgeProjection(),
          file: {
            _tag: "refusal",
            path: "model.bin",
            objectId: "f".repeat(40),
            byteSize: 8_000_000,
            reason: "too_large",
          },
        })}
      />,
    );
    expect(refusal).toContain("Preview refused");
    expect(refusal).toContain("safe web-view limit");

    const auth = renderToStaticMarkup(
      <ForgeRepositoryPage
        request={request}
        result={ForgeRepositoryReadResult.cases.failed.make({
          failure: ForgeRepositoryReadFailure.cases.authentication_required.make({
            detail: "This repository requires a Forge invitation.",
          }),
        })}
      />,
    );
    expect(auth).toContain("Invitation required");
    expect(auth).toContain('href="/login"');
    expect(auth).toContain("No GitHub fallback is used");
  });

  test("escapes raw repository HTML and rejects javascript Markdown links", () => {
    const html = renderToStaticMarkup(
      <ForgeRepositoryPage
        request={{ ...request, path: "README.md" }}
        result={loaded({
          ...forgeProjection(),
          file: {
            _tag: "markdown",
            path: "README.md",
            objectId: "e".repeat(40),
            byteSize: 100,
            content:
              "# Safe\n\n<script>alert(1)</script>\n\n[bad](javascript:alert(1))\n\n![remote](https://github.com/tracker.png)\n\n![owned](media/owned.png)",
            assets: [
              {
                path: "media/owned.png",
                sourceUrl: "/api/forge/assets/owned.png",
                mimeType: "image/png",
              },
            ],
          },
        })}
      />,
    );

    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
    expect(html).not.toContain('href="javascript:');
    expect(html).not.toContain('src="https://github.com');
    expect(html).toContain("[Image: remote]");
    expect(html).toContain('src="/api/forge/assets/owned.png"');
  });

  test("ships geometry-matched loading and responsive reduced-motion rules", () => {
    const html = renderToStaticMarkup(<ForgeRepositorySkeleton />);
    const css = readFileSync(path.resolve(import.meta.dirname, "repository-view.css"), "utf8");

    expect(html).toContain('aria-busy="true"');
    expect(html).toContain("forge-skeleton-facts");
    expect(html).toContain("forge-skeleton-browser");
    expect(css).toContain("@media (max-width: 680px)");
    expect(css).toContain(".forge-mobile-tree");
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
    expect(css).not.toContain("border-radius: 32px");
    expect(css).not.toContain("background-clip: text");
  });
});
