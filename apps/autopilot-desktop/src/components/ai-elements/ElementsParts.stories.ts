import { html } from "../../effuse/template/html"
import {
  Artifact,
  ArtifactAction,
  ArtifactActions,
  ArtifactClose,
  ArtifactContent,
  ArtifactDescription,
  ArtifactHeader,
  ArtifactTitle,
} from "./artifact"
import {
  Attachment,
  AttachmentHoverCard,
  AttachmentHoverCardContent,
  AttachmentHoverCardTrigger,
  AttachmentInfo,
  AttachmentPreview,
  getAttachmentLabel,
  getMediaCategory,
} from "./attachments"
import {
  CodeBlockActions,
  CodeBlockContainer,
  CodeBlockContent,
  CodeBlockCopyButton,
  CodeBlockFilename,
  CodeBlockHeader,
  CodeBlockLanguageSelector,
  CodeBlockLanguageSelectorContent,
  CodeBlockLanguageSelectorItem,
  CodeBlockLanguageSelectorTrigger,
  CodeBlockLanguageSelectorValue,
  CodeBlockTitle,
} from "./code-block"
import {
  Commit,
  CommitActions,
  CommitContent,
  CommitCopyButton,
  CommitFile,
  CommitFileAdditions,
  CommitFileChanges,
  CommitFileDeletions,
  CommitFileInfo,
  CommitFilePath,
  CommitFileStatus,
  CommitFiles,
  CommitHash,
  CommitHeader,
  CommitInfo,
  CommitMessage,
  CommitMetadata,
  CommitSeparator,
  CommitTimestamp,
} from "./commit"
import {
  Confirmation,
  ConfirmationAccepted,
  ConfirmationAction,
  ConfirmationActions,
  ConfirmationRejected,
  ConfirmationRequest,
  ConfirmationTitle,
} from "./confirmation"
import {
  Context,
  ContextCacheUsage,
  ContextContent,
  ContextContentBody,
  ContextContentFooter,
  ContextContentHeader,
  ContextInputUsage,
  ContextOutputUsage,
  ContextReasoningUsage,
  ContextTrigger,
} from "./context"
import {
  Conversation,
  ConversationContent,
  ConversationScrollButton,
} from "./conversation"
import {
  EnvironmentVariable,
  EnvironmentVariableCopyButton,
  EnvironmentVariableGroup,
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
  FileTreeActions,
  FileTreeFile,
  FileTreeFolder,
  FileTreeIcon,
  FileTreeName,
} from "./file-tree"
import {
  InlineCitation,
  InlineCitationCard,
  InlineCitationCardBody,
  InlineCitationCardTrigger,
  InlineCitationCarousel,
  InlineCitationCarouselContent,
  InlineCitationCarouselHeader,
  InlineCitationCarouselIndex,
  InlineCitationCarouselItem,
  InlineCitationCarouselNext,
  InlineCitationCarouselPrev,
  InlineCitationQuote,
  InlineCitationSource,
  InlineCitationText,
} from "./inline-citation"
import {
  Message,
  MessageAction,
  MessageActions,
  MessageBranch,
  MessageBranchContent,
  MessageBranchNext,
  MessageBranchPage,
  MessageBranchPrevious,
  MessageBranchSelector,
  MessageContent,
  MessageResponse,
  MessageToolbar,
} from "./message"
import {
  MicSelector,
  MicSelectorContent,
  MicSelectorEmpty,
  MicSelectorInput,
  MicSelectorItem,
  MicSelectorLabel,
  MicSelectorList,
  MicSelectorTrigger,
  MicSelectorValue,
  useAudioDevices,
} from "./mic-selector"
import {
  ModelSelectorDialog,
  ModelSelectorEmpty,
  ModelSelectorGroup,
  ModelSelectorInput,
  ModelSelectorItem,
  ModelSelectorList,
  ModelSelectorLogo,
  ModelSelectorSeparator,
  ModelSelectorShortcut,
} from "./model-selector"
import {
  OpenIn,
  OpenInChatGPT,
  OpenInClaude,
  OpenInContent,
  OpenInCursor,
  OpenInLabel,
  OpenInScira,
  OpenInSeparator,
  OpenInT3,
  OpenInTrigger,
  OpenInv0,
} from "./open-in-chat"
import {
  PackageInfo,
  PackageInfoChangeType,
  PackageInfoHeader,
  PackageInfoName,
  PackageInfoVersion,
} from "./package-info"
import {
  Plan,
  PlanAction,
  PlanContent,
  PlanDescription,
  PlanFooter,
  PlanHeader,
  PlanTitle,
  PlanTrigger,
} from "./plan"
import {
  LocalReferencedSourcesContext,
  usePromptInputAttachments,
  usePromptInputController,
  usePromptInputReferencedSources,
  useProviderAttachments,
} from "./prompt-input"
import { Reasoning, ReasoningContent, ReasoningTrigger } from "./reasoning"
import {
  SchemaDisplayBody,
  SchemaDisplayContent,
  SchemaDisplayDescription,
  SchemaDisplayExample,
  SchemaDisplayHeader,
  SchemaDisplayMethod,
  SchemaDisplayParameter,
  SchemaDisplayParameters,
  SchemaDisplayPath,
  SchemaDisplayProperty,
  SchemaDisplayRequest,
  SchemaDisplayResponse,
} from "./schema-display"
import {
  Snippet,
  SnippetAddon,
  SnippetCopyButton,
  SnippetInput,
  SnippetText,
} from "./snippet"
import {
  StackTrace,
  StackTraceActions,
  StackTraceContent,
  StackTraceCopyButton,
  StackTraceError,
  StackTraceErrorMessage,
  StackTraceErrorType,
  StackTraceExpandButton,
  StackTraceFrames,
  StackTraceHeader,
} from "./stack-trace"
import {
  Terminal,
  TerminalActions,
  TerminalClearButton,
  TerminalContent,
  TerminalCopyButton,
  TerminalHeader,
  TerminalStatus,
  TerminalTitle,
} from "./terminal"
import {
  Test,
  TestDuration,
  TestError,
  TestErrorMessage,
  TestErrorStack,
  TestName,
  TestResults,
  TestResultsContent,
  TestResultsDuration,
  TestResultsHeader,
  TestResultsProgress,
  TestResultsSummary,
  TestStatus,
  TestSuite,
  TestSuiteContent,
  TestSuiteName,
  TestSuiteStats,
} from "./test-results"
import { getStatusBadge } from "./tool"
import { Transcription, TranscriptionSegment } from "./transcription"
import {
  VoiceSelectorDialog,
  VoiceSelectorEmpty,
  VoiceSelectorGroup,
  VoiceSelectorInput,
  VoiceSelectorItem,
  VoiceSelectorList,
  VoiceSelectorName,
  VoiceSelectorDescription,
  VoiceSelectorAttributes,
  VoiceSelectorGender,
  VoiceSelectorAccent,
  VoiceSelectorAge,
  VoiceSelectorBullet,
  VoiceSelectorPreview,
  VoiceSelectorSeparator,
  VoiceSelectorShortcut,
  useVoiceSelector,
} from "./voice-selector"

export default {
  title: "ai/Elements Parts",
}

const sampleAttachment = {
  type: "source-document",
  filename: "guidance.md",
  title: "Guidance spec",
  mediaType: "text/markdown",
  size: 2048,
}

const sampleCode = "export const nextAction = (state) => ({ action: 'continue' })"

export const Parts = {
  render: () => {
    const attachmentLabel = getAttachmentLabel(sampleAttachment)
    const attachmentCategory = getMediaCategory(sampleAttachment)
    const audioDevices = useAudioDevices()
    const promptController = usePromptInputController()
    const providerAttachments = useProviderAttachments()
    const promptAttachments = usePromptInputAttachments()
    const referencedSources = usePromptInputReferencedSources()
    const voiceSelector = useVoiceSelector()

    return html`
      <div class="flex flex-col gap-10">
        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Artifacts + Attachments</div>
          ${Artifact({
            children: html`
              ${ArtifactHeader({
                children: html`
                  <div>
                    ${ArtifactTitle({ children: "Guidance Draft" })}
                    ${ArtifactDescription({ children: "Turn summary output" })}
                  </div>
                  ${ArtifactActions({
                    children: html`
                      ${ArtifactAction({ tooltip: "Pin", children: "★" })}
                      ${ArtifactClose({})}
                    `,
                  })}
                `,
              })}
              ${ArtifactContent({ children: html`<div class="text-xs">Artifact content preview.</div>` })}
            `,
          })}

          <div class="flex flex-col gap-2">
            ${AttachmentHoverCard({
              children: html`
                ${AttachmentHoverCardTrigger({ children: html`<span class="text-xs underline">Hover for attachment</span>` })}
                ${AttachmentHoverCardContent({
                  children: html`
                    <div class="text-xs">Attachment: ${attachmentLabel} (${attachmentCategory})</div>
                  `,
                })}
              `,
            })}
            ${Attachment({
              data: sampleAttachment,
              variant: "list",
              children: html`
                ${AttachmentPreview({ data: sampleAttachment, variant: "list" })}
                ${AttachmentInfo({ data: sampleAttachment, variant: "list" })}
              `,
            })}
            <div class="text-xs text-muted-foreground">Label: ${attachmentLabel} · Category: ${attachmentCategory}</div>
          </div>
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Code Block Parts</div>
          ${CodeBlockContainer({
            language: "typescript",
            copyValue: sampleCode,
            children: html`
              ${CodeBlockHeader({
                children: html`
                  ${CodeBlockTitle({
                    children: html`${CodeBlockFilename({ children: "guidance.ts" })}`,
                  })}
                  ${CodeBlockActions({
                    children: html`
                      ${CodeBlockLanguageSelector({
                        children: html`
                          ${CodeBlockLanguageSelectorTrigger({
                            children: CodeBlockLanguageSelectorValue({ children: "TypeScript" }),
                          })}
                          ${CodeBlockLanguageSelectorContent({
                            children: html`
                              ${CodeBlockLanguageSelectorItem({ value: "ts", children: "TypeScript" })}
                              ${CodeBlockLanguageSelectorItem({ value: "js", children: "JavaScript" })}
                            `,
                          })}
                        `,
                      })}
                      ${CodeBlockCopyButton({})}
                    `,
                  })}
                `,
              })}
              ${CodeBlockContent({ code: sampleCode, language: "typescript", showLineNumbers: true })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Commit Details</div>
          ${Commit({
            children: html`
              ${CommitHeader({
                children: html`
                  ${CommitInfo({
                    children: html`
                      ${CommitMessage({ children: "Update guidance docs" })}
                      ${CommitMetadata({
                        children: html`
                          ${CommitHash({ children: "a1b2c3d" })}
                          ${CommitSeparator({})}
                          ${CommitTimestamp({ date: new Date() })}
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
                              ${CommitFilePath({ children: "docs/guidance.md" })}
                              ${CommitFileChanges({
                                children: html`
                                  ${CommitFileAdditions({ count: 12 })}
                                  ${CommitFileDeletions({ count: 3 })}
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
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Confirmation States</div>
          ${Confirmation({
            children: html`
              ${ConfirmationTitle({ children: "Approve running tests?" })}
              ${ConfirmationRequest({ children: "This will run the full suite." })}
              ${ConfirmationAccepted({ children: html`<div class="text-xs text-status-connected">Approved by user.</div>` })}
              ${ConfirmationRejected({ children: html`<div class="text-xs text-destructive">Rejected: missing context.</div>` })}
              ${ConfirmationActions({
                children: html`
                  ${ConfirmationAction({ children: "Cancel" })}
                  ${ConfirmationAction({ children: "Approve" })}
                `,
              })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Context Parts</div>
          <div class="flex flex-wrap items-start gap-6">
            <div class="space-y-2">
              ${ContextTrigger({ usedTokens: 3200, maxTokens: 8000 })}
              ${Context({
                usedTokens: 3200,
                maxTokens: 8000,
                usage: { inputTokens: 1200, outputTokens: 1600, reasoningTokens: 300, cachedTokens: 100 },
                modelId: "oa-guidance",
              })}
              ${ContextContent({
                usedTokens: 3200,
                maxTokens: 8000,
                usage: { inputTokens: 1200, outputTokens: 1600, reasoningTokens: 300, cachedTokens: 100 },
                modelId: "oa-guidance",
              })}
            </div>
            <div class="rounded-md border bg-background p-3">
              ${ContextContentHeader({ modelId: "oa-guidance" })}
              ${ContextContentBody({
                usedTokens: 3200,
                maxTokens: 8000,
                percent: 40,
                usage: { inputTokens: 1200, outputTokens: 1600, reasoningTokens: 300, cachedTokens: 100 },
              })}
              ${ContextContentFooter({ children: "Usage recalculated each turn." })}
              <div class="mt-2 space-y-1">
                ${ContextInputUsage({ value: 1200 })}
                ${ContextOutputUsage({ value: 1600 })}
                ${ContextReasoningUsage({ value: 300 })}
                ${ContextCacheUsage({ value: 100 })}
              </div>
            </div>
          </div>
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Conversation Scroll</div>
          ${Conversation({
            children: html`
              ${ConversationContent({
                children: html`
                  <div class="text-xs text-muted-foreground">Conversation content placeholder.</div>
                `,
              })}
              ${ConversationScrollButton({})}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Environment Variable Groups</div>
          ${EnvironmentVariables({
            children: html`
              ${EnvironmentVariablesHeader({
                children: html`
                  ${EnvironmentVariablesTitle({ children: "Environment" })}
                  ${EnvironmentVariablesToggle({ showValues: true })}
                `,
              })}
              ${EnvironmentVariablesContent({
                children: html`
                  ${EnvironmentVariable({
                    name: "API_KEY",
                    value: "abcd-1234-efgh",
                    children: html`
                      ${EnvironmentVariableGroup({
                        children: html`
                          ${EnvironmentVariableName({ name: "API_KEY" })}
                          ${EnvironmentVariableRequired({})}
                        `,
                      })}
                      ${EnvironmentVariableGroup({
                        children: html`
                          ${EnvironmentVariableValue({ value: "abcd-1234-efgh", showValues: true })}
                          ${EnvironmentVariableCopyButton({})}
                        `,
                      })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">File Tree Actions</div>
          ${FileTree({
            children: html`
              ${FileTreeFolder({
                path: "src",
                name: "src",
                expanded: true,
                children: html`
                  ${FileTreeFile({
                    path: "src/index.ts",
                    name: "index.ts",
                    children: html`<span class="text-xs">edit</span>`,
                  })}
                `,
              })}
              ${FileTreeActions({ children: html`<span class="text-xs">⋯</span>` })}
              <div class="flex items-center gap-2 px-2 py-1">
                ${FileTreeIcon({ children: "file" })}
                ${FileTreeName({ children: "README.md" })}
                ${FileTreeActions({ children: html`<span class="text-xs">open</span>` })}
              </div>
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Inline Citation Header + Index</div>
          ${InlineCitation({
            children: html`
              ${InlineCitationText({ children: "Referenced section" })}
              ${InlineCitationCard({
                children: html`
                  ${InlineCitationCardTrigger({ sources: ["https://openai.com", "https://example.com"] })}
                  ${InlineCitationCardBody({
                    children: html`
                      ${InlineCitationCarousel({
                        children: html`
                          ${InlineCitationCarouselHeader({
                            children: html`
                              <div class="text-xs font-medium">Sources</div>
                              ${InlineCitationCarouselIndex({ children: "1/2" })}
                            `,
                          })}
                          ${InlineCitationCarouselContent({
                            children: html`
                              ${InlineCitationCarouselItem({
                                children: html`
                                  ${InlineCitationSource({
                                    title: "OpenAI",
                                    url: "https://openai.com",
                                    description: "Primary reference link.",
                                  })}
                                  ${InlineCitationQuote({ children: "Short quoted excerpt." })}
                                `,
                              })}
                            `,
                          })}
                          <div class="flex items-center justify-between px-3 pb-3">
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
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Message Composition</div>
          ${Message({
            from: "assistant",
            children: html`
              ${MessageContent({
                children: html`
                  ${MessageResponse({
                    children: html`<p class="text-sm">Here is the latest run summary and next action.</p>`,
                  })}
                `,
              })}
              ${MessageToolbar({
                children: html`
                  ${MessageActions({
                    children: html`
                      ${MessageAction({ tooltip: "Copy", children: "copy" })}
                      ${MessageAction({ tooltip: "Pin", children: "pin" })}
                    `,
                  })}
                  ${MessageBranchSelector({
                    from: "assistant",
                    children: html`
                      ${MessageBranchPrevious({})}
                      ${MessageBranchPage({ children: "1 / 2" })}
                      ${MessageBranchNext({})}
                    `,
                  })}
                `,
              })}
              ${MessageBranch({
                children: html`
                  ${MessageBranchContent({
                    active: true,
                    children: html`
                      ${MessageContent({
                        children: MessageResponse({ children: "Alternate branch response." }),
                      })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Selector Empty States</div>
          ${MicSelector({
            children: html`
              ${MicSelectorTrigger({
                children: html`${MicSelectorLabel({})} ${MicSelectorValue({ children: "None" })}`,
              })}
              ${MicSelectorContent({
                children: html`
                  ${MicSelectorInput({})}
                  ${MicSelectorList({
                    children: html`
                      ${MicSelectorItem({ children: "Built-in Mic" })}
                      ${MicSelectorEmpty({ children: "No devices" })}
                    `,
                  })}
                `,
              })}
            `,
          })}

          ${ModelSelectorDialog({
            children: html`
              ${ModelSelectorInput({})}
              ${ModelSelectorList({
                children: html`
                  ${ModelSelectorGroup({
                    children: html`
                      ${ModelSelectorItem({
                        children: html`${ModelSelectorLogo({ provider: "openai" })} GPT-4 ${ModelSelectorShortcut({ children: "⌘1" })}`,
                      })}
                      ${ModelSelectorSeparator({})}
                      ${ModelSelectorEmpty({ children: "No more models" })}
                    `,
                  })}
                `,
              })}
            `,
          })}

          ${VoiceSelectorDialog({
            children: html`
              ${VoiceSelectorInput({})}
              ${VoiceSelectorList({
                children: html`
                  ${VoiceSelectorGroup({
                    children: html`
                      ${VoiceSelectorItem({
                        children: html`
                          ${VoiceSelectorName({ children: "Olivia" })}
                          ${VoiceSelectorDescription({ children: "Warm and calm" })}
                          ${VoiceSelectorAttributes({
                            children: html`
                              ${VoiceSelectorGender({ children: "Female" })}
                              ${VoiceSelectorBullet({ children: "•" })}
                              ${VoiceSelectorAccent({ children: "US" })}
                              ${VoiceSelectorAge({ children: "Adult" })}
                            `,
                          })}
                          ${VoiceSelectorPreview({ children: "Preview sample" })}
                        `,
                      })}
                      ${VoiceSelectorSeparator({})}
                      ${VoiceSelectorEmpty({ children: "No voices" })}
                      ${VoiceSelectorItem({ children: html`Marcus ${VoiceSelectorShortcut({ children: "⌘2" })}` })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Open In Shortcuts</div>
          ${OpenIn({
            query: "Guidance module",
            children: html`
              ${OpenInTrigger({})}
              ${OpenInContent({
                children: html`
                  ${OpenInLabel({ children: "Open in" })}
                  ${OpenInChatGPT({ query: "Guidance module" })}
                  ${OpenInClaude({ query: "Guidance module" })}
                  ${OpenInT3({ query: "Guidance module" })}
                  ${OpenInScira({ query: "Guidance module" })}
                  ${OpenInv0({ query: "Guidance module" })}
                  ${OpenInCursor({ query: "Guidance module" })}
                  ${OpenInSeparator({})}
                `,
              })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Package Info Header</div>
          ${PackageInfo({
            name: "openagents",
            currentVersion: "0.1.0",
            newVersion: "0.2.0",
            changeType: "minor",
            children: html`
              ${PackageInfoHeader({
                children: html`
                  ${PackageInfoName({ name: "openagents" })}
                  ${PackageInfoChangeType({ changeType: "minor" })}
                `,
              })}
              ${PackageInfoVersion({ currentVersion: "0.1.0", newVersion: "0.2.0" })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Plan Parts</div>
          ${Plan({
            children: html`
              ${PlanHeader({
                children: html`
                  <div>
                    ${PlanTitle({ children: "Guidance Plan" })}
                    ${PlanDescription({ children: "Define the next turn actions." })}
                  </div>
                  ${PlanAction({ children: PlanTrigger({}) })}
                `,
              })}
              ${PlanContent({
                children: html`
                  <ol class="space-y-1 text-sm">
                    <li>1. Summarize last turn</li>
                    <li>2. Apply guardrails</li>
                    <li>3. Compose next action</li>
                  </ol>
                `,
              })}
              ${PlanFooter({ children: html`<div class="text-xs text-muted-foreground">Updated just now.</div>` })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Reasoning Parts</div>
          ${Reasoning({
            children: html`
              ${ReasoningTrigger({ isStreaming: true, duration: 4, isOpen: true })}
              ${ReasoningContent({ children: "Evaluated context, budget, and pending approvals." })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Schema Display Parts</div>
          <div class="rounded-lg border bg-background">
            ${SchemaDisplayHeader({
              children: html`
                <div class="flex items-center gap-3">
                  ${SchemaDisplayMethod({ method: "POST" })}
                  ${SchemaDisplayPath({ path: "/v1/guidance" })}
                </div>
              `,
            })}
            ${SchemaDisplayDescription({ description: "Create a guidance decision." })}
            ${SchemaDisplayContent({
              children: html`
                ${SchemaDisplayParameters({
                  parameters: [
                    { name: "workspace", type: "string", required: true },
                    { name: "turn", type: "number" },
                  ],
                })}
                ${SchemaDisplayRequest({
                  properties: [{ name: "goal", type: "string", required: true }],
                })}
                ${SchemaDisplayResponse({
                  properties: [{ name: "action", type: "string", required: true }],
                })}
              `,
            })}
          </div>
          <div class="space-y-2 rounded-md border bg-background p-3">
            ${SchemaDisplayParameter({ parameter: { name: "limit", type: "number", required: true } })}
            ${SchemaDisplayProperty({ property: { name: "action", type: "string", required: true } })}
            ${SchemaDisplayBody({
              title: "Example",
              properties: [{ name: "reason", type: "string" }],
            })}
            ${SchemaDisplayExample({ children: "{ \"action\": \"continue\" }" })}
          </div>
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Snippet Parts</div>
          ${Snippet({
            code: "pnpm run test",
            children: html`
              ${SnippetAddon({ children: "$" })}
              ${SnippetText({ children: "pnpm run test" })}
              ${SnippetInput({ code: "pnpm run test" })}
              ${SnippetCopyButton({})}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Stack Trace Parts</div>
          ${StackTrace({
            trace: "TypeError: undefined is not a function\n  at src/app.ts:12:4",
            children: html`
              ${StackTraceHeader({
                children: html`
                  ${StackTraceError({
                    children: html`
                      ${StackTraceErrorType({ children: "TypeError" })}
                      ${StackTraceErrorMessage({ children: "undefined is not a function" })}
                    `,
                  })}
                  ${StackTraceActions({
                    children: html`
                      ${StackTraceCopyButton({})}
                      ${StackTraceExpandButton({})}
                    `,
                  })}
                `,
              })}
              ${StackTraceContent({
                children: html`
                  ${StackTraceFrames({
                    children: html`
                      <pre class="whitespace-pre-wrap p-3 text-xs text-muted-foreground">at src/app.ts:12:4</pre>
                    `,
                  })}
                `,
              })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Terminal Parts</div>
          ${Terminal({
            output: "$ pnpm run test\nPASS src/app.test.ts",
            children: html`
              ${TerminalHeader({
                children: html`
                  ${TerminalTitle({ children: "Tests" })}
                  ${TerminalStatus({ children: "Running" })}
                  ${TerminalActions({
                    children: html`
                      ${TerminalCopyButton({})}
                      ${TerminalClearButton({})}
                    `,
                  })}
                `,
              })}
              ${TerminalContent({ value: "$ pnpm run test\nPASS src/app.test.ts", children: "PASS src/app.test.ts" })}
            `,
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Test Error Parts</div>
          ${TestResults({
            children: html`
              ${TestResultsHeader({
                children: html`
                  ${TestResultsSummary({ children: "Test Results" })}
                  ${TestResultsDuration({ children: "12s" })}
                `,
              })}
              ${TestResultsProgress({ value: 75 })}
              ${TestResultsContent({
                children: html`
                  ${TestSuite({
                    children: html`
                      ${TestSuiteName({ children: "Unit" })}
                      ${TestSuiteStats({ children: "3 passed, 1 failed" })}
                      ${TestSuiteContent({
                        children: html`
                          ${Test({
                            children: html`
                              ${TestName({ children: "should compile" })}
                              ${TestStatus({ children: "fail" })}
                              ${TestDuration({ children: "8ms" })}
                            `,
                          })}
                          ${TestError({
                            children: html`
                              ${TestErrorMessage({ children: "Assertion failed" })}
                              ${TestErrorStack({ children: "Expected true, received false." })}
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
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Transcription Segments</div>
          ${Transcription({
            segments: [
              { text: "Guidance", startSecond: 0, endSecond: 1 },
              { text: "module", startSecond: 1, endSecond: 2 },
            ],
            children: (segment, index) => TranscriptionSegment({ segment, index }),
          })}
        </section>

        <section class="space-y-3">
          <div class="text-xs uppercase text-muted-foreground">Hooks + Helpers</div>
          <div class="rounded-md border bg-background p-3 text-xs">
            <div class="flex items-center gap-2">${getStatusBadge("output-available")}</div>
            <pre class="mt-2 whitespace-pre-wrap text-[11px] text-muted-foreground">useAudioDevices: ${JSON.stringify(audioDevices)}</pre>
            <pre class="mt-2 whitespace-pre-wrap text-[11px] text-muted-foreground">usePromptInputController: ${JSON.stringify(promptController)}</pre>
            <pre class="mt-2 whitespace-pre-wrap text-[11px] text-muted-foreground">useProviderAttachments: ${JSON.stringify(providerAttachments)}</pre>
            <pre class="mt-2 whitespace-pre-wrap text-[11px] text-muted-foreground">usePromptInputAttachments: ${JSON.stringify(promptAttachments)}</pre>
            <pre class="mt-2 whitespace-pre-wrap text-[11px] text-muted-foreground">usePromptInputReferencedSources: ${JSON.stringify(referencedSources)}</pre>
            <pre class="mt-2 whitespace-pre-wrap text-[11px] text-muted-foreground">LocalReferencedSourcesContext: ${JSON.stringify(LocalReferencedSourcesContext)}</pre>
            <pre class="mt-2 whitespace-pre-wrap text-[11px] text-muted-foreground">useVoiceSelector: ${JSON.stringify(voiceSelector)}</pre>
          </div>
        </section>
      </div>
    `
  },
}
