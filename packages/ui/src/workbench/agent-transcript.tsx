import {
  Brain,
  FileText,
  MessageSquare,
  Terminal,
  TriangleAlert,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import type { ReactElement } from "react";

import { activityStatusIcon, activityStatusLabel } from "./activity-status.tsx";

export type DesktopAgentTranscriptActivity = Readonly<
  | {
      kind: "command";
      label: string;
      status: "running" | "completed" | "failed";
      outputByteCount?: number;
    }
  | {
      kind: "file_change";
      label: string;
      status: "running" | "completed" | "failed";
      fileChangeCount: number;
      outputByteCount?: number;
    }
  | {
      kind: "tool";
      label: string;
      status: "running" | "completed" | "failed";
      outputByteCount?: number;
    }
  | { kind: "reasoning"; label: string; status: "running" | "completed" | "failed" }
  | { kind: "notice"; label: string; status: "running" | "completed" | "failed" }
>;

export type DesktopAgentTranscriptLine = Readonly<{
  entryKey?: string;
  label: string;
  text: string;
  activity?: DesktopAgentTranscriptActivity;
}>;

const activityIcon = (kind: DesktopAgentTranscriptActivity["kind"]): LucideIcon => {
  switch (kind) {
    case "command":
      return Terminal;
    case "file_change":
      return FileText;
    case "tool":
      return Wrench;
    case "reasoning":
      return Brain;
    case "notice":
      return TriangleAlert;
  }
};

const formatBytes = (value: number): string => {
  const bytes = Math.max(0, Math.floor(value));
  if (bytes < 1_000) return `${bytes} B`;
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(bytes < 10_000 ? 1 : 0)} KB`;
  return `${(bytes / 1_000_000).toFixed(bytes < 10_000_000 ? 1 : 0)} MB`;
};

const activityMetadata = (activity: DesktopAgentTranscriptActivity): string => {
  const values: Array<string> = [];
  if (activity.kind === "file_change") {
    values.push(`${activity.fileChangeCount} ${activity.fileChangeCount === 1 ? "file" : "files"}`);
  }
  if ("outputByteCount" in activity && activity.outputByteCount !== undefined) {
    values.push(`${formatBytes(activity.outputByteCount)} output`);
  }
  return values.join(" · ");
};

const DesktopAgentTranscriptRow = ({
  line,
}: Readonly<{
  line: DesktopAgentTranscriptLine;
}>): ReactElement => {
  const activity = line.activity;
  const Icon = activity === undefined ? MessageSquare : activityIcon(activity.kind);
  const label = activity?.label ?? line.label;
  const metadata = activity === undefined ? "" : activityMetadata(activity);
  const preview =
    activity === undefined || activity.kind === "reasoning" || activity.kind === "notice"
      ? line.text
      : metadata === "" && line.text !== label
        ? line.text
        : "";
  return (
    <div
      aria-label={`${label}${preview === "" ? "" : `: ${preview}`}`}
      className="oa-react-agent-work-row"
      data-work-kind={activity?.kind ?? "message"}
      role="listitem"
    >
      <Icon aria-hidden="true" />
      <span className="oa-react-agent-work-copy">
        <strong>{label}</strong>
        {preview === "" ? null : <span title={preview}>{preview}</span>}
      </span>
      {metadata === "" ? null : <small>{metadata}</small>}
      {activity === undefined ? null : (
        <span className="oa-react-event-status" data-status={activity.status}>
          {activityStatusIcon(activity.status)}
          {activityStatusLabel(activity.status)}
        </span>
      )}
    </div>
  );
};

const VISIBLE_WORK_ROW_COUNT = 8;

export const DesktopAgentTranscript = ({
  lines,
}: Readonly<{
  lines: ReadonlyArray<DesktopAgentTranscriptLine>;
}>): ReactElement => {
  const earlierCount = Math.max(0, lines.length - VISIBLE_WORK_ROW_COUNT);
  const earlier = lines.slice(0, earlierCount);
  const visible = lines.slice(earlierCount);
  const row = (line: DesktopAgentTranscriptLine, index: number): ReactElement => (
    <DesktopAgentTranscriptRow key={line.entryKey ?? `${line.label}:${index}`} line={line} />
  );
  return (
    <div className="oa-react-agent-work-list" role="list">
      {earlier.length === 0 ? null : (
        <details className="oa-react-agent-work-history" role="listitem">
          <summary>
            Show {earlier.length} earlier {earlier.length === 1 ? "update" : "updates"}
          </summary>
          <div role="list">{earlier.map(row)}</div>
        </details>
      )}
      {visible.map((line, index) => row(line, earlierCount + index))}
    </div>
  );
};
