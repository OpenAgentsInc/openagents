import type { Action, UITree } from "../../effuse/ui/index.js"

const setAction = (path: string, valuePath: string): Action => ({
  name: "ui.set",
  params: {
    path,
    value: { path: valuePath },
  },
})

export const createSetupTree = (): UITree => ({
  root: "canvas",
  elements: {
    canvas: {
      key: "canvas",
      type: "canvas",
      props: {
        title: "Autopilot",
        subtitle: "Configure your run",
        status: { path: "/status/phase" },
      },
      children: ["stack"],
    },
    stack: {
      key: "stack",
      type: "stack",
      props: { gap: 16 },
      children: ["panel-setup"],
    },
    "panel-setup": {
      key: "panel-setup",
      type: "panel",
      props: {
        title: "Workspace",
        subtitle: "Pick a repo and describe the task",
      },
      children: ["input-workspace", "textarea-prompt", "button-start"],
    },
    "input-workspace": {
      key: "input-workspace",
      type: "input",
      props: {
        name: "workspacePath",
        label: "Working directory",
        placeholder: "/path/to/repo",
        value: { path: "/workspace/path" },
        action: setAction("/workspace/path", "/__event/params/workspacePath"),
        trigger: "change",
      },
    },
    "textarea-prompt": {
      key: "textarea-prompt",
      type: "textarea",
      props: {
        name: "taskPrompt",
        label: "Task prompt",
        placeholder: "Describe what you want Autopilot to do...",
        value: { path: "/task/prompt" },
        action: setAction("/task/prompt", "/__event/params/taskPrompt"),
        trigger: "change",
        rows: 4,
      },
    },
    "button-start": {
      key: "button-start",
      type: "button",
      props: {
        label: "Start Autopilot",
        variant: "primary",
        action: {
          name: "ui.start",
          params: {
            workspacePath: { path: "/workspace/path" },
            prompt: { path: "/task/prompt" },
          },
        },
      },
    },
  },
})
