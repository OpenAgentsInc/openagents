export type MultipartMixedPart = {
  name: string
  contentType: string
  body: string
}

export type MultipartMixedBody = {
  contentType: string
  body: string
}

const boundary = "openagents-updates-multipart-boundary"

const quoteHeaderValue = (value: string): string =>
  value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')

export const buildMultipartMixed = (
  parts: MultipartMixedPart[],
): MultipartMixedBody => {
  const bodyLines = parts.flatMap((part) => [
    `--${boundary}`,
    `Content-Disposition: form-data; name="${quoteHeaderValue(part.name)}"`,
    `Content-Type: ${part.contentType}`,
    "",
    part.body,
  ])

  return {
    contentType: `multipart/mixed; boundary=${boundary}`,
    body: [...bodyLines, `--${boundary}--`, ""].join("\r\n"),
  }
}
