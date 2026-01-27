import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { ButtonGroup, ButtonGroupText } from "../ui/button-group.js"
import { cx, type AIChildren } from "./utils.js"

export type AudioPlayerProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const AudioPlayer = ({ className, children }: AudioPlayerProps): TemplateResult => html`
  <div
    data-slot="audio-player"
    class="${cx("flex w-full items-center gap-2", className)}"
  >
    ${children ?? ""}
  </div>
`

export type AudioPlayerElementProps = {
  readonly src?: string
  readonly data?: { base64: string; mediaType: string }
}

export const AudioPlayerElement = ({ src, data }: AudioPlayerElementProps): TemplateResult => {
  const source = src ?? (data ? `data:${data.mediaType};base64,${data.base64}` : "")
  return html`<audio data-slot="audio-player-element" src="${source}"></audio>`
}

export type AudioPlayerControlBarProps = {
  readonly children?: AIChildren
}

export const AudioPlayerControlBar = ({ children }: AudioPlayerControlBarProps): TemplateResult =>
  html`<div data-slot="audio-player-control-bar">${ButtonGroup({ orientation: "horizontal", children })}</div>`

export type AudioPlayerPlayButtonProps = {
  readonly className?: string
}

export const AudioPlayerPlayButton = ({ className }: AudioPlayerPlayButtonProps): TemplateResult =>
  Button({
    className: cx("bg-transparent", className),
    size: "icon-sm",
    type: "button",
    variant: "outline",
    children: "play",
  })

export type AudioPlayerSeekBackwardButtonProps = {
  readonly seekOffset?: number
}

export const AudioPlayerSeekBackwardButton = ({ seekOffset = 10 }: AudioPlayerSeekBackwardButtonProps): TemplateResult =>
  Button({
    size: "icon-sm",
    type: "button",
    variant: "outline",
    children: `-${seekOffset}s`,
  })

export type AudioPlayerSeekForwardButtonProps = {
  readonly seekOffset?: number
}

export const AudioPlayerSeekForwardButton = ({ seekOffset = 10 }: AudioPlayerSeekForwardButtonProps): TemplateResult =>
  Button({
    size: "icon-sm",
    type: "button",
    variant: "outline",
    children: `+${seekOffset}s`,
  })

export type AudioPlayerTimeDisplayProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const AudioPlayerTimeDisplay = ({ className, children }: AudioPlayerTimeDisplayProps): TemplateResult =>
  ButtonGroupText({ className: cx("bg-transparent tabular-nums", className), children: children ?? "0:00" })

export type AudioPlayerTimeRangeProps = {
  readonly className?: string
}

export const AudioPlayerTimeRange = ({ className }: AudioPlayerTimeRangeProps): TemplateResult =>
  ButtonGroupText({ className: cx("bg-transparent", className), children: "|||||" })

export type AudioPlayerDurationDisplayProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const AudioPlayerDurationDisplay = ({ className, children }: AudioPlayerDurationDisplayProps): TemplateResult =>
  ButtonGroupText({ className: cx("bg-transparent tabular-nums", className), children: children ?? "0:00" })

export type AudioPlayerMuteButtonProps = {
  readonly className?: string
}

export const AudioPlayerMuteButton = ({ className }: AudioPlayerMuteButtonProps): TemplateResult =>
  Button({
    className,
    size: "icon-sm",
    type: "button",
    variant: "outline",
    children: "mute",
  })

export type AudioPlayerVolumeRangeProps = {
  readonly className?: string
}

export const AudioPlayerVolumeRange = ({ className }: AudioPlayerVolumeRangeProps): TemplateResult =>
  ButtonGroupText({ className: cx("bg-transparent", className), children: "||" })
