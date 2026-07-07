import { View } from "react-native"

import { khalaMobileTheme } from "../theme/tokens"
import { ActivityIndicator } from "./activity-indicator"
import { KhalaText } from "./khala-text"

export type KhalaEmptyStateTone = "accent" | "danger" | "muted"

export type KhalaEmptyStateProps = Readonly<{
  className?: string
  detail?: string
  loading?: boolean
  testID?: string
  title: string
  tone?: KhalaEmptyStateTone
}>

const titleToneClassName: Record<KhalaEmptyStateTone, string> = {
  accent: "text-text",
  danger: "text-danger",
  muted: "text-text",
}

const detailToneVariant: Record<KhalaEmptyStateTone, "danger" | "muted"> = {
  accent: "muted",
  danger: "danger",
  muted: "muted",
}

export const KhalaEmptyState = ({
  className = "",
  detail,
  loading = false,
  testID,
  title,
  tone = "muted",
}: KhalaEmptyStateProps) => (
  <View
    accessibilityRole={loading ? "progressbar" : "summary"}
    className={`items-center justify-center px-6 py-10 ${className}`.trim()}
    testID={testID}
  >
    {loading ? (
      <ActivityIndicator color={khalaMobileTheme.accent} size={180} strokeWidth={9} type="large" />
    ) : null}
    <KhalaText
      className={`${loading ? "mt-4 " : ""}text-center ${titleToneClassName[tone]}`.trim()}
      variant="body"
    >
      {title}
    </KhalaText>
    {detail === undefined ? null : (
      <KhalaText className="mt-2 text-center" variant={detailToneVariant[tone]}>
        {detail}
      </KhalaText>
    )}
  </View>
)
