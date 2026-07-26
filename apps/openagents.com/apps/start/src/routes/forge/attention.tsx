import { ForgeCollaborationPage, ForgeCollaborationSkeleton } from "@/features/forge/collaboration-view";
import { readForgeCollaboration } from "@/features/forge/collaboration-server-fn";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/forge/attention")({
  loader: () => readForgeCollaboration({ data: { owner: "openagents", repo: "attention", view: "attention" } }),
  pendingComponent: ForgeCollaborationSkeleton,
  pendingMs: 120,
  component: AttentionRoute,
  head: () => ({ meta: [{ title: "For me · OpenAgents Forge" }] }),
});

function AttentionRoute() { return <ForgeCollaborationPage request={{ owner: "openagents", repo: "attention", view: "attention" }} result={Route.useLoaderData()} />; }
