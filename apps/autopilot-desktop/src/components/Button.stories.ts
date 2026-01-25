import { html } from "../effuse/template/html"
import { Button } from "./Button"
import { Effect } from "effect"

// Action helper to mock actions in Effuse environment
const action = (name: string) => Effect.sync(() => console.log(`[Action] ${name}`))

export default {
  title: "core/Button",
  component: Button,
  parameters: {
    // Optional: configuring controls/actions if we had them fully implemented
  }
}

export const All = {
  render: () => html`
    <div class="flex flex-col gap-3">
      <div class="text-xs text-muted-foreground">Primary</div>
      ${Button({
        label: "Click Me",
        variant: "primary",
        disabled: false,
        onClick: action("Clicked Primary"),
      })}

      <div class="text-xs text-muted-foreground">Secondary</div>
      ${Button({
        label: "Secondary Action",
        variant: "secondary",
        onClick: action("Clicked Secondary"),
      })}

      <div class="text-xs text-muted-foreground">Disabled</div>
      ${Button({
        label: "Can't Touch This",
        disabled: true,
        variant: "primary",
      })}

      <div class="text-xs text-muted-foreground">Delete Button (Large)</div>
      ${Button({
        label: "Delete Everything",
        variant: "danger",
        size: "lg",
        onClick: action("Clicked Delete"),
      })}
    </div>
  `,
}
