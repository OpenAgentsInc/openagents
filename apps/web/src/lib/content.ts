import matter from 'gray-matter';

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

const kbFiles = import.meta.glob<string>('../content/kb/**/*.{md,mdx}', {
  query: '?raw',
  import: 'default',
  eager: true,
});

function parseDate(value: unknown): Date {
  if (value instanceof Date) return value;
  if (typeof value === 'string' || typeof value === 'number') {
    const parsed = new Date(value);
    if (!Number.isNaN(parsed.valueOf())) return parsed;
  }
  return new Date(0);
}

function normalizeFrontmatter(raw: Record<string, unknown>): ContentFrontmatter {
  const title = typeof raw.title === 'string' ? raw.title : 'Untitled';
  const description = typeof raw.description === 'string' ? raw.description : '';
  const pubDate = parseDate(raw.pubDate);
  const updatedDate = raw.updatedDate ? parseDate(raw.updatedDate) : undefined;
  const heroImage = typeof raw.heroImage === 'string' ? raw.heroImage : undefined;
  const tags = Array.isArray(raw.tags) ? raw.tags.map((tag) => String(tag)) : undefined;
  return { title, description, pubDate, updatedDate, heroImage, tags };
}

function getIdFromPath(path: string): string {
  const normalized = path.replace(/\\/g, '/');
  const marker = '/content/kb/';
  const idx = normalized.lastIndexOf(marker);
  const relative = idx >= 0 ? normalized.slice(idx + marker.length) : normalized.split('/').pop() ?? '';
  return relative.replace(/\.(md|mdx)$/, '');
}

function parseKbCollection(): ContentEntry[] {
  return Object.entries(kbFiles).map(([path, raw]) => {
    const rawContent = typeof raw === 'string' ? raw : '';
    const { data, content: body } = matter(rawContent);
    const id = getIdFromPath(path);
    return {
      id,
      slug: id,
      body,
      data: normalizeFrontmatter(data as Record<string, unknown>),
    };
  });
}

const kbEntries = parseKbCollection();

export function getKbEntries(): ContentEntry[] {
  return [...kbEntries].sort((a, b) => b.data.pubDate.valueOf() - a.data.pubDate.valueOf());
}

export function getKbEntryBySlug(slug: string): ContentEntry | undefined {
  return kbEntries.find((entry) => entry.slug === slug);
}
