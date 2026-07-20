#!/usr/bin/env node

"use strict"

const MAX_HEADER_BYTES = 8 * 1024
const MAX_BODY_BYTES = 4 * 1024 * 1024
const HEADER_SEPARATOR = Buffer.from("\r\n\r\n", "ascii")
const flags = new Set(process.argv.slice(2))
const FIXTURE_SOURCE = {
  name: flags.has("--alternate-source") ? "alternate.ts" : "fixture.ts",
  path: flags.has("--alternate-source")
    ? "/workspace/src/alternate.ts"
    : "/workspace/src/fixture.ts",
  sourceReference: 1,
}

let buffered = Buffer.alloc(0)
let nextSequence = 1
let nextBreakpointId = 1
let closing = false
let reverseRequestSequence = null
let delayedStartRequest = null

const state = {
  initialized: false,
  configured: false,
  mode: null,
  counter: 7,
  label: "fixture",
  breakpoints: [],
}

if (flags.has("--ignore-sigterm")) {
  process.on("SIGTERM", () => {
    process.stderr.write("fixture ignored SIGTERM\n")
  })
}

const encode = (message) => {
  const body = Buffer.from(JSON.stringify(message), "utf8")
  return Buffer.concat([
    Buffer.from(`Content-Length: ${body.byteLength}\r\n\r\n`, "ascii"),
    body,
  ])
}

const withSequence = (message) => ({ seq: nextSequence++, ...message })

const response = (request, body = undefined) => withSequence({
  type: "response",
  request_seq: request.seq,
  success: true,
  command: request.command,
  ...(body === undefined ? {} : { body }),
})

const failedResponse = (request, message) => withSequence({
  type: "response",
  request_seq: request.seq,
  success: false,
  command: request.command,
  message,
})

const event = (eventName, body = undefined) => withSequence({
  type: "event",
  event: eventName,
  ...(body === undefined ? {} : { body }),
})

const sendPayload = (payload, exitAfterWrite) => {
  if (!flags.has("--fragment-writes") || payload.byteLength < 3) {
    process.stdout.write(payload, () => {
      if (exitAfterWrite) process.exit(0)
    })
    return
  }
  const first = Math.max(1, Math.floor(payload.byteLength / 3))
  const second = Math.max(first + 1, Math.floor(payload.byteLength * 2 / 3))
  process.stdout.write(payload.subarray(0, first), () => {
    process.stdout.write(payload.subarray(first, second), () => {
      process.stdout.write(payload.subarray(second), () => {
        if (exitAfterWrite) process.exit(0)
      })
    })
  })
}

const sendBatch = (messages, exitAfterWrite = false) => {
  sendPayload(Buffer.concat(messages.map(encode)), exitAfterWrite)
}

const transportFailure = (detail) => {
  if (closing) return
  closing = true
  process.stderr.write(`ide-dap-fixture: ${detail}\n`, () => process.exit(1))
}

const argumentsOf = (request) => {
  const value = request.arguments
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value : {}
}

const initializedGuard = (request) => {
  if (state.initialized) return null
  return [failedResponse(request, "The initialize request must complete first.")]
}

const runningGuard = (request) => {
  const initialized = initializedGuard(request)
  if (initialized !== null) return initialized
  if (state.mode !== null) return null
  return [failedResponse(request, "A launch or attach request must complete first.")]
}

const stoppedEvent = (reason, description) => event("stopped", {
  reason,
  description,
  threadId: 1,
  allThreadsStopped: true,
  ...(reason === "breakpoint" && state.breakpoints.length > 0
    ? { hitBreakpointIds: [state.breakpoints[0].id] }
    : {}),
})

const continuedEvent = () => event("continued", {
  threadId: 1,
  allThreadsContinued: true,
})

const handleInitialize = (request) => {
  state.initialized = true
  if (flags.has("--stderr-burst")) {
    process.stderr.write(`${"x".repeat(32 * 1024)} token=fixture-secret-token\n`)
  }
  const messages = [
    response(request, {
      supportsConfigurationDoneRequest: true,
      supportsConditionalBreakpoints: true,
      supportsHitConditionalBreakpoints: true,
      supportsLogPoints: true,
      supportsFunctionBreakpoints: true,
      supportsDataBreakpoints: true,
      supportsSetVariable: true,
      supportsEvaluateForHovers: true,
      supportsRestartRequest: true,
      supportsRestartFrame: true,
      supportsTerminateRequest: true,
      supportsStepBack: true,
      supportsLoadedSourcesRequest: true,
      supportsModulesRequest: true,
    }),
    event("initialized"),
  ]
  if (flags.has("--reverse-request")) {
    const reverse = withSequence({
      type: "request",
      command: "runInTerminal",
      arguments: { kind: "integrated", title: "not admitted", args: ["secret"] },
    })
    reverseRequestSequence = reverse.seq
    messages.push(reverse)
  }
  return messages
}

const handleStart = (request, mode) => {
  const rejected = initializedGuard(request)
  if (rejected !== null) return rejected
  state.mode = mode
  state.configured = false
  state.counter = 7
  if (flags.has("--delay-start-response")) {
    delayedStartRequest = request
    return [event("initialized")]
  }
  return [
    response(request),
    event("output", {
      category: "console",
      output: `Fixture ${mode} target ready.\n`,
      source: FIXTURE_SOURCE,
      line: 1,
      column: 1,
    }),
  ]
}

const handleSetBreakpoints = (request) => {
  const rejected = runningGuard(request)
  if (rejected !== null) return rejected
  const requested = Array.isArray(argumentsOf(request).breakpoints)
    ? argumentsOf(request).breakpoints
    : []
  if (flags.has("--reject-empty-breakpoint-clear") && requested.length === 0) {
    return [failedResponse(request, "The fixture rejected a stale source clear.")]
  }
  state.breakpoints = requested.map((candidate) => {
    const line = candidate !== null
      && typeof candidate === "object"
      && Number.isInteger(candidate.line)
      ? candidate.line
      : 1
    return {
      id: nextBreakpointId++,
      verified: true,
      message: "Fixture breakpoint admitted.",
      source: FIXTURE_SOURCE,
      line,
      column: 1,
    }
  })
  return [response(request, { breakpoints: state.breakpoints })]
}

const handleVariables = (request) => {
  const variablesReference = argumentsOf(request).variablesReference
  if (variablesReference === 201 && flags.has("--nested-variables")) {
    return [response(request, {
      variables: [
        { name: "nested-value", value: "49", type: "number", variablesReference: 0 },
      ],
    })]
  }
  if (variablesReference !== 200) {
    return [response(request, { variables: [] })]
  }
  return [response(request, {
    variables: [
      { name: "counter", value: String(state.counter), type: "number", variablesReference: 0 },
      { name: "label", value: JSON.stringify(state.label), type: "string", variablesReference: 0 },
      { name: "mode", value: JSON.stringify(state.mode), type: "string", variablesReference: 0 },
      ...(flags.has("--nested-variables")
        ? [{ name: "nested", value: "Object", type: "object", variablesReference: 201 }]
        : []),
    ],
  })]
}

const handleEvaluate = (request) => {
  const expression = String(argumentsOf(request).expression ?? "")
  const result = expression === "counter"
    ? String(state.counter)
    : expression === "label"
      ? JSON.stringify(state.label)
      : expression === "mode"
        ? JSON.stringify(state.mode)
        : `evaluated:${expression}`
  return [response(request, { result, type: "string", variablesReference: 0 })]
}

const handleSetVariable = (request) => {
  const requestArguments = argumentsOf(request)
  const name = String(requestArguments.name ?? "")
  const value = String(requestArguments.value ?? "")
  if (requestArguments.variablesReference !== 200) {
    return [failedResponse(request, "The variables reference is not active.")]
  }
  if (name === "counter") {
    const parsed = Number(value)
    if (!Number.isFinite(parsed)) {
      return [failedResponse(request, "counter requires a finite number.")]
    }
    state.counter = parsed
    return [response(request, {
      value: String(state.counter),
      type: "number",
      variablesReference: 0,
    })]
  }
  if (name === "label") {
    state.label = value
    return [response(request, {
      value: JSON.stringify(state.label),
      type: "string",
      variablesReference: 0,
    })]
  }
  return [failedResponse(request, `The fixture variable ${name} is not mutable.`)]
}

const handleControl = (request, reason, description) => [
  response(request),
  continuedEvent(),
  stoppedEvent(reason, description),
]

const handleRequest = (request) => {
  if (closing) return
  let messages
  let exitAfterWrite = false

  switch (request.command) {
    case "initialize":
      messages = handleInitialize(request)
      break
    case "launch":
      messages = handleStart(request, "launch")
      break
    case "attach":
      messages = handleStart(request, "attach")
      break
    case "configurationDone": {
      const rejected = runningGuard(request)
      if (rejected !== null) {
        messages = rejected
      } else {
        state.configured = true
        messages = [
          response(request),
          ...(delayedStartRequest === null ? [] : [response(delayedStartRequest)]),
          stoppedEvent("entry", "Fixture target stopped at entry."),
        ]
        delayedStartRequest = null
      }
      break
    }
    case "setBreakpoints":
      messages = handleSetBreakpoints(request)
      break
    case "setFunctionBreakpoints":
    case "setDataBreakpoints": {
      const requested = Array.isArray(argumentsOf(request).breakpoints)
        ? argumentsOf(request).breakpoints
        : []
      messages = [response(request, {
        breakpoints: requested.map(() => ({ id: nextBreakpointId++, verified: true })),
      })]
      break
    }
    case "threads":
      if (flags.has("--ignore-threads")) return
      messages = [response(request, { threads: [{ id: 1, name: "Fixture Main Thread" }] })]
      break
    case "stackTrace":
      messages = [response(request, {
        stackFrames: [{
          id: 100,
          name: "fixtureMain",
          source: FIXTURE_SOURCE,
          line: 7,
          column: 1,
          endLine: 7,
          endColumn: 24,
        }],
        totalFrames: 1,
      })]
      break
    case "scopes":
      messages = [response(request, {
        scopes: [{
          name: "Locals",
          presentationHint: "locals",
          variablesReference: 200,
          expensive: false,
        }],
      })]
      break
    case "variables":
      messages = handleVariables(request)
      break
    case "evaluate":
      messages = handleEvaluate(request)
      break
    case "setVariable":
      messages = handleSetVariable(request)
      break
    case "continue":
      messages = [
        response(request, { allThreadsContinued: true }),
        continuedEvent(),
        stoppedEvent("breakpoint", "Fixture target stopped at a breakpoint."),
      ]
      break
    case "pause":
      messages = [response(request), stoppedEvent("pause", "Fixture target paused.")]
      break
    case "next":
      messages = handleControl(request, "step", "Fixture target completed step over.")
      break
    case "stepIn":
      messages = handleControl(request, "step", "Fixture target completed step in.")
      break
    case "stepOut":
      messages = handleControl(request, "step", "Fixture target completed step out.")
      break
    case "stepBack":
      messages = handleControl(request, "step", "Fixture target completed step back.")
      break
    case "restartFrame":
      messages = [response(request), stoppedEvent("step", "Fixture frame restarted.")]
      break
    case "restart":
      state.counter = 7
      messages = [
        response(request),
        event("output", { category: "console", output: "Fixture target restarted.\n" }),
        stoppedEvent("entry", "Fixture target stopped after restart."),
      ]
      break
    case "source":
      messages = [response(request, {
        content: "export const fixtureMain = () => 7\n",
        mimeType: "text/typescript",
      })]
      break
    case "loadedSources":
      messages = [response(request, { sources: [FIXTURE_SOURCE] })]
      break
    case "modules":
      messages = [response(request, {
        modules: [{ id: "fixture-module", name: "fixture", path: FIXTURE_SOURCE.path }],
        totalModules: 1,
      })]
      break
    case "disconnect":
    case "terminate":
      closing = true
      messages = [
        flags.has("--reject-disconnect") && request.command === "disconnect"
          ? failedResponse(request, "The fixture adapter refused detach.")
          : response(request),
        event("terminated", { restart: false }),
      ]
      exitAfterWrite = true
      break
    default:
      messages = [failedResponse(request, `The fixture does not implement ${request.command}.`)]
  }

  sendBatch(messages, exitAfterWrite)
  if (request.command === "initialize" && flags.has("--malformed-after-initialize")) {
    process.stdout.write(Buffer.from("Content-Length: nope\r\n\r\n{}", "ascii"))
  }
}

const handleResponse = (message) => {
  if (reverseRequestSequence === null || message.request_seq !== reverseRequestSequence) return
  const outcome = message.success === false ? "rejected" : "accepted"
  sendBatch([event("output", {
    category: "telemetry",
    output: `reverse-request-${outcome}\n`,
  })])
  reverseRequestSequence = null
}

const parseHeader = (header) => {
  for (const byte of header) {
    if (byte > 0x7f) throw new Error("DAP headers must use ASCII bytes.")
  }
  const lines = header.toString("ascii").split("\r\n")
  let contentLength = null
  for (const line of lines) {
    const separator = line.indexOf(":")
    if (separator <= 0) throw new Error("A DAP header line is malformed.")
    const name = line.slice(0, separator).trim().toLowerCase()
    const value = line.slice(separator + 1).trim()
    if (name !== "content-length") continue
    if (contentLength !== null || !/^(?:0|[1-9][0-9]*)$/u.test(value)) {
      throw new Error("DAP Content-Length is repeated or invalid.")
    }
    contentLength = Number(value)
  }
  if (
    !Number.isSafeInteger(contentLength)
    || contentLength < 2
    || contentLength > MAX_BODY_BYTES
  ) {
    throw new Error("DAP Content-Length is outside the fixture limit.")
  }
  return contentLength
}

const acceptChunk = (chunk) => {
  buffered = Buffer.concat([buffered, chunk])
  while (!closing) {
    const headerEnd = buffered.indexOf(HEADER_SEPARATOR)
    if (headerEnd < 0) {
      if (buffered.byteLength > MAX_HEADER_BYTES) {
        throw new Error("DAP header exceeded the fixture limit.")
      }
      return
    }
    if (headerEnd === 0 || headerEnd > MAX_HEADER_BYTES) {
      throw new Error("DAP header size is invalid.")
    }
    const contentLength = parseHeader(buffered.subarray(0, headerEnd))
    const bodyStart = headerEnd + HEADER_SEPARATOR.byteLength
    const frameEnd = bodyStart + contentLength
    if (buffered.byteLength < frameEnd) return
    const body = buffered.subarray(bodyStart, frameEnd)
    buffered = buffered.subarray(frameEnd)

    let message
    try {
      message = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(body))
    } catch {
      throw new Error("DAP body is not valid UTF-8 JSON.")
    }
    if (message === null || typeof message !== "object") {
      throw new Error("DAP body is not a valid envelope.")
    }
    if (
      message.type === "request"
      && Number.isInteger(message.seq)
      && message.seq >= 1
      && typeof message.command === "string"
      && message.command.length > 0
    ) {
      handleRequest(message)
      continue
    }
    if (
      message.type === "response"
      && Number.isInteger(message.request_seq)
      && typeof message.command === "string"
      && typeof message.success === "boolean"
    ) {
      handleResponse(message)
      continue
    }
    throw new Error("DAP body is not a valid request or response envelope.")
  }
}

process.stdin.on("data", (chunk) => {
  try {
    acceptChunk(chunk)
  } catch (error) {
    transportFailure(error instanceof Error ? error.message : String(error))
  }
})

process.stdin.on("end", () => {
  if (closing) return
  if (buffered.byteLength > 0) {
    transportFailure("DAP input ended with an incomplete frame.")
    return
  }
  process.exit(0)
})

process.stdin.on("error", (error) => transportFailure(error.message))
process.stdin.resume()
