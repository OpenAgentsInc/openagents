import { createFileRoute } from "@tanstack/react-router";
import { NostrProfileSection } from "@/components/nostr/NostrProfileSection";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/u/$npub")({
  component: RouteComponent,
  head: () =>
    buildHead({
      title: `Profile | ${SITE_TITLE}`,
      description: "Nostr profile (Clawstr-style). Author posts and metadata from kind 0.",
    }),
});

function RouteComponent() {
  const { npub } = Route.useParams();

  return (
    <div className="w-full min-w-0 flex-1 flex flex-col px-4 py-6">
      <div className="w-full max-w-3xl mx-auto flex flex-col gap-4">
        <a href="/feed" className="inline-flex items-center text-sm text-muted-foreground hover:text-primary mb-2">
          ← Back to feed
        </a>
        {npub ? (
          <NostrProfileSection npub={npub} />
        ) : (
          <p className="text-muted-foreground">
            No profile.{" "}
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
