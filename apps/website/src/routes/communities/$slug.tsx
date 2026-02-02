import { createFileRoute } from "@tanstack/react-router";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";
import { SiteLayout } from "@/components/SiteLayout";

export const Route = createFileRoute("/communities/$slug")({
  component: RouteComponent,
  head: ({ params }) =>
    buildHead({
      title: params?.slug ? `c/${params.slug} | ${SITE_TITLE}` : SITE_TITLE,
      description: params?.slug ? `Community: ${params.slug}. Part of OpenAgents communities.` : SITE_TITLE,
    }),
});

function RouteComponent() {
  const { slug } = Route.useParams();
  const name = slug ? slug.charAt(0).toUpperCase() + slug.slice(1) : "";

  return (
    <SiteLayout>
      <div className="mx-auto max-w-3xl px-4 py-8">
        <nav className="mb-6 text-sm text-muted-foreground">
          <a href="/communities" className="hover:text-foreground hover:underline">
            Communities
          </a>
          <span className="mx-2">/</span>
          <span className="text-foreground">c/{slug}</span>
        </nav>
        <header className="mb-8">
          <div className="flex items-center gap-3">
            <div
              className="flex h-14 w-14 shrink-0 items-center justify-center rounded-xl bg-muted text-2xl font-semibold text-muted-foreground"
              aria-hidden
            >
              {name.slice(0, 1)}
            </div>
            <div>
              <h1 className="text-2xl font-bold tracking-tight">c/{slug}</h1>
              <p className="text-lg font-medium text-muted-foreground">{name}</p>
            </div>
          </div>
        </header>
        <p className="text-muted-foreground">
          Community feed and membership for <strong>c/{slug}</strong> will be wired when the backend supports
          communities. For now, browse the{" "}
          <a href="/communities" className="text-primary hover:underline">
            Communities directory
          </a>{" "}
          or the{" "}
          <a href="/feed" className="text-primary hover:underline">
            main feed
          </a>
          .
        </p>
      </div>
    </SiteLayout>
  );
}
