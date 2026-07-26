import { ForgeCollaborationPage, ForgeCollaborationSkeleton } from "@/features/forge/collaboration-view";
import { readForgeCollaboration } from "@/features/forge/collaboration-server-fn";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/forge/$owner/$repo/work/$workRef")({
  loader: ({ params }) => readForgeCollaboration({ data: { owner: params.owner, repo: params.repo, view: "work", workRef: params.workRef } }),
  pendingComponent: ForgeCollaborationSkeleton,
  pendingMs: 120,
  component: WorkRoute,
  head: ({ params }) => ({ meta: [{ title: `${params.workRef} · OpenAgents Forge` }] }),
});

function WorkRoute() { return <ForgeCollaborationPage request={{ ...Route.useParams(), view: "work", workRef: Route.useParams().workRef }} result={Route.useLoaderData()} />; }
