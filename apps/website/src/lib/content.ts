import matter from "gray-matter";

export type ContentFrontmatter = {
  title: string;
  description: string;
  pubDate: Date;
  updatedDate?: Date;
  heroImage?: string;
  tags?: string[];
};

export type ContentEntry<T extends ContentFrontmatter = ContentFrontmatter> = {
  id: string;
  slug: string;
  body: string;
  data: T;
};

const blogFiles = import.meta.glob("../content/blog/**/*.{md,mdx}", {
  query: "?raw",
  import: "default",
  eager: true,
});
const kbFiles = import.meta.glob("../content/kb/**/*.{md,mdx}", {
  query: "?raw",
  import: "default",
  eager: true,
});

function parseDate(value: unknown): Date {
  if (value instanceof Date) return value;
  if (typeof value === "string" || typeof value === "number") {
    const parsed = new Date(value);
    if (!Number.isNaN(parsed.valueOf())) return parsed;
  }
  return new Date(0);
}

function normalizeFrontmatter(raw: Record<string, unknown>): ContentFrontmatter {
  const title = typeof raw.title === "string" ? raw.title : "Untitled";
  const description = typeof raw.description === "string" ? raw.description : "";
  const pubDate = parseDate(raw.pubDate);
  const updatedDate = raw.updatedDate ? parseDate(raw.updatedDate) : undefined;
  const heroImage = typeof raw.heroImage === "string" ? raw.heroImage : undefined;
  const tags = Array.isArray(raw.tags) ? raw.tags.map((tag) => String(tag)) : undefined;
  return { title, description, pubDate, updatedDate, heroImage, tags };
}

function getIdFromPath(path: string, collection: "blog" | "kb"): string {
  const normalized = path.replace(/\\/g, "/");
  const marker = `/content/${collection}/`;
  const idx = normalized.lastIndexOf(marker);
  const relative = idx >= 0 ? normalized.slice(idx + marker.length) : normalized.split("/").pop() ?? "";
  return relative.replace(/\.(md|mdx)$/, "");
}

function parseCollection(files: Record<string, string>, collection: "blog" | "kb") {
  return Object.entries(files).map(([path, raw]) => {
    const { data, content } = matter(raw);
    const id = getIdFromPath(path, collection);
    return {
      id,
      slug: id,
      body: content,
      data: normalizeFrontmatter(data as Record<string, unknown>),
    };
  });
}

const blogEntries = parseCollection(blogFiles as Record<string, string>, "blog");
const kbEntries = parseCollection(kbFiles as Record<string, string>, "kb");

export function getBlogPosts(): ContentEntry[] {
  return [...blogEntries].sort((a, b) => b.data.pubDate.valueOf() - a.data.pubDate.valueOf());
}

export function getBlogPostBySlug(slug: string): ContentEntry | undefined {
  return blogEntries.find((entry) => entry.slug === slug);
}

export function getKbEntries(): ContentEntry[] {
  return [...kbEntries].sort((a, b) => b.data.pubDate.valueOf() - a.data.pubDate.valueOf());
}

export function getKbEntryBySlug(slug: string): ContentEntry | undefined {
  return kbEntries.find((entry) => entry.slug === slug);
}

export function getAllContentSlugs(): { type: "blog" | "kb"; slug: string; updated?: Date }[] {
  return [
    ...blogEntries.map((entry) => ({
      type: "blog" as const,
      slug: entry.slug,
      updated: entry.data.updatedDate ?? entry.data.pubDate,
    })),
    ...kbEntries.map((entry) => ({
      type: "kb" as const,
      slug: entry.slug,
      updated: entry.data.updatedDate ?? entry.data.pubDate,
    })),
  ];
}
