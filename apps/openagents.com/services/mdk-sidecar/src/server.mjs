import { GET, POST } from '@moneydevkit/core/route'

const port = Number(process.env.PORT ?? '8080')

const json = (status, body) =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json',
    },
  })

const readBody = request =>
  request
    .arrayBuffer()
    .then(buffer => (buffer.byteLength === 0 ? null : buffer))

const envConfigured = value =>
  typeof value === 'string' && value.trim().length > 0

const toMdkRequest = async request => {
  const url = new URL(request.url)
  url.pathname = '/api/mdk'
  const body = request.method === 'GET' ? undefined : await readBody(request)

  return new Request(url, {
    body,
    headers: request.headers,
    method: request.method,
  })
}

const handleRequest = async request => {
  const url = new URL(request.url)

  if (request.method === 'GET' && url.pathname === '/healthz') {
    return json(200, {
      ok: true,
      service: 'openagents-mdk-sidecar',
      mdkAccessTokenConfigured: envConfigured(process.env.MDK_ACCESS_TOKEN),
      mdkMnemonicConfigured: envConfigured(process.env.MDK_MNEMONIC),
      withdrawalDestinationConfigured: envConfigured(
        process.env.WITHDRAWAL_DESTINATION,
      ),
    })
  }

  if (url.pathname !== '/api/mdk') {
    return json(404, { error: 'not_found' })
  }

  if (request.method === 'POST') {
    return POST(await toMdkRequest(request))
  }

  if (request.method === 'GET') {
    return GET(await toMdkRequest(request))
  }

  return json(405, { error: 'method_not_allowed' })
}

const Runtime = defineRuntime()
const server = await Runtime.serve({
  fetch: handleRequest,
  port,
})

console.log(`openagents-mdk-sidecar listening on ${server.url}`)

function defineRuntime() {
  if (typeof globalThis.Bun !== 'undefined') {
    return globalThis.Bun
  }

  return {
    serve: ({ fetch, port: listenPort }) => nodeServe(fetch, listenPort),
  }
}

async function nodeServe(fetch, listenPort) {
  const http = await import('node:http')
  const server = http.createServer(async (incoming, outgoing) => {
    const protocol = incoming.headers['x-forwarded-proto'] ?? 'http'
    const host = incoming.headers.host ?? `localhost:${listenPort}`
    const url = `${protocol}://${host}${incoming.url ?? '/'}`
    const chunks = []

    for await (const chunk of incoming) {
      chunks.push(chunk)
    }

    const body =
      chunks.length === 0 || incoming.method === 'GET'
        ? undefined
        : Buffer.concat(chunks)
    const headers = new Headers()

    Object.entries(incoming.headers).forEach(([key, value]) => {
      if (Array.isArray(value)) {
        headers.set(key, value.join(', '))
      } else if (typeof value === 'string') {
        headers.set(key, value)
      }
    })

    const request = new Request(url, {
      body,
      headers,
      method: incoming.method,
    })

    try {
      const response = await fetch(request)
      outgoing.writeHead(response.status, Object.fromEntries(response.headers))
      if (response.body) {
        const reader = response.body.getReader()
        while (true) {
          const { done, value } = await reader.read()
          if (done) {
            break
          }
          outgoing.write(value)
        }
      }
      outgoing.end()
    } catch (error) {
      outgoing.writeHead(500, { 'content-type': 'application/json' })
      outgoing.end(
        JSON.stringify({
          error: 'sidecar_error',
          message: error instanceof Error ? error.message : String(error),
        }),
      )
    }
  })

  await new Promise(resolve => server.listen(listenPort, resolve))

  return {
    url: new URL(`http://localhost:${listenPort}`),
  }
}
