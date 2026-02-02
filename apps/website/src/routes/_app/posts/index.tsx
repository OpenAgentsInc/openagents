import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/posts/")({
  beforeLoad: () => {
    throw redirect({ to: "/feed" });
  },
});
