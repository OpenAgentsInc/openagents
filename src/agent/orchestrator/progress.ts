/**
 * Progress File Infrastructure
 * 
 * Enables cross-session coordination by reading/writing progress.md.
 * Following Anthropic's "Effective Harnesses for Long-Running Agents" pattern:
 * - Write orientation summary after understanding repo state
 * - Track current session work (subtasks, files, tests)
 * - Leave instructions for next session
 */
import * as fs from "node:fs";
import { type SessionProgress, getProgressPath } from "./types.js";

const compressOutput = (output: string, maxLength = 500): string => {
  const condensed = output.replace(/\s+/g, " ").trim();
  if (condensed.length <= maxLength) return condensed;
  return `${condensed.slice(0, maxLength)}...`;
};

/**
 * Write progress file for next session to read.
 * Uses markdown format for human readability.
 */
export const writeProgress = (openagentsDir: string, progress: SessionProgress): void => {
  const progressPath = getProgressPath(openagentsDir);
  
  const markdown = formatProgressMarkdown(progress);
  fs.writeFileSync(progressPath, markdown);
};

/**
 * Format SessionProgress as markdown
 */
export const formatProgressMarkdown = (progress: SessionProgress): string => {
  const lines: string[] = [
    "# Session Progress",
    "",
    "## Session Info",
    `- **Session ID**: ${progress.sessionId}`,
    `- **Started**: ${progress.startedAt}`,
    `- **Task**: ${progress.taskId} - ${progress.taskTitle}`,
    "",
    "## Orientation",
    `- **Repo State**: ${progress.orientation.repoState}`,
    `- **Tests Passing at Start**: ${progress.orientation.testsPassingAtStart ? "Yes" : "No"}`,
  ];

  if (progress.orientation.initScript) {
    const init = progress.orientation.initScript;
    lines.push(`- **Init Script**: ${init.ran ? (init.success ? "Success" : "Failed") : "Not Found"}`);

    if (init.output) {
      lines.push(`- **Init Output**: ${compressOutput(init.output)}`);
    }
  }

  if (progress.orientation.previousSessionSummary) {
    lines.push(`- **Previous Session**: ${progress.orientation.previousSessionSummary}`);
  }

  lines.push(
    "",
    "## Work Done",
    `- **Subtasks Completed**: ${progress.work.subtasksCompleted.length > 0 ? progress.work.subtasksCompleted.join(", ") : "None"}`,
    `- **Subtasks In Progress**: ${progress.work.subtasksInProgress.length > 0 ? progress.work.subtasksInProgress.join(", ") : "None"}`,
    `- **Files Modified**: ${progress.work.filesModified.length > 0 ? progress.work.filesModified.join(", ") : "None"}`,
    `- **Tests Run**: ${progress.work.testsRun ? "Yes" : "No"}`,
    `- **Tests Passing After Work**: ${progress.work.testsPassingAfterWork ? "Yes" : "No"}`,
  );

  // Add Claude Code session metadata if present
  if (progress.work.claudeCodeSession) {
    lines.push("", "### Claude Code Session");

    if (progress.work.claudeCodeSession.toolsUsed) {
      const tools = Object.entries(progress.work.claudeCodeSession.toolsUsed)
        .map(([tool, count]) => `${tool}(${count})`)
        .join(", ");
      lines.push(`- **Tools Used**: ${tools}`);
    }

    if (progress.work.claudeCodeSession.summary) {
      lines.push(`- **Summary**: ${progress.work.claudeCodeSession.summary}`);
    }
  }

  lines.push(
    "",
    "## Next Session Should",
  );

  if (progress.nextSession.suggestedNextSteps.length > 0) {
    for (const step of progress.nextSession.suggestedNextSteps) {
      lines.push(`- ${step}`);
    }
  } else {
    lines.push("- Continue with next task");
  }

  if (progress.nextSession.blockers && progress.nextSession.blockers.length > 0) {
    lines.push("", "### Blockers");
    for (const blocker of progress.nextSession.blockers) {
      lines.push(`- ${blocker}`);
    }
  }

  if (progress.nextSession.notes) {
    lines.push("", "### Notes", progress.nextSession.notes);
  }

  lines.push(
    "",
    "---",
    `Completed: ${progress.completedAt || "In Progress"}`,
  );

  return lines.join("\n");
};

/**
 * Parse a markdown progress file back into SessionProgress.
 * Extracts key information for context bridging.
 */
export const parseProgressMarkdown = (markdown: string): Partial<SessionProgress> => {
  const result: Partial<SessionProgress> = {
    orientation: {
      repoState: "",
      testsPassingAtStart: false,
    },
    work: {
      subtasksCompleted: [],
      subtasksInProgress: [],
      filesModified: [],
      testsRun: false,
      testsPassingAfterWork: false,
    },
    nextSession: {
      suggestedNextSteps: [],
    },
  };

  const lines = markdown.split("\n");
  let currentSection = "";

  for (const line of lines) {
    const trimmed = line.trim();

    // Detect section headers (## for main sections, ### for subsections)
    if (trimmed.startsWith("### ")) {
      currentSection = trimmed.slice(4).toLowerCase();
      continue;
    }
    if (trimmed.startsWith("## ")) {
      currentSection = trimmed.slice(3).toLowerCase();
      continue;
    }

    // Parse key-value pairs
    if (trimmed.startsWith("- **")) {
      const match = trimmed.match(/^- \*\*(.+?)\*\*:\s*(.*)$/);
      if (match) {
        const [, key, value] = match;
        parseKeyValue(result, currentSection, key.toLowerCase(), value);
      }
      continue;
    }

    // Parse list items in "Next Session Should" section
    if (currentSection === "next session should" && trimmed.startsWith("- ")) {
      const step = trimmed.slice(2);
      result.nextSession!.suggestedNextSteps.push(step);
      continue;
    }

    // Parse blockers
    if (currentSection === "blockers" && trimmed.startsWith("- ")) {
      const blocker = trimmed.slice(2);
      if (!result.nextSession!.blockers) {
        result.nextSession!.blockers = [];
      }
      result.nextSession!.blockers.push(blocker);
      continue;
    }

    // Parse completed timestamp
    if (trimmed.startsWith("Completed:")) {
      const value = trimmed.slice("Completed:".length).trim();
      if (value !== "In Progress") {
        result.completedAt = value;
      }
    }
  }

  return result;
};

/**
 * Parse a key-value pair into the result object
 */
const parseKeyValue = (
  result: Partial<SessionProgress>,
  section: string,
  key: string,
  value: string
): void => {
  const trimmedValue = value.trim();

  switch (section) {
    case "session info":
      if (key === "session id") result.sessionId = trimmedValue;
      if (key === "started") result.startedAt = trimmedValue;
      if (key === "task") {
        const taskMatch = trimmedValue.match(/^(\S+)\s*-\s*(.*)$/);
        if (taskMatch) {
          result.taskId = taskMatch[1];
          result.taskTitle = taskMatch[2];
        }
      }
      break;

    case "orientation":
      if (key === "repo state") result.orientation!.repoState = trimmedValue;
      if (key === "tests passing at start") {
        result.orientation!.testsPassingAtStart = trimmedValue.toLowerCase() === "yes";
      }
      if (key === "previous session") {
        result.orientation!.previousSessionSummary = trimmedValue;
      }
      if (key === "init script") {
        const init =
          result.orientation!.initScript ?? {
            ran: false,
            success: true,
          };
        const normalized = trimmedValue.toLowerCase();
        if (normalized.includes("success")) {
          init.ran = true;
          init.success = true;
        } else if (normalized.includes("failed")) {
          init.ran = true;
          init.success = false;
        } else {
          init.ran = false;
          init.success = true;
        }
        result.orientation!.initScript = init;
      }
      if (key === "init output") {
        const init =
          result.orientation!.initScript ?? {
            ran: false,
            success: true,
          };
        init.output = trimmedValue;
        result.orientation!.initScript = init;
      }
      break;

    case "work done":
      if (key === "subtasks completed") {
        result.work!.subtasksCompleted = parseList(trimmedValue);
      }
      if (key === "subtasks in progress") {
        result.work!.subtasksInProgress = parseList(trimmedValue);
      }
      if (key === "files modified") {
        result.work!.filesModified = parseList(trimmedValue);
      }
      if (key === "tests run") {
        result.work!.testsRun = trimmedValue.toLowerCase() === "yes";
      }
      if (key === "tests passing after work") {
        result.work!.testsPassingAfterWork = trimmedValue.toLowerCase() === "yes";
      }
      break;
  }
};

/**
 * Parse a comma-separated list or "None"
 */
const parseList = (value: string): string[] => {
  if (value.toLowerCase() === "none" || !value) return [];
  return value.split(",").map((s) => s.trim()).filter(Boolean);
};

/**
 * Read the previous session's progress file if it exists.
 * Returns parsed progress for context bridging.
 */
export const readProgress = (openagentsDir: string): Partial<SessionProgress> | null => {
  const progressPath = getProgressPath(openagentsDir);
  if (!fs.existsSync(progressPath)) return null;

  try {
    const content = fs.readFileSync(progressPath, "utf-8");
    return parseProgressMarkdown(content);
  } catch {
    return null;
  }
};

/**
 * Check if a progress file exists
 */
export const progressExists = (openagentsDir: string): boolean => {
  const progressPath = getProgressPath(openagentsDir);
  return fs.existsSync(progressPath);
};

/**
 * Get a summary of the previous session for context bridging.
 * Returns a concise string suitable for including in prompts.
 */
export const getPreviousSessionSummary = (openagentsDir: string): string | null => {
  const progress = readProgress(openagentsDir);
  if (!progress) return null;

  const parts: string[] = [];

  if (progress.taskId && progress.taskTitle) {
    parts.push(`Previous task: ${progress.taskId} - ${progress.taskTitle}`);
  }

  if (progress.work?.subtasksCompleted && progress.work.subtasksCompleted.length > 0) {
    parts.push(`Completed: ${progress.work.subtasksCompleted.join(", ")}`);
  }

  if (progress.work?.subtasksInProgress && progress.work.subtasksInProgress.length > 0) {
    parts.push(`In progress: ${progress.work.subtasksInProgress.join(", ")}`);
  }

  if (progress.nextSession?.blockers && progress.nextSession.blockers.length > 0) {
    parts.push(`Blockers: ${progress.nextSession.blockers.join(", ")}`);
  }

  if (progress.nextSession?.suggestedNextSteps && progress.nextSession.suggestedNextSteps.length > 0) {
    parts.push(`Next steps: ${progress.nextSession.suggestedNextSteps.join("; ")}`);
  }

  return parts.length > 0 ? parts.join("\n") : null;
};

/**
 * Create an empty SessionProgress object
 */
export const createEmptyProgress = (sessionId: string, taskId: string, taskTitle: string): SessionProgress => ({
  sessionId,
  startedAt: new Date().toISOString(),
  taskId,
  taskTitle,
  orientation: {
    repoState: "",
    testsPassingAtStart: false,
  },
  work: {
    subtasksCompleted: [],
    subtasksInProgress: [],
    filesModified: [],
    testsRun: false,
    testsPassingAfterWork: false,
  },
  nextSession: {
    suggestedNextSteps: [],
  },
});
