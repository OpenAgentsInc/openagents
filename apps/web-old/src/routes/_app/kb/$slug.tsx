import { useEffect } from 'react';
import { createFileRoute, Link } from '@tanstack/react-router';
import { KbArticleLayout, MarkdownContent } from '@/components/kb';
import { posthogCapture } from '@/lib/posthog';
import { SITE_DESCRIPTION, SITE_TITLE } from '@/consts';
import { buildHead } from '@/lib/seo';
import { getKbEntryBySlug } from '@/lib/content';

export const Route = createFileRoute('/_app/kb/$slug')({
  component: KbArticlePage,
  head: ({ params }) => {
    const slug = params.slug ?? '';
    const entry = slug ? getKbEntryBySlug(slug) : undefined;
    return buildHead({
      title: entry ? entry.data.title : SITE_TITLE,
      description: entry?.data.description ?? SITE_DESCRIPTION,
      image: entry?.data.heroImage,
    });
  },
});

function KbArticlePage() {
  const { slug } = Route.useParams();
  const entry = slug ? getKbEntryBySlug(slug) : undefined;

  useEffect(() => {
    const e = slug ? getKbEntryBySlug(slug) : undefined;
    if (e) posthogCapture('kb_article_view', { slug, title: e.data.title });
  }, [slug]);

  if (!entry) {
    return (
      <div className="mx-auto max-w-3xl px-4 py-8">
        <p className="text-muted-foreground">
          Article not found.{' '}
          <Link to="/kb" className="text-primary hover:underline">
            Back to knowledge base
          </Link>
          .
        </p>
      </div>
    );
  }

  return (
    <KbArticleLayout
      title={entry.data.title}
      pubDate={entry.data.pubDate}
      updatedDate={entry.data.updatedDate}
      heroImage={entry.data.heroImage}
      tags={entry.data.tags}
    >
      <MarkdownContent content={entry.body} />
    </KbArticleLayout>
  );
}
