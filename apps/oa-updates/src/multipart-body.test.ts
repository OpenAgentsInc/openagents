import { describe, expect, test } from "bun:test"

import { buildMultipartMixed } from "./multipart-body"

type ParsedPart = {
  headers: string[]
  body: string
}

const boundaryFrom = (contentType: string): string => {
  const match = /^multipart\/mixed; boundary=(.+)$/.exec(contentType)

  if (match === null) {
    throw new Error(`Unexpected content type: ${contentType}`)
  }

  return match[1]
}

const parseMultipartMixed = (body: string, boundary: string): ParsedPart[] => {
  const opening = `--${boundary}\r\n`
  const closing = `\r\n--${boundary}--\r\n`
  const empty = `--${boundary}--\r\n`

  if (body === empty) {
    return []
  }

  expect(body.startsWith(opening)).toBe(true)
  expect(body.endsWith(closing)).toBe(true)

  return body
    .slice(opening.length, -closing.length)
    .split(`\r\n--${boundary}\r\n`)
    .map((rawPart) => {
      const headerEnd = rawPart.indexOf("\r\n\r\n")

      expect(headerEnd).toBeGreaterThanOrEqual(0)

      return {
        headers: rawPart.slice(0, headerEnd).split("\r\n"),
        body: rawPart.slice(headerEnd + 4),
      }
    })
}

describe("multipart mixed body", () => {
  test("uses a deterministic multipart/mixed content type boundary", () => {
    const first = buildMultipartMixed([])
    const second = buildMultipartMixed([])

    expect(first.contentType).toBe(
      "multipart/mixed; boundary=openagents-updates-multipart-boundary",
    )
    expect(second.contentType).toBe(first.contentType)
    expect(second.body).toBe(first.body)
  })

  test("renders an empty multipart body with only the closing boundary", () => {
    const response = buildMultipartMixed([])
    const boundary = boundaryFrom(response.contentType)

    expect(response.body).toBe(`--${boundary}--\r\n`)
    expect(parseMultipartMixed(response.body, boundary)).toEqual([])
  })

  test("renders a single part with disposition and content type headers", () => {
    const response = buildMultipartMixed([
      {
        name: "manifest",
        contentType: "application/json",
        body: '{"id":"update-1"}',
      },
    ])
    const boundary = boundaryFrom(response.contentType)
    const parts = parseMultipartMixed(response.body, boundary)

    expect(parts).toEqual([
      {
        headers: [
          'Content-Disposition: form-data; name="manifest"',
          "Content-Type: application/json",
        ],
        body: '{"id":"update-1"}',
      },
    ])
  })

  test("preserves part order and body contents across multiple parts", () => {
    const response = buildMultipartMixed([
      {
        name: "manifest",
        contentType: "application/json",
        body: '{"id":"update-1"}',
      },
      {
        name: "directive",
        contentType: "application/json",
        body: '{"type":"rollBackToEmbedded"}',
      },
      {
        name: "extensions",
        contentType: "application/json",
        body: "{}",
      },
    ])
    const parts = parseMultipartMixed(response.body, boundaryFrom(response.contentType))

    expect(parts.map((part) => part.body)).toEqual([
      '{"id":"update-1"}',
      '{"type":"rollBackToEmbedded"}',
      "{}",
    ])
    expect(parts.map((part) => part.headers[0])).toEqual([
      'Content-Disposition: form-data; name="manifest"',
      'Content-Disposition: form-data; name="directive"',
      'Content-Disposition: form-data; name="extensions"',
    ])
  })

  test("uses CRLF between boundaries, headers, blank lines, and bodies", () => {
    const response = buildMultipartMixed([
      {
        name: "metadata",
        contentType: "text/plain",
        body: "plain body",
      },
    ])
    const boundary = boundaryFrom(response.contentType)

    expect(response.body).toBe(
      [
        `--${boundary}`,
        'Content-Disposition: form-data; name="metadata"',
        "Content-Type: text/plain",
        "",
        "plain body",
        `--${boundary}--`,
        "",
      ].join("\r\n"),
    )
  })

  test("round trips bodies containing line breaks without normalizing them", () => {
    const body = "line one\nline two\r\nline three"
    const response = buildMultipartMixed([
      {
        name: "log",
        contentType: "text/plain",
        body,
      },
    ])
    const [part] = parseMultipartMixed(response.body, boundaryFrom(response.contentType))

    expect(part.body).toBe(body)
  })

  test("escapes quoted disposition names while preserving round-trippable parts", () => {
    const response = buildMultipartMixed([
      {
        name: 'asset"metadata\\ios',
        contentType: "application/json",
        body: "{}",
      },
    ])
    const [part] = parseMultipartMixed(response.body, boundaryFrom(response.contentType))

    expect(part.headers[0]).toBe(
      'Content-Disposition: form-data; name="asset\\"metadata\\\\ios"',
    )
    expect(part.body).toBe("{}")
  })
})
