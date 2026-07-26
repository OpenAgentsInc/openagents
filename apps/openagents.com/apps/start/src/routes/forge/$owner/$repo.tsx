import { ForgeRepositoryPage, ForgeRepositorySkeleton } from "@/features/forge/repository-view";
import { readForgeRepository } from "@/features/forge/repository-server-fn";
import type { ForgeRepositoryReadRequest } from "@/features/forge/repository-read";
import { createFileRoute } from "@tanstack/react-router";

type ForgeSearch = Readonly<{
  view?: "code" | "commits" | "commit" | "diff";
  ref?: string;
  path?: string;
  commit?: string;
  base?: string;
}>;

const nonEmpty = (value: unknown): string | undefined =>
  typeof value === "string" && value.trim() !== "" ? value : undefined;

const validateSearch = (search: Record<string, unknown>): ForgeSearch => {
  const view =
    search.view === "commits" || search.view === "commit" || search.view === "diff"
      ? search.view
      : "code";
  const ref = nonEmpty(search.ref);
  const commit = nonEmpty(search.commit);
  const base = nonEmpty(search.base);
  return {
    view,
    ...(ref === undefined ? {} : { ref }),
    ...(typeof search.path === "string" ? { path: search.path } : {}),
    ...(commit === undefined ? {} : { commit }),
    ...(base === undefined ? {} : { base }),
  };
};

export const Route = createFileRoute("/forge/$owner/$repo")({
  validateSearch,
  loaderDeps: ({ search }) => search,
  loader: ({ params, deps }) => {
    const request: ForgeRepositoryReadRequest = {
      owner: params.owner,
      repo: params.repo,
      view: deps.view ?? "code",
      ...(deps.ref === undefined ? {} : { ref: deps.ref }),
      ...(deps.path === undefined ? {} : { path: deps.path }),
      ...(deps.commit === undefined ? {} : { commit: deps.commit }),
      ...(deps.base === undefined ? {} : { base: deps.base }),
    };
    return readForgeRepository({ data: request }).then((result) => ({ request, result }));
  },
  pendingComponent: ForgeRepositorySkeleton,
  pendingMs: 120,
  component: ForgeRoutePage,
  head: ({ params }) => ({
    meta: [
      { title: `${params.owner}/${params.repo} · OpenAgents Forge` },
      {
        name: "description",
        content: `Browse ${params.owner}/${params.repo} from the owned OpenAgents Forge service.`,
      },
    ],
  }),
});

function ForgeRoutePage() {
  const { request, result } = Route.useLoaderData();
  return <ForgeRepositoryPage request={request} result={result} />;
}
