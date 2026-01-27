import { html } from "../../effuse/template/html"
import { Button } from "./button"
import { Heading, Text } from "./text"
import { Input } from "./input"
import { TextArea } from "./textarea"
import { Toggle } from "./toggle"
import { Select } from "./select"
import {
  PromptInput,
  PromptInputActionAddAttachments,
  PromptInputActionMenu,
  PromptInputActionMenuContent,
  PromptInputActionMenuItem,
  PromptInputActionMenuTrigger,
  PromptInputBody,
  PromptInputButton,
  PromptInputCommand,
  PromptInputCommandEmpty,
  PromptInputCommandGroup,
  PromptInputCommandInput,
  PromptInputCommandItem,
  PromptInputCommandList,
  PromptInputCommandSeparator,
  PromptInputFooter,
  PromptInputHeader,
  PromptInputHoverCard,
  PromptInputHoverCardContent,
  PromptInputHoverCardTrigger,
  PromptInputSelect,
  PromptInputSelectContent,
  PromptInputSelectItem,
  PromptInputSelectTrigger,
  PromptInputSelectValue,
  PromptInputSubmit,
  PromptInputTab,
  PromptInputTabBody,
  PromptInputTabItem,
  PromptInputTabLabel,
  PromptInputTabsList,
  PromptInputTextarea,
  PromptInputTools,
} from "./prompt-input"
import {
  MicSelector,
  MicSelectorContent,
  MicSelectorInput,
  MicSelectorItem,
  MicSelectorList,
  MicSelectorLabel,
  MicSelectorTrigger,
  MicSelectorValue,
} from "./mic-selector"
import {
  ModelSelector,
  ModelSelectorContent,
  ModelSelectorEmpty,
  ModelSelectorGroup,
  ModelSelectorInput,
  ModelSelectorItem,
  ModelSelectorList,
  ModelSelectorLogo,
  ModelSelectorSeparator,
  ModelSelectorShortcut,
  ModelSelectorTrigger,
} from "./model-selector"
import {
  VoiceSelector,
  VoiceSelectorContent,
  VoiceSelectorAccent,
  VoiceSelectorAge,
  VoiceSelectorAttributes,
  VoiceSelectorBullet,
  VoiceSelectorDescription,
  VoiceSelectorGender,
  VoiceSelectorGroup,
  VoiceSelectorInput,
  VoiceSelectorItem,
  VoiceSelectorList,
  VoiceSelectorName,
  VoiceSelectorPreview,
  VoiceSelectorSeparator,
  VoiceSelectorShortcut,
  VoiceSelectorTrigger,
} from "./voice-selector"
import { SpeechInput } from "./speech-input"
import { Suggestions, Suggestion } from "./suggestion"

export default {
  title: "ai/Elements Inputs",
}

export const BasicInputs = {
  render: () => html`
    <div class="flex flex-col gap-4">
      ${Heading({ text: "Inputs" })}
      ${Text({ text: "Core text and toggle fields", tone: "muted" })}
      ${Input({ name: "query", label: "Search", placeholder: "Search" })}
      ${TextArea({ name: "notes", label: "Notes", placeholder: "Describe the task" })}
      ${Toggle({ name: "auto", label: "Auto mode", checked: true })}
      ${Select({
        name: "mode",
        label: "Mode",
        value: "full",
        options: [
          { value: "full", label: "Full" },
          { value: "assist", label: "Assist" },
          { value: "review", label: "Review" },
        ],
      })}
      <div class="flex flex-wrap gap-2">
        ${Button({ label: "Primary" })}
        ${Button({ label: "Secondary", variant: "secondary" })}
        ${Button({ label: "Danger", variant: "danger" })}
      </div>
    </div>
  `,
}

export const PromptComposer = {
  render: () => html`
    <div class="flex flex-col gap-4">
      ${Heading({ text: "Prompt Input" })}
      ${PromptInput({
        children: html`
          ${PromptInputHeader({
            children: html`
              ${PromptInputSelect({
                children: html`
                  ${PromptInputSelectTrigger({
                    children: PromptInputSelectValue({ children: "Default" }),
                  })}
                  ${PromptInputSelectContent({
                    children: html`
                      ${PromptInputSelectItem({ value: "default", children: "Default" })}
                      ${PromptInputSelectItem({ value: "coding", children: "Coding" })}
                      ${PromptInputSelectItem({ value: "research", children: "Research" })}
                    `,
                  })}
                `,
              })}
              ${PromptInputActionAddAttachments({ children: "+" })}
            `,
          })}
          ${PromptInputBody({ children: PromptInputTextarea({ placeholder: "Ask something" }) })}
          ${PromptInputFooter({
            children: html`
              ${PromptInputTools({
                children: html`
                  ${PromptInputButton({ children: "Attach" })}
                  ${PromptInputButton({ children: "Stop" })}
                  ${PromptInputHoverCard({
                    children: html`
                      ${PromptInputHoverCardTrigger({ children: "?" })}
                      ${PromptInputHoverCardContent({ children: "Quick tips" })}
                    `,
                  })}
                `,
              })}
              <div class="flex items-center gap-2">
                ${PromptInputActionMenu({
                  children: html`
                    ${PromptInputActionMenuTrigger({ children: "+" })}
                    ${PromptInputActionMenuContent({
                      children: html`
                        ${PromptInputActionMenuItem({ children: "Add file" })}
                        ${PromptInputActionMenuItem({ children: "Add image" })}
                      `,
                    })}
                  `,
                })}
                ${PromptInputSubmit({ status: "idle" })}
              </div>
            `,
          })}
        `,
      })}
    </div>
  `,
}

export const Selectors = {
  render: () => html`
    <div class="flex flex-col gap-6">
      ${Heading({ text: "Selectors" })}
      ${MicSelector({
        children: html`
          ${MicSelectorTrigger({
            children: html`${MicSelectorLabel({})} ${MicSelectorValue({ children: "Built-in Mic" })}`,
          })}
          ${MicSelectorContent({
            children: html`
              ${MicSelectorInput({})}
              ${MicSelectorList({
                children: html`
                  ${MicSelectorItem({ children: "Built-in Mic" })}
                  ${MicSelectorItem({ children: "External USB" })}
                `,
              })}
            `,
          })}
        `,
      })}

      ${ModelSelector({
        children: html`
          ${ModelSelectorTrigger({ children: "Select model" })}
          ${ModelSelectorContent({
            children: html`
              ${ModelSelectorInput({})}
              ${ModelSelectorList({
                children: html`
                  ${ModelSelectorGroup({
                    children: html`
                      ${ModelSelectorItem({
                        children: html`${ModelSelectorLogo({ provider: "openai" })} OpenAI GPT-4 ${ModelSelectorShortcut({
                          children: "⌘1",
                        })}`,
                      })}
                      ${ModelSelectorItem({
                        children: html`Anthropic Claude ${ModelSelectorShortcut({ children: "⌘2" })}`,
                      })}
                      ${ModelSelectorSeparator({})}
                      ${ModelSelectorEmpty({ children: "No more models" })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        `,
      })}

      ${VoiceSelector({
        children: html`
          ${VoiceSelectorTrigger({ children: "Select voice" })}
          ${VoiceSelectorContent({
            children: html`
              ${VoiceSelectorInput({})}
              ${VoiceSelectorList({
                children: html`
                  ${VoiceSelectorGroup({
                    children: html`
                      ${VoiceSelectorItem({ children: html`
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
                      ` })}
                      ${VoiceSelectorSeparator({})}
                      ${VoiceSelectorItem({
                        children: html`Marcus ${VoiceSelectorShortcut({ children: "⌘2" })}`,
                      })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        `,
      })}
    </div>
  `,
}

export const SpeechAndSuggestions = {
  render: () => html`
    <div class="flex flex-col gap-4">
      ${Heading({ text: "Speech + Suggestions" })}
      <div class="flex gap-2">
        ${SpeechInput({ label: "Talk", isListening: false })}
        ${SpeechInput({ label: "Listening", isListening: true })}
      </div>
      ${Suggestions({
        children: html`
          ${Suggestion({ suggestion: "Summarize this" })}
          ${Suggestion({ suggestion: "List files" })}
          ${Suggestion({ suggestion: "Run tests" })}
        `,
      })}
    </div>
  `,
}

export const PromptTabsAndCommand = {
  render: () => html`
    <div class=\"flex flex-col gap-4\">\n      ${Heading({ text: \"Prompt Tabs + Command\" })}\n      ${PromptInputTab({\n        children: html`\n          ${PromptInputTabsList({\n            children: html`\n              ${PromptInputTabItem({ children: PromptInputTabLabel({ children: \"Draft\" }) })}\n              ${PromptInputTabItem({ children: PromptInputTabLabel({ children: \"History\" }) })}\n            `,\n          })}\n          ${PromptInputTabBody({\n            children: html`\n              ${PromptInputCommand({\n                children: html`\n                  ${PromptInputCommandInput({})}\n                  ${PromptInputCommandList({\n                    children: html`\n                      ${PromptInputCommandEmpty({ children: \"No results\" })}\n                      ${PromptInputCommandGroup({\n                        children: html`\n                          ${PromptInputCommandItem({ children: \"Fix tests\" })}\n                          ${PromptInputCommandSeparator({})}\n                          ${PromptInputCommandItem({ children: \"Update docs\" })}\n                        `,\n                      })}\n                    `,\n                  })}\n                `,\n              })}\n            `,\n          })}\n        `,\n      })}\n    </div>\n  `,
}
