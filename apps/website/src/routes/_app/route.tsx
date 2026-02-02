import { Outlet, createFileRoute } from "@tanstack/react-router";
import { SiteLayout } from "@/components/SiteLayout";

export const Route = createFileRoute("/_app")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <SiteLayout>
      <Outlet />
    </SiteLayout>
  );
}
