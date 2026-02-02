import { createFileRoute } from "@tanstack/react-router";
import { BlogPostLayout } from "@/components/blog/BlogPostLayout";
import { MarkdownContent } from "@/components/blog/MarkdownContent";
import { SITE_DESCRIPTION, SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";
import { getKbEntryBySlug } from "@/lib/content";

export const Route = createFileRoute("/_app/kb/$")({
  component: RouteComponent,
  head: ({ params }) => {
    const slug = (params?._splat ?? "").replace(/\/$/, "");
    const entry = slug ? getKbEntryBySlug(slug) : undefined;
    return buildHead({
      title: entry ? entry.data.title : SITE_TITLE,
      description: entry?.data.description ?? SITE_DESCRIPTION,
      image: entry?.data.heroImage,
    });
  },
});

function RouteComponent() {
  const { _splat } = Route.useParams();
  const slug = (_splat ?? "").replace(/\/$/, "");
  const entry = slug ? getKbEntryBySlug(slug) : undefined;

  if (!entry) {
    return (
      <div className="mx-auto max-w-3xl px-4 py-8">
        <p className="text-muted-foreground">
          Article not found.{" "}
          <a href="/kb" className="text-primary hover:underline">
            Back to knowledge base
          </a>
          .
        </p>
      </div>
    );
  }

  return (
    <BlogPostLayout
      title={entry.data.title}
      pubDate={entry.data.pubDate}
      updatedDate={entry.data.updatedDate}
      heroImage={entry.data.heroImage}
      tags={entry.data.tags}
    >
      <MarkdownContent content={entry.body} />
    </BlogPostLayout>
  );
}
