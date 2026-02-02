import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/communities/")({
  beforeLoad: () => {
    throw redirect({ to: "/c" });
  },
});
