/**
 * Research Prompt Templates
 *
 * Templates for instructing Claude Code to find and summarize academic papers.
 */
import type { ResearchRequest } from "./index.js";

/**
 * Build the research prompt for Claude Code.
 *
 * This prompt instructs Claude Code to:
 * 1. Use WebSearch to find the paper
 * 2. Use WebFetch to retrieve content
 * 3. Generate a structured summary
 * 4. Write the summary to the output directory
 */
export const buildResearchPrompt = (
  request: ResearchRequest,
  outputDir: string
): string => {
  const searchHints: string[] = [];
  if (request.authors) searchHints.push(`Authors: ${request.authors}`);
  if (request.year) searchHints.push(`Year: ${request.year}`);
  if (request.urls?.length) {
    searchHints.push(`Direct URLs to analyze:\n${request.urls.map((u) => `  - ${u}`).join("\n")}`);
  }

  const hintsSection = searchHints.length > 0
    ? `\n### Additional Context\n${searchHints.join("\n")}\n`
    : "";

  return `## Research Task: Find and Summarize Paper

You are a research assistant helping to discover and summarize academic papers for the OpenAgents project.

### Paper to Research
**Query:** ${request.query}
**Priority:** ${request.priority ?? "MEDIUM"}
${hintsSection}
### Your Capabilities
- Use **WebSearch** to find papers by title, topic, or author
- Use **WebFetch** to read paper abstracts, full text, and metadata from arXiv, conference pages, etc.
- Use **Write** to create the summary markdown file

### Research Process

1. **Search for the paper**
   - Use WebSearch with the paper title and any author/year hints
   - Look for official sources: arXiv, ACL Anthology, IEEE, ACM DL, OpenReview
   - If you have direct URLs, use those first

2. **Retrieve paper content**
   - Use WebFetch to read the abstract and paper content
   - For arXiv papers, the HTML version (arxiv.org/abs/...) is readable
   - Note: PDFs may not be directly readable, use HTML/abstract pages

3. **Generate structured summary**
   - Follow the template below EXACTLY
   - Focus on extracting actionable insights for autonomous coding agents
   - Note any code implementations or demos mentioned

4. **Write the summary file**
   - Output path: ${outputDir}/<slug>-summary.md
   - The slug should be lowercase, hyphenated (e.g., "a-mem-summary.md")

### Summary Template (REQUIRED FORMAT)

\`\`\`markdown
# [Paper Title]

**Paper Summary**

- **Authors:** [Full author names]
- **Institutions:** [List of institutions]
- **Published:** [Venue, Year]
- **arXiv:** [arXiv ID if available]
- **Source:** [URL to official page]

---

## Executive Summary

[2-3 sentences summarizing the main contribution]

---

## Core Contribution

[1 paragraph explaining the key novelty]

---

## Architecture / Methodology

[Detailed explanation of the approach, with ASCII diagrams if helpful]

---

## Key Results

| Metric | Result |
|--------|--------|
| [Metric 1] | [Value] |
| [Metric 2] | [Value] |

---

## Limitations and Future Work

[Bullet points of limitations mentioned]

---

## Relevance to MechaCoder

[CRITICAL SECTION: How this research applies to autonomous coding agents]

- **Applicable patterns:** [What can we use?]
- **Implementation considerations:** [How to adapt?]
- **Potential value:** [What problem does it solve for us?]

---

## Citation

\\\`\`\`bibtex
@article{...}
\\\`\`\`
\`\`\`

### Important Notes

- If you cannot find the paper, report that clearly and suggest alternative search terms
- Focus on **actionable insights** for building autonomous coding agents
- The "Relevance to MechaCoder" section is the most important - be specific about how ideas apply
- Include any links to code repositories or demos
- If the paper builds on other foundational work, briefly mention those connections

### Output

After completing the research:
1. Report the path of the summary file you created
2. Summarize the key takeaway in 1-2 sentences
3. Note any follow-up papers that might be worth researching
`;
};

/**
 * Build a batch research prompt for processing multiple papers.
 */
export const buildBatchResearchPrompt = (
  requests: ResearchRequest[],
  outputDir: string
): string => {
  const paperList = requests
    .map((r, i) => `${i + 1}. **${r.query}** (${r.priority ?? "MEDIUM"})${r.urls?.length ? ` - URL: ${r.urls[0]}` : ""}`)
    .join("\n");

  return `## Batch Research Task

You are researching multiple papers for the OpenAgents project.

### Papers to Process
${paperList}

### Instructions
For each paper:
1. Search and retrieve the paper content
2. Generate a structured summary following the standard template
3. Write to ${outputDir}/<slug>-summary.md

Process papers in priority order (HIGH first).
Report progress after each paper is completed.

### Output Format
After each paper, report:
- File created: <path>
- Key takeaway: <1 sentence>
- Status: complete/failed

At the end, summarize: X of Y papers processed successfully.
`;
};
