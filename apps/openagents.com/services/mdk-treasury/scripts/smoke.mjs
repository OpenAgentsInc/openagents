// Unconfigured-boot smoke: the treasury container must serve honest healthz
// flags and refuse wallet routes when secrets are absent. No spend, no node.
const port = 18099
const base = `http://127.0.0.1:${port}`

delete process.env.MDK_TREASURY_MNEMONIC
delete process.env.MDK_TREASURY_ACCESS_TOKEN
delete process.env.SPARK_TREASURY_MNEMONIC
delete process.env.SPARK_TREASURY_API_KEY
process.env.MDK_TREASURY_SERVICE_TOKEN = 'smoke-service-token'
process.env.PORT = String(port)

const failures = []
const check = (name, condition) => {
  if (!condition) {
    failures.push(name)
  }
  console.log(`${condition ? 'PASS' : 'FAIL'} ${name}`)
}

const serverProcess = Bun.spawn(
  ['bun', new URL('../src/server.mjs', import.meta.url).pathname],
  { env: process.env, stderr: 'inherit', stdout: 'inherit' },
)

try {
  let healthz = null

  for (let attempt = 0; attempt < 50; attempt += 1) {
    try {
      const response = await fetch(`${base}/healthz`)
      healthz = await response.json()
      break
    } catch {
      await new Promise(resolve => setTimeout(resolve, 100))
    }
  }

  check('healthz responds', healthz !== null)
  check(
    'healthz names the service',
    healthz?.service === 'openagents-mdk-treasury',
  )
  check(
    'healthz reports mnemonic unconfigured',
    healthz?.mnemonicConfigured === false,
  )
  check(
    'healthz reports access token unconfigured',
    healthz?.accessTokenConfigured === false,
  )
  check(
    'healthz reports Spark mnemonic unconfigured',
    healthz?.sparkMnemonicConfigured === false,
  )
  check(
    'healthz reports Spark API key configured',
    healthz?.sparkApiKeyConfigured === true,
  )
  check(
    'healthz reports service token configured',
    healthz?.serviceTokenConfigured === true,
  )
  check(
    'healthz leaks no secret values',
    !JSON.stringify(healthz).includes('smoke-service-token'),
  )

  const unauthorized = await fetch(`${base}/balance`)
  check('balance without service token is 403', unauthorized.status === 403)

  const unconfigured = await fetch(`${base}/balance`, {
    headers: { 'x-treasury-service-token': 'smoke-service-token' },
  })
  check('balance while unconfigured is 503', unconfigured.status === 503)
  const unconfiguredBody = await unconfigured.json()
  check(
    'unconfigured balance names the blocker',
    unconfiguredBody?.error === 'treasury_unconfigured',
  )

  const sparkUnconfigured = await fetch(`${base}/spark/balance`, {
    headers: { 'x-treasury-service-token': 'smoke-service-token' },
  })
  check(
    'Spark balance while unconfigured is 503',
    sparkUnconfigured.status === 503,
  )
  const sparkUnconfiguredBody = await sparkUnconfigured.json()
  check(
    'unconfigured Spark balance names the blocker',
    sparkUnconfiguredBody?.error === 'spark_treasury_unconfigured',
  )

  const payUnconfigured = await fetch(`${base}/pay`, {
    body: JSON.stringify({ amountSat: 1, destination: 'lno1example' }),
    headers: {
      'content-type': 'application/json',
      'x-treasury-service-token': 'smoke-service-token',
    },
    method: 'POST',
  })
  check('pay while unconfigured is 503', payUnconfigured.status === 503)
} finally {
  serverProcess.kill()
}

if (failures.length > 0) {
  console.error(`treasury smoke FAILED: ${failures.join(', ')}`)
  process.exit(1)
}

console.log('treasury unconfigured-boot smoke passed')
