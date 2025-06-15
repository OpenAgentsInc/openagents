import type { Fx } from "@typed/fx/Fx"
import { RenderEvent } from "@typed/template/RenderEvent"
import { Button } from "./button.js"

export type CopyButtonProps = {
  content: string
  copyMessage?: string
  className?: string
}

export const CopyButton = (props: CopyButtonProps): Fx<RenderEvent, never, any> => {
  const handleCopy = () => {
    navigator.clipboard.writeText(props.content)
  }

  return Button({
    variant: "ghost",
    size: "icon", 
    className: ["relative h-6 w-6", props.className].filter(Boolean).join(" "),
    onClick: handleCopy,
    children: "📋"
  })
}