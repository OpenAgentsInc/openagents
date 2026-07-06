import { Pressable, type PressableProps } from "react-native"

import { khalaMobileTheme } from "../../theme/tokens"
import { ActivityIndicator } from "../activity-indicator"
import { KhalaText } from "../khala-text"

export type NexusSignInButtonProps = Omit<PressableProps, "children" | "disabled"> &
  Readonly<{
    disabled?: boolean
    loading?: boolean
    text: string
  }>

/** The "Sign in with GitHub" CTA restyled after wireframe direction 1d's
 * ("Nexus Beam") button — a bordered, near-transparent rectangular bar (not a
 * filled pill): cyan `accent` (`#4fd0ff`) border, `surfaceRaised` fill, a
 * small diamond glyph before the label. See `id="1d"` lines 181-186 of
 * `~/Downloads/Khala Mobile landing wireframe/Khala Mobile Landing
 * Wireframes.dc.html`.
 *
 * Bespoke rather than a `KhalaButton` variant: `KhalaButton`'s shared
 * variants (`primary`/`secondary`/`ghost`/`danger`) are filled/bordered pills
 * used across the rest of the app, and this screen's one-off bar treatment
 * (plus the leading glyph swapping for a themed spinner while `loading`)
 * doesn't generalize cleanly as another shared variant without touching
 * every other `KhalaButton` call site. Keeps the same `disabled`/`loading`/
 * `onPress` contract the shared button exposes. */
export const NexusSignInButton = ({
  disabled = false,
  loading = false,
  text,
  ...pressableProps
}: NexusSignInButtonProps) => {
  const unavailable = disabled || loading

  return (
    <Pressable
      {...pressableProps}
      accessibilityRole="button"
      accessibilityState={{ busy: loading, disabled: unavailable }}
      className={`h-14 w-full flex-row items-center justify-center gap-3 rounded-md border-[1.5px] border-accent bg-surfaceRaised/90 ${
        unavailable ? "opacity-50" : ""
      }`}
      disabled={unavailable}
    >
      {loading ? (
        <ActivityIndicator color={khalaMobileTheme.accent} size={20} />
      ) : (
        <KhalaText style={{ color: khalaMobileTheme.accent, fontSize: 15 }} variant="mono">
          {"◈"}
        </KhalaText>
      )}
      <KhalaText
        className="text-center font-semibold tracking-wide"
        style={{ color: khalaMobileTheme.accentText, fontSize: 19 }}
        variant="body"
      >
        {text}
      </KhalaText>
    </Pressable>
  )
}
