import { createFileRoute } from "@tanstack/react-router";

import { ChangelogPage } from "./-changelog-page";

export const Route = createFileRoute("/changelog")({
  component: ChangelogPage,
  head: () => ({
    meta: [
      { title: "Changelog - OpenAgents" },
      {
        content:
          "What changed in each OpenAgents release, in plain language, with a link to the detailed engineering ledger for every release.",
        name: "description",
      },
    ],
    links: [{ href: "https://openagents.com/changelog", rel: "canonical" }],
  }),
});
