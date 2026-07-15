import { useMemo, useState, type ReactElement } from "react"

import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "#components/ui/tooltip"

const REDACTED_TEXT_ALPHABET = "abcdefghjkmnpqrstuvwxyz23456789"

/** Stable fake text keeps width/punctuation while never exposing the value. */
export const redactedSensitivePlaceholder = (value: string): string => {
  let state = 0x811c9dc5
  for (let index = 0; index < value.length; index += 1) {
    state ^= value.charCodeAt(index)
    state = Math.imul(state, 0x01000193)
  }

  const nextChar = (): string => {
    state = Math.imul(state ^ (state >>> 13), 0x85ebca6b)
    state = Math.imul(state ^ (state >>> 16), 0xc2b2ae35)
    return REDACTED_TEXT_ALPHABET[Math.abs(state) % REDACTED_TEXT_ALPHABET.length] ?? "x"
  }

  return Array.from(value, character =>
    character === "@" || character === "." || character === "-" || character === "_"
      ? character
      : nextChar(),
  ).join("")
}

export const RedactedSensitiveText = ({
  value,
  ariaLabel,
  revealTooltip,
  hideTooltip,
  className,
}: Readonly<{
  value: string | null | undefined
  ariaLabel: string
  revealTooltip: string
  hideTooltip: string
  className?: string
}>): ReactElement | null => {
  const [revealed, setRevealed] = useState(false)
  const trimmed = value?.trim()
  const redacted = useMemo(
    () => trimmed === undefined || trimmed.length === 0 ? "" : redactedSensitivePlaceholder(trimmed),
    [trimmed],
  )
  if (trimmed === undefined || trimmed.length === 0) return null

  return <TooltipProvider>
    <Tooltip>
      <TooltipTrigger
        render={<button
          type="button"
          className={["oa-react-sensitive-text", className].filter(Boolean).join(" ")}
          data-revealed={revealed ? "true" : "false"}
          onClick={() => setRevealed(current => !current)}
          aria-label={ariaLabel}
        />}
      >
        {revealed ? trimmed : redacted}
      </TooltipTrigger>
      <TooltipContent>{revealed ? hideTooltip : revealTooltip}</TooltipContent>
    </Tooltip>
  </TooltipProvider>
}
