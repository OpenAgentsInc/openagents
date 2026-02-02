import type { ReactNode } from "react";
import FormattedDate from "@/components/FormattedDate";

interface BlogPostLayoutProps {
  title: string;
  pubDate: Date;
  updatedDate?: Date;
  heroImage?: string;
  tags?: string[];
  children: ReactNode;
}

export function BlogPostLayout({
  title,
  pubDate,
  updatedDate,
  heroImage,
  tags,
  children,
}: BlogPostLayoutProps) {
  return (
    <article className="mx-auto max-w-3xl px-4 py-8">
      {heroImage && (
        <div className="mb-6">
          <img
            src={heroImage}
            alt=""
            className="rounded-lg border border-border shadow-sm w-full object-cover"
          />
        </div>
      )}
      <div className="prose prose-neutral dark:prose-invert max-w-none">
        <div className="mb-6 border-b border-border pb-4 text-center">
          <p className="text-sm text-muted-foreground m-0">
            <FormattedDate date={pubDate} />
            {updatedDate && (
              <span className="block italic mt-1">
                Last updated <FormattedDate date={updatedDate} />
              </span>
            )}
          </p>
          <h1 className="text-3xl font-bold mt-2 mb-0">{title}</h1>
          {tags?.length ? (
            <div className="flex flex-wrap justify-center gap-2 mt-3">
              {tags.slice(0, 8).map((tag) => (
                <span
                  key={tag}
                  className="rounded-md border border-border bg-muted px-2 py-0.5 text-xs font-mono text-muted-foreground"
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
