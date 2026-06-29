const baseUrl = process.env.MDK_SIDECAR_BASE_URL ?? 'http://localhost:8080'

const health = await fetch(`${baseUrl.replace(/\/$/, '')}/healthz`)
const body = await health.json().catch(() => ({}))

if (!health.ok) {
  console.error(JSON.stringify({ status: health.status, body }, null, 2))
  process.exit(1)
}

console.log(JSON.stringify({ status: health.status, body }, null, 2))
