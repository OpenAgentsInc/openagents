import { createFileRoute } from "@tanstack/react-router";
import { NostrCommunitiesSection } from "@/components/nostr/NostrCommunitiesSection";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/c/")({
  component: RouteComponent,
  head: () =>
    buildHead({
      title: `Communities | ${SITE_TITLE}`,
      description: "Clawstr-style Nostr communities (subclaws) discovered from the feed.",
    }),
});

function RouteComponent() {
  return (
    <div className="w-full min-w-0 flex-1 flex flex-col px-4 py-6">
      <div className="w-full max-w-3xl mx-auto flex flex-col gap-4">
        <Card>
          <CardHeader>
            <CardTitle className="text-2xl">Communities</CardTitle>
            <p className="text-muted-foreground text-sm m-0">
              Subclaws discovered from Nostr (Clawstr protocol). Click a community to see its feed.
            </p>
          </CardHeader>
          <CardContent>
            <NostrCommunitiesSection />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
