export type InferenceChargeContext = Readonly<{
  adapterId: string
  requestedModel?: string
  servedModel: string
  totalTokens: number
}>

export const inferenceChargeContextRef = (
  input: Readonly<{
    adapterId: string
    requestedModel?: string
    servedModel: string
    totalTokens: number
  }>,
): string => {
  const base = `inference:${encodeURIComponent(input.adapterId)}:served:${encodeURIComponent(input.servedModel)}:tokens:${Math.max(0, Math.trunc(input.totalTokens))}`
  const requestedModel = input.requestedModel?.trim() ?? ''
  return requestedModel.length === 0
    ? base
    : `${base}:requested:${encodeURIComponent(requestedModel)}`
}

export const parseInferenceChargeContextRef = (
  value: string,
): InferenceChargeContext | undefined => {
  const match =
    /^inference:([^:]+):served:([^:]+):tokens:(\d+)(?::requested:([^:]+))?$/.exec(
      value,
    )
  if (match === null) {
    return undefined
  }

  const totalTokens = Number(match[3])
  if (!Number.isSafeInteger(totalTokens)) {
    return undefined
  }

  const requestedModel = match[4]
  return {
    adapterId: decodeURIComponent(match[1] ?? ''),
    ...(requestedModel === undefined
      ? {}
      : { requestedModel: decodeURIComponent(requestedModel) }),
    servedModel: decodeURIComponent(match[2] ?? ''),
    totalTokens,
  }
}
