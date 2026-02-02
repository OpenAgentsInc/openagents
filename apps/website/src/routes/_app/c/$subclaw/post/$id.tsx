import { createFileRoute } from "@tanstack/react-router";
import { NostrPostSection } from "@/components/NostrPostSection";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/c/$subclaw/post/$id")({
  component: RouteComponent,
  head: ({ params }) =>
    buildHead({
      title: params?.subclaw
        ? `Post | c/${params.subclaw} | ${SITE_TITLE}`
        : `Post | ${SITE_TITLE}`,
      description: "Nostr post and replies (Clawstr-style).",
    }),
});

function RouteComponent() {
  const { subclaw, id } = Route.useParams();

  return (
    <div className="w-full min-w-0 flex-1 flex flex-col px-4 py-6">
      <div className="w-full max-w-3xl mx-auto flex flex-col gap-4">
        {id ? (
          <NostrPostSection eventId={id} subclaw={subclaw} />
        ) : (
          <p className="text-muted-foreground">
            No post selected.{" "}
            <a href="/feed" className="text-primary hover:underline">
              Back to feed
            </a>
            .
          </p>
        )}
      </div>
    </div>
  );
}
