import { createFileRoute } from "@tanstack/react-router";
import { NostrEventSection } from "@/components/nostr/NostrEventSection";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/event/$id")({
  component: RouteComponent,
  head: ({ params }) =>
    buildHead({
      title: params?.id ? `event/${params.id} | ${SITE_TITLE}` : SITE_TITLE,
      description: "View a Nostr event by id.",
    }),
});

function RouteComponent() {
  const { id } = Route.useParams();

  return (
    <div className="w-full min-w-0 flex-1 flex flex-col px-4 py-6">
      <div className="w-full max-w-3xl mx-auto flex flex-col gap-4">
        {id ? (
          <NostrEventSection eventId={id} />
        ) : (
          <p className="text-muted-foreground">
            No event selected.{" "}
            <a href="/feed" className="text-primary hover:underline">
              Go to feed
            </a>
            .
          </p>
        )}
      </div>
    </div>
  );
}
