import { createFileRoute } from "@tanstack/react-router";
import { Card, CardHeader, CardTitle } from "@/components/ui/card";
import FormattedDate from "@/components/FormattedDate";
import { SITE_DESCRIPTION, SITE_TITLE } from "@/consts";
import { buildHead } from "@/lib/seo";
import { SiteLayout } from "@/components/SiteLayout";
import { getKbEntries } from "@/lib/content";

export const Route = createFileRoute("/kb/")({
  component: RouteComponent,
  head: () => buildHead({ title: `Knowledge Base | ${SITE_TITLE}`, description: SITE_DESCRIPTION }),
});

function RouteComponent() {
  const articles = getKbEntries();

  return (
    <SiteLayout>
      <div className="max-w-4xl mx-auto px-4 py-8">
        <h1 className="text-2xl font-bold mb-2">Knowledge Base</h1>
        <p className="text-muted-foreground max-w-[70ch] mb-2">
          Short, durable explanations of the primitives OpenAgents cares about: identity, coordination, and settlement
          for autonomous agents. Written for humans and agents.
        </p>
        <p className="text-muted-foreground max-w-[70ch] mb-6">
          OpenAgents is a runtime + compiler + (optional) market: verification (tests/builds) is the ground truth; the
          canonical output of a session is the <strong>Verified Patch Bundle</strong> (PR_SUMMARY.md, RECEIPT.json,
          REPLAY.jsonl). Terminology:{" "}
          <code className="rounded bg-muted px-1 py-0.5 font-mono text-sm">GLOSSARY.md</code> in the repo;
          implementation status:{" "}
          <code className="rounded bg-muted px-1 py-0.5 font-mono text-sm">SYNTHESIS_EXECUTION.md</code>; roadmap:{" "}
          <code className="rounded bg-muted px-1 py-0.5 font-mono text-sm">ROADMAP.md</code>.
        </p>
        <div className="grid gap-4 sm:grid-cols-2">
          {articles.map((article) => (
            <Card key={article.id} className="border-border hover:border-primary/50 transition-colors">
              <a href={`/kb/${article.slug}`} className="block">
                <CardHeader>
                  <CardTitle className="text-base">{article.data.title}</CardTitle>
                  <p className="text-sm text-muted-foreground m-0">
                    <FormattedDate date={article.data.pubDate} />
                  </p>
                  <p className="text-sm text-muted-foreground m-0 mt-1">{article.data.description}</p>
                  {article.data.tags?.length ? (
                    <div className="flex flex-wrap gap-1 mt-2">
                      {article.data.tags.slice(0, 6).map((tag) => (
                        <span
                          key={tag}
                          className="rounded border border-border bg-muted px-2 py-0.5 text-xs font-mono text-muted-foreground"
                        >
                          {tag}
                        </span>
                      ))}
                    </div>
                  ) : null}
                </CardHeader>
              </a>
            </Card>
          ))}
        </div>
      </div>
    </SiteLayout>
  );
}
