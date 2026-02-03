import type { ReactNode } from 'react';
import { FormattedDate } from '@/components/kb/FormattedDate';

interface KbArticleLayoutProps {
  title: string;
  pubDate: Date;
  updatedDate?: Date;
  heroImage?: string;
  tags?: string[];
  children: ReactNode;
}

export function KbArticleLayout({
  title,
  pubDate,
  updatedDate,
  heroImage,
  tags,
  children,
}: KbArticleLayoutProps) {
  return (
    <article className="mx-auto max-w-3xl px-4 py-8">
      {heroImage && (
        <div className="mb-6">
          <img
            src={heroImage}
            alt=""
            className="w-full rounded-lg border border-border object-cover shadow-sm"
          />
        </div>
      )}
      <div className="prose prose-neutral dark:prose-invert max-w-none">
        <div className="mb-6 border-b border-border pb-4 text-center">
          <p className="m-0 text-sm text-muted-foreground">
            <FormattedDate date={pubDate} />
            {updatedDate && (
              <span className="mt-1 block italic">
                Last updated <FormattedDate date={updatedDate} />
              </span>
            )}
          </p>
          <h1 className="mb-0 mt-2 text-3xl font-bold">{title}</h1>
          {tags?.length ? (
            <div className="mt-3 flex flex-wrap justify-center gap-2">
              {tags.slice(0, 8).map((tag) => (
                <span
                  key={tag}
                  className="rounded-md border border-border bg-muted px-2 py-0.5 font-mono text-xs text-muted-foreground"
                >
                  {tag}
                </span>
              ))}
            </div>
          ) : null}
        </div>
        {children}
      </div>
    </article>
  );
}
