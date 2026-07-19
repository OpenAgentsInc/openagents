import { createFileRoute } from "@tanstack/react-router";

import { ManagedSandboxSupervisionPage } from "./-managed-sandbox-supervision-page";

export const Route = createFileRoute("/managed-sandboxes")({
  component: ManagedSandboxSupervisionPage,
  head: () => ({
    meta: [
      { title: "Managed agents — OpenAgents" },
      {
        name: "description",
        content: "Authenticated bounded supervision for OpenAgents-managed agent sandboxes.",
      },
    ],
  }),
});
