/**
 * Reflection Document Parser
 *
 * Parses paper requests from analysis/reflection markdown documents.
 * Supports both table format and prose format.
 */
import type { PaperPriority } from "./index.js";

// ============================================================================
// Types
// ============================================================================

/**
 * A parsed paper request from a reflection document.
 */
export interface ParsedPaperRequest {
  /** Paper title */
  title: string;
  /** Publication year (if found) */
  year?: number;
  /** URL to paper (if found) */
  url?: string;
  /** Priority level */
  priority: PaperPriority;
  /** Line number in source document */
  sourceLine?: number;
}

// ============================================================================
// Table Parser
// ============================================================================

/**
 * Parse a markdown table for paper requests.
 *
 * Expected format:
 * | Paper | Year | URL | Priority |
 * |-------|------|-----|----------|
 * | A-Mem: Agentic Memory | 2025 | arxiv.org/abs/... | HIGH |
 */
const parseTable = (content: string, startLine: number): ParsedPaperRequest[] => {
  const results: ParsedPaperRequest[] = [];
  const lines = content.split("\n");

  // Find table header patterns
  const headerPatterns = [
    /\|\s*paper\s*\|/i,
    /\|\s*title\s*\|/i,
    /\|\s*name\s*\|/i,
  ];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Check if this looks like a table header
    if (!headerPatterns.some((p) => p.test(line))) continue;

    // Parse header to find column indices
    const headerCells = line.split("|").map((c) => c.trim().toLowerCase());
    const titleIdx = headerCells.findIndex((c) =>
      ["paper", "title", "name"].includes(c)
    );
    const yearIdx = headerCells.findIndex((c) => c === "year");
    const urlIdx = headerCells.findIndex((c) =>
      ["url", "link", "source"].includes(c)
    );
    const priorityIdx = headerCells.findIndex((c) =>
      ["priority", "pri", "importance"].includes(c)
    );

    if (titleIdx === -1) continue;

    // Skip separator line
    if (i + 1 < lines.length && /^\|[\s-:|]+\|$/.test(lines[i + 1])) {
      i++;
    }

    // Parse data rows
    for (let j = i + 1; j < lines.length; j++) {
      const row = lines[j];
      if (!row.startsWith("|") || row.trim() === "") break;

      const cells = row.split("|").map((c) => c.trim());

      const title = cells[titleIdx];
      if (!title || title === "" || title === "-") continue;

      const year = yearIdx !== -1 ? parseInt(cells[yearIdx], 10) : undefined;
      const url = urlIdx !== -1 ? cells[urlIdx] : undefined;
      const priorityStr = priorityIdx !== -1 ? cells[priorityIdx]?.toUpperCase() : "MEDIUM";

      const priority: PaperPriority =
        priorityStr === "HIGH" || priorityStr === "H"
          ? "HIGH"
          : priorityStr === "LOW" || priorityStr === "L"
            ? "LOW"
            : "MEDIUM";

      const paperRequest: ParsedPaperRequest = {
        title,
        priority,
        sourceLine: startLine + j,
      };
      if (year && !Number.isNaN(year)) paperRequest.year = year;
      if (url && url !== "-" && url !== "") paperRequest.url = normalizeUrl(url);
      results.push(paperRequest);
    }

    // Only parse first table found
    break;
  }

  return results;
};

// ============================================================================
// Prose Parser
// ============================================================================

/**
 * Parse prose sections for paper requests.
 *
 * Expected patterns:
 * - **Paper Title** (HIGH) - Description
 * - Paper Title (2023, HIGH)
 * - "Paper Title" [HIGH]
 */
const parseProse = (content: string, startLine: number): ParsedPaperRequest[] => {
  const results: ParsedPaperRequest[] = [];
  const lines = content.split("\n");

  // Section header patterns that indicate paper requests
  const sectionPatterns = [
    /papers?\s+to\s+request/i,
    /papers?\s+to\s+read/i,
    /papers?\s+needed/i,
    /acquire\s+pdfs?/i,
    /recommended\s+papers?/i,
    /suggested\s+reading/i,
  ];

  let inSection = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Check for section header
    if (sectionPatterns.some((p) => p.test(line))) {
      inSection = true;
      continue;
    }

    // End section on empty line after content or new header
    if (inSection && (line.startsWith("#") || (line.trim() === "" && i > 0 && lines[i - 1].trim() === ""))) {
      inSection = false;
      continue;
    }

    if (!inSection) continue;

    // Parse list items: - **Title** (PRIORITY) or - Title (YEAR, PRIORITY)
    const listMatch = line.match(/^[\s-*]+\*?\*?([^*]+)\*?\*?\s*\(([^)]+)\)/);
    if (listMatch) {
      const title = listMatch[1].trim();
      const parenContent = listMatch[2];

      // Parse parenthetical content
      const yearMatch = parenContent.match(/\b(19|20)\d{2}\b/);
      const year = yearMatch ? parseInt(yearMatch[0], 10) : undefined;

      const priorityMatch = parenContent.match(/\b(HIGH|MEDIUM|LOW|H|M|L)\b/i);
      const priority: PaperPriority = priorityMatch
        ? priorityMatch[1].toUpperCase().startsWith("H")
          ? "HIGH"
          : priorityMatch[1].toUpperCase().startsWith("L")
            ? "LOW"
            : "MEDIUM"
        : "MEDIUM";

      if (title.length > 5) {
        const paperRequest: ParsedPaperRequest = {
          title,
          priority,
          sourceLine: startLine + i,
        };
        if (year) paperRequest.year = year;
        results.push(paperRequest);
      }
    }

    // Also try matching quoted titles: "Paper Title" [HIGH]
    const quotedMatch = line.match(/["""]([^"""]+)["""].*\[(HIGH|MEDIUM|LOW|H|M|L)\]/i);
    if (quotedMatch) {
      const title = quotedMatch[1].trim();
      const priorityStr = quotedMatch[2].toUpperCase();
      const priority: PaperPriority = priorityStr.startsWith("H")
        ? "HIGH"
        : priorityStr.startsWith("L")
          ? "LOW"
          : "MEDIUM";

      if (title.length > 5) {
        results.push({
          title,
          priority,
          sourceLine: startLine + i,
        });
      }
    }
  }

  return results;
};

// ============================================================================
// URL Normalization
// ============================================================================

/**
 * Normalize a URL (add https:// if missing).
 */
const normalizeUrl = (url: string): string => {
  if (!url) return url;
  url = url.trim();

  if (url.startsWith("http://") || url.startsWith("https://")) {
    return url;
  }

  // Handle arxiv short format
  if (url.startsWith("arxiv.org") || url.startsWith("www.arxiv.org")) {
    return `https://${url}`;
  }

  // Default to https://
  return `https://${url}`;
};

// ============================================================================
// Main Parser
// ============================================================================

/**
 * Parse a reflection document for paper requests.
 *
 * Tries both table and prose parsing, deduplicates by title.
 */
export const parseReflectionDoc = async (
  docPath: string
): Promise<ParsedPaperRequest[]> => {
  const file = Bun.file(docPath);
  if (!(await file.exists())) {
    return [];
  }

  const content = await file.text();

  // Parse with both methods
  const tableResults = parseTable(content, 0);
  const proseResults = parseProse(content, 0);

  // Combine and deduplicate by title
  const allResults = [...tableResults, ...proseResults];
  const seen = new Set<string>();
  const deduplicated: ParsedPaperRequest[] = [];

  for (const result of allResults) {
    const normalizedTitle = result.title.toLowerCase().trim();
    if (!seen.has(normalizedTitle)) {
      seen.add(normalizedTitle);
      deduplicated.push(result);
    }
  }

  return deduplicated;
};

/**
 * Parse paper requests from a markdown string.
 */
export const parseReflectionContent = (content: string): ParsedPaperRequest[] => {
  const tableResults = parseTable(content, 0);
  const proseResults = parseProse(content, 0);

  const allResults = [...tableResults, ...proseResults];
  const seen = new Set<string>();
  const deduplicated: ParsedPaperRequest[] = [];

  for (const result of allResults) {
    const normalizedTitle = result.title.toLowerCase().trim();
    if (!seen.has(normalizedTitle)) {
      seen.add(normalizedTitle);
      deduplicated.push(result);
    }
  }

  return deduplicated;
};
