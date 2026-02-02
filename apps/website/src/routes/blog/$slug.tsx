import { createFileRoute } from "@tanstack/react-router";
import { SiteLayout } from "@/components/SiteLayout";
import { BlogPostLayout } from "@/components/BlogPostLayout";
import { MarkdownContent } from "@/components/MarkdownContent";
import { SITE_DESCRIPTION, SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";
import { getBlogPostBySlug } from "@/lib/content";

export const Route = createFileRoute("/blog/$slug")({
  component: RouteComponent,
  head: ({ params }) => {
    const post = params?.slug ? getBlogPostBySlug(params.slug) : undefined;
    return buildHead({
      title: post ? post.data.title : SITE_TITLE,
      description: post?.data.description ?? SITE_DESCRIPTION,
      image: post?.data.heroImage,
    });
  },
});

function RouteComponent() {
  const { slug } = Route.useParams();
  const post = slug ? getBlogPostBySlug(slug) : undefined;

  if (!post) {
    return (
      <SiteLayout>
        <div className="mx-auto max-w-3xl px-4 py-8">
          <p className="text-muted-foreground">
            Post not found.{" "}
            <a href="/blog" className="text-primary hover:underline">
              Back to blog
            </a>
            .
          </p>
        </div>
      </SiteLayout>
    );
  }

  return (
    <SiteLayout>
      <BlogPostLayout
        title={post.data.title}
        pubDate={post.data.pubDate}
        updatedDate={post.data.updatedDate}
        heroImage={post.data.heroImage}
        tags={post.data.tags}
      >
        <MarkdownContent content={post.body} />
      </BlogPostLayout>
    </SiteLayout>
  );
}
