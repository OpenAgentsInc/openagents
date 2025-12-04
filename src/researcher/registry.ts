/**
 * Papers Registry Management
 *
 * Manages the papers.jsonl registry for tracking paper discovery and summarization.
 */
import type { PaperPriority, PaperStatus } from "./index.js";

// ============================================================================
// Types
// ============================================================================

/**
 * A paper record in the registry.
 */
export interface PaperRecord {
  /** Unique ID (e.g., "paper-a-mem-2025") */
  id: string;
  /** Paper title */
  title: string;
  /** Authors (optional) */
  authors?: string;
  /** Publication year (optional) */
  year?: number;
  /** URLs to paper (arXiv, DOI, conference, etc.) */
  urls: string[];
  /** Priority level */
  priority: PaperPriority;
  /** Current status */
  status: PaperStatus;
  /** Source document that requested this paper */
  sourceDoc: string;
  /** Path to generated summary (if complete) */
  summaryPath?: string;
  /** Associated task ID (if task integration enabled) */
  taskId?: string;
  /** ISO timestamp when created */
  createdAt: string;
  /** ISO timestamp when last updated */
  updatedAt: string;
  /** Error message (if failed) */
  error?: string;
}

/**
 * Input for creating a new paper record.
 */
export interface PaperCreate {
  title: string;
  authors?: string;
  year?: number;
  urls?: string[];
  priority: PaperPriority;
  sourceDoc: string;
}

/**
 * Input for updating a paper record.
 */
export interface PaperUpdate {
  status?: PaperStatus;
  summaryPath?: string;
  taskId?: string;
  error?: string;
  urls?: string[];
}

/**
 * Registry statistics.
 */
export interface RegistryStats {
  total: number;
  pending: number;
  processing: number;
  complete: number;
  failed: number;
  byPriority: Record<PaperPriority, number>;
}

// ============================================================================
// ID Generation
// ============================================================================

/**
 * Generate a slug from a paper title.
 */
const slugify = (title: string): string => {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .slice(0, 50)
    .replace(/-$/, "");
};

/**
 * Generate a unique ID for a paper.
 */
const generatePaperId = (title: string, year?: number, existingIds: string[] = []): string => {
  const slug = slugify(title);
  const yearSuffix = year ? `-${year}` : "";
  const baseId = `paper-${slug}${yearSuffix}`;

  if (!existingIds.includes(baseId)) {
    return baseId;
  }

  // Add counter if ID exists
  let counter = 2;
  while (existingIds.includes(`${baseId}-${counter}`)) {
    counter++;
  }
  return `${baseId}-${counter}`;
};

// ============================================================================
// Registry Operations
// ============================================================================

/**
 * Load the papers registry from disk.
 */
export const loadRegistry = async (registryPath: string): Promise<PaperRecord[]> => {
  try {
    const file = Bun.file(registryPath);
    if (!(await file.exists())) {
      return [];
    }

    const content = await file.text();
    const lines = content.split("\n").filter((line) => line.trim().length > 0);

    return lines.map((line) => JSON.parse(line) as PaperRecord);
  } catch {
    return [];
  }
};

/**
 * Save the papers registry to disk.
 */
export const saveRegistry = async (
  registryPath: string,
  registry: PaperRecord[]
): Promise<void> => {
  const content = registry.map((r) => JSON.stringify(r)).join("\n") + "\n";
  await Bun.write(registryPath, content);
};

/**
 * Add a new paper to the registry.
 */
export const addPaper = async (
  registryPath: string,
  paper: PaperCreate
): Promise<PaperRecord> => {
  const registry = await loadRegistry(registryPath);
  const existingIds = registry.map((r) => r.id);

  const now = new Date().toISOString();
  const record: PaperRecord = {
    id: generatePaperId(paper.title, paper.year, existingIds),
    title: paper.title,
    urls: paper.urls ?? [],
    priority: paper.priority,
    status: "pending",
    sourceDoc: paper.sourceDoc,
    createdAt: now,
    updatedAt: now,
  };
  // Only add optional properties if defined
  if (paper.authors) record.authors = paper.authors;
  if (paper.year) record.year = paper.year;

  registry.push(record);
  await saveRegistry(registryPath, registry);

  return record;
};

/**
 * Update an existing paper in the registry.
 */
export const updatePaper = async (
  registryPath: string,
  id: string,
  update: PaperUpdate
): Promise<PaperRecord | null> => {
  const registry = await loadRegistry(registryPath);
  const index = registry.findIndex((r) => r.id === id);

  if (index === -1) {
    return null;
  }

  const record = registry[index];
  const now = new Date().toISOString();

  if (update.status !== undefined) record.status = update.status;
  if (update.summaryPath !== undefined) record.summaryPath = update.summaryPath;
  if (update.taskId !== undefined) record.taskId = update.taskId;
  if (update.error !== undefined) record.error = update.error;
  if (update.urls !== undefined) record.urls = update.urls;
  record.updatedAt = now;

  registry[index] = record;
  await saveRegistry(registryPath, registry);

  return record;
};

/**
 * Find a paper by title (case-insensitive partial match).
 */
export const findPaperByTitle = (
  registry: PaperRecord[],
  title: string
): PaperRecord | undefined => {
  const normalized = title.toLowerCase();
  return registry.find((r) => r.title.toLowerCase().includes(normalized));
};

/**
 * Find a paper by ID.
 */
export const findPaperById = (
  registry: PaperRecord[],
  id: string
): PaperRecord | undefined => {
  return registry.find((r) => r.id === id);
};

/**
 * Get all pending papers.
 */
export const getPendingPapers = (registry: PaperRecord[]): PaperRecord[] => {
  return registry.filter((r) => r.status === "pending");
};

/**
 * Get registry statistics.
 */
export const getRegistryStats = async (registryPath: string): Promise<RegistryStats> => {
  const registry = await loadRegistry(registryPath);

  const stats: RegistryStats = {
    total: registry.length,
    pending: 0,
    processing: 0,
    complete: 0,
    failed: 0,
    byPriority: { HIGH: 0, MEDIUM: 0, LOW: 0 },
  };

  for (const paper of registry) {
    switch (paper.status) {
      case "pending":
        stats.pending++;
        break;
      case "processing":
        stats.processing++;
        break;
      case "complete":
        stats.complete++;
        break;
      case "failed":
        stats.failed++;
        break;
    }
    stats.byPriority[paper.priority]++;
  }

  return stats;
};
