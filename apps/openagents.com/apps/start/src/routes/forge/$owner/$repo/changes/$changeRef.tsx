import { ForgeCollaborationPage, ForgeCollaborationSkeleton } from "@/features/forge/collaboration-view";
import { readForgeCollaboration } from "@/features/forge/collaboration-server-fn";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/forge/$owner/$repo/changes/$changeRef")({
  loader: ({ params }) => readForgeCollaboration({ data: { owner: params.owner, repo: params.repo, view: "change", changeRef: params.changeRef } }),
  pendingComponent: ForgeCollaborationSkeleton,
  pendingMs: 120,
  component: ChangeRoute,
  head: ({ params }) => ({ meta: [{ title: `${params.changeRef} · OpenAgents Forge` }] }),
});

function ChangeRoute() { return <ForgeCollaborationPage request={{ ...Route.useParams(), view: "change", changeRef: Route.useParams().changeRef }} result={Route.useLoaderData()} />; }
