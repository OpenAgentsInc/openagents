const assistantMessageClass = "message-bubble message-bubble--assistant"
const shimmerBaseTag = "ai-elements:shimmer/Shimmer"
const shimmerClass = "oa-ai-shimmer"

const renderTranscriptStatusIndicator = (input: {
  readonly ariaLabel: string
  readonly datasetKey: "khalaThinking" | "khalaThreadLoading"
  readonly messageId: string
  readonly text: string
}): HTMLElement => {
  const article = document.createElement("article")
  article.className = `${assistantMessageClass} message-bubble--thinking`
  article.dataset.messageId = input.messageId
  article.dataset[input.datasetKey] = "true"

  const body = document.createElement("div")
  body.className = "message-body"

  const shimmer = document.createElement("span")
  shimmer.className = shimmerClass
  shimmer.dataset.uiBase = shimmerBaseTag
  shimmer.dataset.oaAiShimmer = ""
  shimmer.setAttribute("role", "status")
  shimmer.setAttribute("aria-live", "polite")
  shimmer.setAttribute("aria-label", input.ariaLabel)
  shimmer.textContent = input.text

  body.append(shimmer)
  article.append(body)
  return article
}

export const renderThreadLoadingIndicator = (
  selectionId: number | null,
): HTMLElement | null => {
  if (selectionId === null) return null
  return renderTranscriptStatusIndicator({
    ariaLabel: "Loading messages",
    datasetKey: "khalaThreadLoading",
    messageId: `thread-loading-${selectionId}`,
    text: "Loading messages",
  })
}

export const renderThinkingIndicator = (
  thinkingTurnId: string | null,
): HTMLElement | null => {
  if (thinkingTurnId === null) return null
  return renderTranscriptStatusIndicator({
    ariaLabel: "Thinking",
    datasetKey: "khalaThinking",
    messageId: `thinking-${thinkingTurnId}`,
    text: "Thinking",
  })
}
