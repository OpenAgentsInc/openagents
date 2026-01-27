import { html } from "../../effuse/template/html"
import {
  Agent,
  AgentContent,
  AgentHeader,
  AgentInstructions,
  AgentOutput,
  AgentTool,
  AgentTools,
} from "./agent"
import { Alert } from "./alert"
import {
  Artifact,
  ArtifactAction,
  ArtifactActions,
  ArtifactContent,
  ArtifactDescription,
  ArtifactHeader,
  ArtifactTitle,
} from "./artifact"
import {
  Attachments,
  Attachment,
  AttachmentEmpty,
  AttachmentInfo,
  AttachmentPreview,
  AttachmentRemove,
} from "./attachments"
import {
  AudioPlayer,
  AudioPlayerControlBar,
  AudioPlayerDurationDisplay,
  AudioPlayerElement,
  AudioPlayerMuteButton,
  AudioPlayerPlayButton,
  AudioPlayerSeekBackwardButton,
  AudioPlayerSeekForwardButton,
  AudioPlayerTimeDisplay,
  AudioPlayerTimeRange,
  AudioPlayerVolumeRange,
} from "./audio-player"
import {
  ChainOfThought,
  ChainOfThoughtContent,
  ChainOfThoughtHeader,
  ChainOfThoughtSearchResult,
  ChainOfThoughtSearchResults,
  ChainOfThoughtStep,
} from "./chain-of-thought"
import {
  Checkpoint,
  CheckpointIcon,
  CheckpointTrigger,
} from "./checkpoint"
import {
  CodeBlock,
  CodeBlockActions,
  CodeBlockCopyButton,
  CodeBlockHeader,
  CodeBlockTitle,
} from "./code-block"
import {
  Commit,
  CommitActions,
  CommitAuthor,
  CommitAuthorAvatar,
  CommitContent,
  CommitCopyButton,
  CommitFile,
  CommitFileIcon,
  CommitFileInfo,
  CommitFilePath,
  CommitFileStatus,
  CommitFiles,
  CommitHeader,
  CommitInfo,
  CommitMessage,
  CommitMetadata,
  CommitSeparator,
  CommitTimestamp,
} from "./commit"
import {
  Confirmation,
  ConfirmationAction,
  ConfirmationActions,
  ConfirmationRequest,
  ConfirmationTitle,
} from "./confirmation"
import { Context } from "./context"
import { Controls } from "./controls"
import {
  EnvironmentVariable,
  EnvironmentVariableCopyButton,
  EnvironmentVariableName,
  EnvironmentVariableRequired,
  EnvironmentVariableValue,
  EnvironmentVariables,
  EnvironmentVariablesContent,
  EnvironmentVariablesHeader,
  EnvironmentVariablesTitle,
  EnvironmentVariablesToggle,
} from "./environment-variables"
import {
  FileTree,
  FileTreeFile,
  FileTreeFolder,
  FileTreeIcon,
  FileTreeName,
} from "./file-tree"
import { Image } from "./image"
import {
  InlineCitation,
  InlineCitationCard,
  InlineCitationCardBody,
  InlineCitationCardTrigger,
  InlineCitationCarousel,
  InlineCitationCarouselContent,
  InlineCitationCarouselItem,
  InlineCitationCarouselNext,
  InlineCitationCarouselPrev,
  InlineCitationQuote,
  InlineCitationSource,
  InlineCitationText,
} from "./inline-citation"
import { Loader } from "./loader"
import {
  OpenIn,
  OpenInContent,
  OpenInItem,
  OpenInLabel,
  OpenInSeparator,
  OpenInTrigger,
} from "./open-in-chat"
import {
  PackageInfo,
  PackageInfoContent,
  PackageInfoDependencies,
  PackageInfoDependency,
  PackageInfoDescription,
} from "./package-info"
import { Panel as ShellPanel } from "./panel"
import { Persona } from "./persona"
import {
  QueueItem,
  QueueItemActions,
  QueueItemAction,
  QueueItemAttachment,
  QueueItemContent,
  QueueItemDescription,
  QueueItemFile,
  QueueItemImage,
  QueueItemIndicator,
  QueueList,
  QueueSection,
  QueueSectionContent,
  QueueSectionHeader,
} from "./queue"
import {
  Sandbox,
  SandboxContent,
  SandboxHeader,
  SandboxTabContent,
  SandboxTabs,
  SandboxTabsBar,
  SandboxTabsList,
  SandboxTabsTrigger,
} from "./sandbox"
import {
  SchemaDisplay,
} from "./schema-display"
import { Shimmer } from "./shimmer"
import { Snippet } from "./snippet"
import {
  Source,
  Sources,
  SourcesContent,
  SourcesTrigger,
} from "./sources"
import {
  StackTrace,
  StackTraceCopyButton,
  StackTraceError,
  StackTraceErrorMessage,
  StackTraceHeader,
} from "./stack-trace"
import {
  Task,
  TaskContent,
  TaskItem,
  TaskItemFile,
  TaskTrigger,
} from "./task"
import {
  Terminal,
  TerminalActions,
  TerminalContent,
  TerminalCopyButton,
  TerminalHeader,
  TerminalTitle,
} from "./terminal"
import {
  TestResults,
  TestResultsContent,
  TestResultsDuration,
  TestResultsHeader,
  TestResultsProgress,
  TestResultsSummary,
  TestSuite,
  TestSuiteContent,
  TestSuiteName,
  TestSuiteStats,
  Test,
  TestDuration,
  TestName,
  TestStatus,
} from "./test-results"
import {
  Tool,
  ToolContent,
  ToolHeader,
  ToolInput,
  ToolOutput,
} from "./tool"
import { Toolbar } from "./toolbar"
import {
  Transcription,
} from "./transcription"
import {
  WebPreview,
  WebPreviewBody,
  WebPreviewConsole,
  WebPreviewNavigation,
  WebPreviewNavigationButton,
  WebPreviewUrl,
} from "./web-preview"
import { Panel, Row, Stack } from "./layout"
import { Button as UiButton } from "../ui/button"

export default {
  title: "ai/Elements Gallery",
}

export const Core = {
  render: () => html`
    <div class="flex flex-col gap-6">
      ${Agent({
        children: html`
          ${AgentHeader({ name: "Adjutant", model: "gpt-5" })}
          ${AgentContent({
            children: html`
              ${AgentInstructions({ children: "Follow the repo instructions and log decisions." })}
              ${AgentTools({
                children: html`
                  ${AgentTool({
                    tool: { description: "search", inputSchema: { query: "string" } },
                  })}
                `,
              })}
              ${AgentOutput({ schema: "type Guidance = { action: string; reason: string }" })}
            `,
          })}
        `,
      })}

      ${Context({
        usedTokens: 8400,
        maxTokens: 16000,
        usage: { inputTokens: 3200, outputTokens: 4800, reasoningTokens: 400 },
        modelId: "openagents-full-auto",
      })}

      ${ChainOfThought({
        children: html`
          ${ChainOfThoughtHeader({})}
          ${ChainOfThoughtContent({
            children: html`
              ${ChainOfThoughtStep({ label: "Scan repo", description: "Identify entry points" })}
              ${ChainOfThoughtStep({ label: "Update UI", description: "Apply design changes", status: "active" })}
              ${ChainOfThoughtSearchResults({
                children: html`
                  ${ChainOfThoughtSearchResult({ children: "docs/" })}
                  ${ChainOfThoughtSearchResult({ children: "src/" })}
                `,
              })}
            `,
          })}
        `,
      })}

      ${Task({
        children: html`
          ${TaskTrigger({ title: "Open guidance doc" })}
          ${TaskContent({
            children: html`
              ${TaskItem({ children: html`Read instructions ${TaskItemFile({ children: "AGENTS.md" })}` })}
              ${TaskItem({ children: "Summarize decisions" })}
            `,
          })}
        `,
      })}

      ${QueueSection({
        children: html`
          ${QueueSectionHeader({ title: "Queued" })}
          ${QueueSectionContent({
            children: html`
              ${QueueList({
                children: html`
                  ${QueueItem({
                    children: html`
                      <div class="flex items-start gap-2">
                        ${QueueItemIndicator({})}
                        ${QueueItemContent({ children: "Update telemetry panel" })}
                        ${QueueItemActions({ children: QueueItemAction({ children: "…" }) })}
                      </div>
                      ${QueueItemDescription({ children: "Pending" })}
                      ${QueueItemAttachment({
                        children: html`
                          ${QueueItemImage({ src: "https://placehold.co/32x32" })}
                          ${QueueItemFile({ children: "spec.md" })}
                        `,
                      })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        `,
      })}

      ${Confirmation({
        children: html`
          ${ConfirmationTitle({ children: "Approve running tests?" })}
          ${ConfirmationRequest({ children: "This will run the full suite." })}
          ${ConfirmationActions({
            children: html`
              ${ConfirmationAction({ children: "Cancel" })}
              ${ConfirmationAction({ children: "Approve" })}
            `,
          })}
        `,
      })}

      ${Controls({
        children: html`
          ${UiButton({ size: "icon-sm", variant: "ghost", children: "Play" })}
          ${UiButton({ size: "icon-sm", variant: "ghost", children: "Pause" })}
          ${UiButton({ size: "icon-sm", variant: "ghost", children: "Stop" })}
        `,
      })}
    </div>
  `,
}

export const Artifacts = {
  render: () => html`
    <div class="flex flex-col gap-6">
      ${Artifact({
        children: html`
          ${ArtifactHeader({
            children: html`
              <div>
                ${ArtifactTitle({ children: "Report" })}
                ${ArtifactDescription({ children: "Autopilot audit output" })}
              </div>
              ${ArtifactActions({
                children: html`
                  ${ArtifactAction({ tooltip: "Download", children: "↓" })}
                  ${ArtifactAction({ tooltip: "Share", children: "↗" })}
                `,
              })}
            `,
          })}
          ${ArtifactContent({ children: "Artifact contents preview." })}
        `,
      })}

      ${Attachments({
        children: html`
          ${Attachment({
            data: { type: "image", filename: "screenshot.png", mediaType: "image/png", url: "https://placehold.co/96x96" },
            children: html`
              ${AttachmentPreview({ data: { type: "image", filename: "screenshot.png", mediaType: "image/png", url: "https://placehold.co/96x96" } })}
            `,
          })}
          ${Attachment({
            data: { type: "document", filename: "report.pdf", size: 32000 },
            variant: "list",
            children: html`
              ${AttachmentPreview({ data: { type: "document", filename: "report.pdf" }, variant: "list" })}
              ${AttachmentInfo({ data: { type: "document", filename: "report.pdf", size: 32000 }, variant: "list" })}
              ${AttachmentRemove({})}
            `,
          })}
          ${AttachmentEmpty({})}
        `,
      })}

      ${PackageInfo({
        name: "openagents",
        currentVersion: "0.1.0",
        newVersion: "0.1.1",
        changeType: "patch",
        children: html`
          ${PackageInfoDescription({ children: "Patch update with minor fixes." })}
          ${PackageInfoContent({
            children: html`
              ${PackageInfoDependencies({
                children: html`
                  ${PackageInfoDependency({ name: "tauri", version: "2.0.0" })}
                  ${PackageInfoDependency({ name: "effect", version: "3.0.0" })}
                `,
              })}
            `,
          })}
        `,
      })}

      ${Sources({
        children: html`
          ${SourcesTrigger({ count: 2 })}
          ${SourcesContent({
            children: html`
              ${Source({ href: "https://example.com", title: "Example" })}
              ${Source({ href: "https://openai.com", title: "OpenAI" })}
            `,
          })}
        `,
      })}

      ${InlineCitation({
        children: html`
          ${InlineCitationText({ children: "Referenced text" })}
          ${InlineCitationCard({
            children: html`
              ${InlineCitationCardTrigger({ sources: ["https://example.com"] })}
              ${InlineCitationCardBody({
                children: html`
                  ${InlineCitationCarousel({
                    children: html`
                      ${InlineCitationCarouselContent({
                        children: html`
                          ${InlineCitationCarouselItem({
                            children: html`
                              ${InlineCitationSource({
                                title: "Example",
                                url: "https://example.com",
                                description: "Sample citation." })}
                              ${InlineCitationQuote({ children: "Quoted excerpt" })}
                            `,
                          })}
                        `,
                      })}
                      <div class="flex items-center justify-between px-4 pb-3">
                        ${InlineCitationCarouselPrev({})}
                        ${InlineCitationCarouselNext({})}
                      </div>
                    `,
                  })}
                `,
              })}
            `,
          })}
        `,
      })}

      ${OpenIn({
        query: "Guidance module",
        children: html`
          ${OpenInTrigger({})}
          ${OpenInContent({
            children: html`
              ${OpenInLabel({ children: "Open in" })}
              ${OpenInItem({ children: "ChatGPT" })}
              ${OpenInItem({ children: "Claude" })}
              ${OpenInSeparator({})}
              ${OpenInItem({ children: "Cursor" })}
            `,
          })}
        `,
      })}
    </div>
  `,
}

export const Diagnostics = {
  render: () => html`
    <div class="flex flex-col gap-6">
      ${CodeBlock({
        code: "const status = 'ok'\nconsole.log(status)",
        language: "typescript",
        children: html`
          ${CodeBlockHeader({
            children: html`
              ${CodeBlockTitle({ children: "status.ts" })}
              ${CodeBlockActions({ children: CodeBlockCopyButton({}) })}
            `,
          })}
        `,
      })}

      ${Snippet({ code: "pnpm run test" })}

      ${Terminal({
        output: "$ pnpm run test\nPASS src/index.test.ts",
        children: html`
          ${TerminalHeader({
            children: html`
              ${TerminalTitle({})}
              ${TerminalActions({ children: TerminalCopyButton({}) })}
            `,
          })}
          ${TerminalContent({ value: "$ pnpm run test\nPASS src/index.test.ts", children: "PASS src/index.test.ts" })}
        `,
      })}

      ${StackTrace({
        trace: "Error: Something failed\n  at src/index.ts:10:5",
        children: html`
          ${StackTraceHeader({
            children: html`
              ${StackTraceError({ children: StackTraceErrorMessage({ children: "Something failed" }) })}
              ${StackTraceCopyButton({})}
            `,
          })}
        `,
      })}

      ${TestResults({
        children: html`
          ${TestResultsHeader({
            children: html`
              ${TestResultsSummary({})}
              ${TestResultsDuration({ children: "12s" })}
            `,
          })}
          ${TestResultsProgress({ value: 75 })}
          ${TestResultsContent({
            children: html`
              ${TestSuite({
                children: html`
                  ${TestSuiteName({ children: "Unit" })}
                  ${TestSuiteStats({ children: "3 passed" })}
                  ${TestSuiteContent({
                    children: html`
                      ${Test({ children: html`${TestName({ children: "adds" })}${TestStatus({ children: "pass" })}${TestDuration({ children: "5ms" })}` })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        `,
      })}

      ${Tool({
        children: html`
          ${ToolHeader({ type: "run-command", state: "output-available" })}
          ${ToolContent({
            children: html`
              ${ToolInput({ input: { command: "pnpm test" } })}
              ${ToolOutput({ output: { status: "ok" } })}
            `,
          })}
        `,
      })}

      ${Sandbox({
        children: html`
          ${SandboxHeader({ title: "Sandbox" })}
          ${SandboxContent({
            children: html`
              ${SandboxTabs({
                children: html`
                  ${SandboxTabsBar({
                    children: html`
                      ${SandboxTabsList({
                        children: html`
                          ${SandboxTabsTrigger({ children: "Console" })}
                          ${SandboxTabsTrigger({ children: "Files" })}
                        `,
                      })}
                    `,
                  })}
                  ${SandboxTabContent({ children: "Console output" })}
                `,
              })}
            `,
          })}
        `,
      })}

      ${Commit({
        children: html`
          ${CommitHeader({
            children: html`
              ${CommitInfo({
                children: html`
                  ${CommitMessage({ children: "Fix guidance panel" })}
                  ${CommitMetadata({
                    children: html`
                      ${CommitAuthor({ children: CommitAuthorAvatar({ initials: "OA" }) })}
                      ${CommitSeparator({})}
                      ${CommitTimestamp({ date: new Date(Date.now() - 86400000) })}
                    `,
                  })}
                `,
              })}
              ${CommitActions({ children: CommitCopyButton({ hash: "a1b2c3d" }) })}
            `,
          })}
          ${CommitContent({
            children: html`
              ${CommitFiles({
                children: html`
                  ${CommitFile({
                    children: html`
                      ${CommitFileInfo({
                        children: html`
                          ${CommitFileStatus({ status: "modified" })}
                          ${CommitFileIcon({ children: "file" })}
                          ${CommitFilePath({ children: "src/ui.ts" })}
                        `,
                      })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        `,
      })}

      ${SchemaDisplay({
        method: "POST",
        path: "/v1/guidance",
        description: "Create a guidance decision.",
        parameters: [
          { name: "workspace", type: "string", required: true },
          { name: "turn", type: "number" },
        ],
        requestBody: [{ name: "goal", type: "string", required: true }],
        responseBody: [{ name: "action", type: "string" }],
      })}

      ${FileTree({
        children: html`
          ${FileTreeFolder({
            path: "src",
            name: "src",
            expanded: true,
            children: html`
              ${FileTreeFile({ path: "src/main.ts", name: "main.ts", selected: true })}
              ${FileTreeFile({ path: "src/ui.ts", name: "ui.ts" })}
            `,
          })}
          ${FileTreeFile({ path: "README.md", name: "README.md" })}
        `,
      })}
    </div>
  `,
}

export const Media = {
  render: () => html`
    <div class="flex flex-col gap-6">
      ${Image({ src: "https://placehold.co/320x180", alt: "Placeholder" })}

      ${AudioPlayer({
        children: html`
          ${AudioPlayerElement({ src: "https://www.w3schools.com/html/horse.mp3" })}
          ${AudioPlayerControlBar({
            children: html`
              ${AudioPlayerPlayButton({})}
              ${AudioPlayerSeekBackwardButton({})}
              ${AudioPlayerSeekForwardButton({})}
              ${AudioPlayerTimeDisplay({ children: "0:12" })}
              ${AudioPlayerTimeRange({})}
              ${AudioPlayerDurationDisplay({ children: "1:03" })}
              ${AudioPlayerMuteButton({})}
              ${AudioPlayerVolumeRange({})}
            `,
          })}
        `,
      })}

      ${WebPreview({
        children: html`
          ${WebPreviewNavigation({
            children: html`
              ${WebPreviewNavigationButton({ tooltip: "Back", children: "<-" })}
              ${WebPreviewNavigationButton({ tooltip: "Forward", children: "->" })}
              ${WebPreviewUrl({ value: "https://example.com" })}
            `,
          })}
          ${WebPreviewBody({ url: "https://example.com" })}
          ${WebPreviewConsole({ children: "No console output." })}
        `,
      })}

      ${Transcription({
        segments: [
          { text: "Hello", startSecond: 0, endSecond: 1 },
          { text: "world", startSecond: 1, endSecond: 2 },
          { text: "from", startSecond: 2, endSecond: 3 },
          { text: "autopilot", startSecond: 3, endSecond: 4 },
        ],
      })}

      <div class="flex items-center gap-3">
        ${Loader({})}
        ${Shimmer({ children: "Streaming" })}
      </div>

      ${EnvironmentVariables({
        children: html`
          ${EnvironmentVariablesHeader({
            children: html`
              ${EnvironmentVariablesTitle({ children: "Environment" })}
              ${EnvironmentVariablesToggle({ showValues: false })}
            `,
          })}
          ${EnvironmentVariablesContent({
            children: html`
              ${EnvironmentVariable({
                name: "API_KEY",
                value: "abcd-1234-efgh",
                children: html`
                  <div class="flex items-center gap-2">
                    ${EnvironmentVariableName({ name: "API_KEY" })}
                    ${EnvironmentVariableRequired({})}
                  </div>
                  <div class="flex items-center gap-2">
                    ${EnvironmentVariableValue({ value: "abcd-1234-efgh" })}
                    ${EnvironmentVariableCopyButton({})}
                  </div>
                `,
              })}
            `,
          })}
        `,
      })}
    </div>
  `,
}

export const Misc = {
  render: () => html`
    <div class="flex flex-col gap-6">
      ${Alert({ message: "Full auto decision complete.", tone: "success" })}

      ${Checkpoint({
        children: html`
          ${CheckpointIcon({})}
          ${CheckpointTrigger({ children: "Checkpoint" })}
        `,
      })}

      ${Persona({ state: "thinking" })}

      ${ShellPanel({ children: html`<div class="p-4 text-sm">Panel content</div>` })}

      ${Panel({
        title: "Stats",
        subtitle: "Run summary",
        children: html`
          ${Stack({
            gap: 4,
            children: [
              html`Runs: 12`,
              html`Errors: 0`,
              html`Avg: 2m`,
            ],
          })}
          ${Row({
            gap: 8,
            children: [html`CPU 48%`, html`Mem 1.2GB`],
          })}
        `,
      })}

      ${Toolbar({
        children: html`
          ${UiButton({ size: "icon-sm", variant: "ghost", children: "B" })}
          ${UiButton({ size: "icon-sm", variant: "ghost", children: "I" })}
          ${UiButton({ size: "icon-sm", variant: "ghost", children: "U" })}
        `,
      })}
    </div>
  `,
}
