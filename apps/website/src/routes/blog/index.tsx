import { createFileRoute } from "@tanstack/react-router";
import { Card, CardHeader, CardTitle } from "@/components/ui/card";
import FormattedDate from "@/components/FormattedDate";
import { SITE_DESCRIPTION, SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";
import { SiteLayout } from "@/components/SiteLayout";
import { getBlogPosts } from "@/lib/content";

export const Route = createFileRoute("/blog/")({
  component: RouteComponent,
  head: () => buildHead({ title: `Blog | ${SITE_TITLE}`, description: SITE_DESCRIPTION }),
});

function RouteComponent() {
  const posts = getBlogPosts();

  return (
    <SiteLayout>
      <div className="max-w-4xl mx-auto px-4 py-8">
        <h1 className="text-2xl font-bold mb-6">Blog</h1>
        <div className="grid gap-6 sm:grid-cols-2">
          {posts.map((post, i) => (
            <Card key={post.id} className={i === 0 ? "sm:col-span-2" : ""}>
              <a href={`/blog/${post.slug}`} className="block">
                <CardHeader>
                  {post.data.heroImage && (
                    <div className="mb-2">
                      <img
                        src={post.data.heroImage}
                        alt=""
                        className="rounded-md w-full object-cover"
                      />
                    </div>
                  )}
                  <CardTitle className={i === 0 ? "text-2xl" : "text-lg"}>
                    {post.data.title}
                  </CardTitle>
                  <p className="text-sm text-muted-foreground m-0">
                    <FormattedDate date={post.data.pubDate} />
                  </p>
                </CardHeader>
              </a>
            </Card>
          ))}
        </div>
      </div>
    </SiteLayout>
  );
}
