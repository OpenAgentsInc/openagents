import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/communities/")({
  beforeLoad: () => {
    throw redirect({ to: "/c" });
  },
});
