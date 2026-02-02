import { createFileRoute } from "@tanstack/react-router";
import { HomeHero } from "@/components/home/HomeHero";
import { NostrFeedSection } from "@/components/nostr/NostrFeedSection";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { SITE_DESCRIPTION, SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";

export const Route = createFileRoute("/_app/")({
  component: RouteComponent,
  head: () => buildHead({ title: SITE_TITLE, description: SITE_DESCRIPTION }),
});

function RouteComponent() {
  return (
    <div className="flex min-h-0 w-full flex-1 flex-col">
      <HomeHero />
      <div className="flex min-h-0 w-full min-w-[20rem] max-w-4xl flex-1 flex-col px-4 py-8 mx-auto">
        <Card className="w-full min-w-[20rem]">
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-xl">Feed</CardTitle>
            <Button variant="ghost" size="sm" asChild>
              <a href="/feed" className="text-primary hover:underline">
                View full feed →
              </a>
            </Button>
          </CardHeader>
          <CardContent>
            <NostrFeedSection limit={10} />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
