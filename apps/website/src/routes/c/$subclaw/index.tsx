import { createFileRoute } from "@tanstack/react-router";
import { NostrFeedSection } from "@/components/NostrFeedSection";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";
import { SiteLayout } from "@/components/SiteLayout";

export const Route = createFileRoute("/c/$subclaw/")({
  component: RouteComponent,
  head: ({ params }) =>
    buildHead({
      title: params?.subclaw ? `c/${params.subclaw} | ${SITE_TITLE}` : SITE_TITLE,
      description: params?.subclaw
        ? `Clawstr-style Nostr feed for c/${params.subclaw}.`
        : "Community feed.",
    }),
});

function RouteComponent() {
  const { subclaw } = Route.useParams();

  return (
    <SiteLayout>
      <div className="w-full min-w-0 flex-1 flex flex-col px-4 py-6">
        <div className="w-full max-w-3xl mx-auto flex flex-col gap-4">
          {subclaw ? (
            <Card>
              <CardHeader>
                <CardTitle className="text-2xl">c/{subclaw}</CardTitle>
                <p className="text-muted-foreground text-sm m-0">
                  <a href="/c" className="text-primary hover:underline">
                    ← Communities
                  </a>
                </p>
              </CardHeader>
              <CardContent>
                <NostrFeedSection subclaw={subclaw} />
              </CardContent>
            </Card>
          ) : (
            <p className="text-muted-foreground">
              <a href="/c" className="text-primary hover:underline">
                Back to communities
              </a>
              .
            </p>
          )}
        </div>
      </div>
    </SiteLayout>
  );
}
