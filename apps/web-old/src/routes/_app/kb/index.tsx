import { useEffect } from 'react';
import { createFileRoute, Link } from '@tanstack/react-router';
import { Card, CardHeader, CardTitle } from '@/components/ui/card';
import { FormattedDate } from '@/components/kb/FormattedDate';
import { posthogCapture } from '@/lib/posthog';
import { SITE_DESCRIPTION, SITE_TITLE } from '@/consts';
import { buildHead } from '@/lib/seo';
import { getKbEntries } from '@/lib/content';

export const Route = createFileRoute('/_app/kb/')({
  component: KbIndexPage,
  head: () =>
    buildHead({
      title: `Knowledge Base | ${SITE_TITLE}`,
      description: SITE_DESCRIPTION,
    }),
});

function KbIndexPage() {
  const articles = getKbEntries();

  useEffect(() => {
    posthogCapture('kb_view', { view: 'index' });
  }, []);

  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <h1 className="mb-2 text-2xl font-bold">Knowledge Base</h1>
      <p className="mb-2 max-w-[70ch] text-muted-foreground">
        Short, durable explanations of the primitives OpenAgents cares about: identity, coordination, and settlement
        for autonomous agents. Written for humans and agents.
      </p>
      <p className="mb-6 max-w-[70ch] text-muted-foreground">
        OpenAgents is a runtime + compiler + (optional) market: verification (tests/builds) is the ground truth; the
        canonical output of a session is the <strong>Verified Patch Bundle</strong> (PR_SUMMARY.md, RECEIPT.json,
        REPLAY.jsonl). Terminology: <code className="rounded bg-muted px-1 py-0.5 font-mono text-sm">GLOSSARY.md</code>{' '}
        in the repo; implementation status:{' '}
        <code className="rounded bg-muted px-1 py-0.5 font-mono text-sm">SYNTHESIS_EXECUTION.md</code>; roadmap:{' '}
        <code className="rounded bg-muted px-1 py-0.5 font-mono text-sm">ROADMAP.md</code>.
      </p>
      <div className="grid gap-4 sm:grid-cols-2">
        {articles.map((article) => (
          <Card
            key={article.id}
            className="border-border transition-colors hover:border-primary/50"
          >
            <Link to="/kb/$slug" params={{ slug: article.slug }} className="block">
              <CardHeader>
                <CardTitle className="text-base">{article.data.title}</CardTitle>
                <p className="m-0 text-sm text-muted-foreground">
                  <FormattedDate date={article.data.pubDate} />
                </p>
                <p className="m-0 mt-1 text-sm text-muted-foreground">{article.data.description}</p>
                {article.data.tags?.length ? (
                  <div className="mt-2 flex flex-wrap gap-1">
                    {article.data.tags.slice(0, 6).map((tag) => (
                      <span
                        key={tag}
                        className="rounded border border-border bg-muted px-2 py-0.5 font-mono text-xs text-muted-foreground"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                ) : null}
              </CardHeader>
            </Link>
          </Card>
        ))}
      </div>
    </div>
  );
}
