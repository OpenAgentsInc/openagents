#!/usr/bin/env bun

import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const listFiles = dir =>
  readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name)

    return entry.isDirectory() ? listFiles(path) : [path]
  })

const sourceFiles = ['workers/api/src', 'packages', 'apps/web/src']
  .flatMap(listFiles)
  .filter(path => /\.tsx?$/.test(path))
  .filter(path => !/\.test\.tsx?$/.test(path))
  .filter(path => !/\.test-support\.tsx?$/.test(path))
  .filter(path => !/\.story\.test\.tsx?$/.test(path))
  .filter(path => !/\.scene\.test\.tsx?$/.test(path))
  .filter(path => !path.includes('workers/api/src/test/'))

const workerFiles = sourceFiles.filter(path =>
  path.startsWith('workers/api/src/'),
)

const routeFiles = workerFiles.filter(
  path =>
    path.endsWith('-routes.ts') ||
    path.endsWith('/routes.ts') ||
    path.endsWith('/worker-routes.ts') ||
    path.endsWith('/index.ts'),
)

const workerServiceDomainFiles = workerFiles.filter(
  path =>
    !path.includes('/http/') &&
    !routeFiles.includes(path) &&
    !path.endsWith('-handlers.ts'),
)

const read = path => readFileSync(path, 'utf8')

const countMatches = (text, regex) => Array.from(text.matchAll(regex)).length

const countByFile = (files, regex) =>
  files
    .map(path => ({
      count: countMatches(read(path), regex),
      path,
    }))
    .filter(result => result.count > 0)

const totalCount = results =>
  results.reduce((total, result) => total + result.count, 0)

const convertedBrowserDomainFiles = [
  'apps/web/src/page/loggedIn/page/chat.ts',
  'apps/web/src/page/loggedIn/page/files.ts',
  'apps/web/src/page/loggedIn/run-timeline/projection.ts',
]

const browserPolicyConsumerFiles = sourceFiles.filter(
  path =>
    path.startsWith('apps/web/src/') &&
    !path.endsWith('apps/web/src/domain/session.ts') &&
    !path.endsWith('apps/web/src/product-policy.ts'),
)

const runtimePrimitiveBoundaryFiles = new Set([
  'apps/web/src/time-format.ts',
  'packages/sync-worker/src/runtime-primitives.ts',
  'workers/api/src/runtime-primitives.ts',
])

const jsonBoundaryFiles = new Set([
  'apps/web/src/json-boundary.ts',
  'packages/sync-schema/src/json-boundary.ts',
  'packages/sync-worker/src/json-boundary.ts',
  'workers/api/src/json-boundary.ts',
])

const deterministicBusinessLogicFiles = sourceFiles.filter(
  path => !runtimePrimitiveBoundaryFiles.has(path),
)

const lineCount = path => {
  const text = read(path)
  const lines = text.split('\n').length

  return text.endsWith('\n') ? lines - 1 : lines
}

const formatDetails = results =>
  results.map(result => `${result.path}: ${result.count}`).join('\n')

const budgetChecks = [
  {
    budget: 8,
    description:
      'Route modules may not add Promise dependency adapters while route signatures migrate to Effect programs.',
    details: countByFile(
      routeFiles,
      /Effect\.promise\(\s*\(\)\s*=>\s*dependencies\./g,
    ),
    name: 'route dependency Effect.promise adapters',
  },
  {
    budget: 12,
    description:
      'Production Worker modules may not add generic thrown expected errors while typed errors are introduced.',
    details: countByFile(workerFiles, /throw\s+new\s+Error\(/g),
    name: 'Worker throw new Error calls',
  },
  {
    budget: 0,
    description:
      'Provider-account routes must map ProviderAccountError tags, not English error-message substrings.',
    details: countByFile(
      workerFiles.filter(path => path.includes('provider-account')),
      /message\.includes\(\s*['"](?:credential-shaped|expired|not connected|not issued|not pending|does not match|cannot be marked|not found)['"]\s*\)/g,
    ),
    name: 'provider-account string error classifiers',
  },
  {
    budget: 0,
    description:
      'GitHub-write routes must map GitHubWriteError tags, not English error-message substrings.',
    details: countByFile(
      workerFiles.filter(
        path =>
          path.includes('github-write') ||
          path.endsWith('workers/api/src/index.ts'),
      ),
      /message\.includes\(\s*['"](?:expired|not issued|does not match|not usable|missing required scopes)['"]\s*\)/g,
    ),
    name: 'GitHub-write string error classifiers',
  },
  {
    budget: 0,
    description:
      'Domain modules must keep raw JSON.parse inside named boundary decoders.',
    details: countByFile(
      sourceFiles.filter(path => !jsonBoundaryFiles.has(path)),
      /JSON\.parse\(/g,
    ),
    name: 'raw JSON.parse outside json-boundary',
  },
  {
    budget: 0,
    description:
      'Business logic must consume runtime primitive helpers or injected services instead of raw time, UUID, or randomness primitives.',
    details: countByFile(
      deterministicBusinessLogicFiles,
      /\bDate\.now\(|new Date\(|crypto\.randomUUID\(|Math\.random\(/g,
    ),
    name: 'raw time/id/random primitives',
  },
  {
    budget: 160,
    description:
      'Worker modules may not add raw Cloudflare Env parameters outside the future config/binding boundary.',
    details: countByFile(
      workerFiles.filter(
        path =>
          !path.endsWith('workers/api/src/bindings.ts') &&
          !path.endsWith('workers/api/src/config.ts'),
      ),
      /\benv\s*:\s*Env\b/g,
    ),
    name: 'raw Env parameter annotations',
  },
  {
    budget: 0,
    description:
      'Business code must consume OpenAgentsWorkerConfig instead of reading migrated secret/config fields directly from Cloudflare Env.',
    details: countByFile(
      workerFiles.filter(path => !path.endsWith('workers/api/src/config.ts')),
      /\benv\.(GITHUB_CLIENT_ID|GITHUB_CLIENT_SECRET|OPENAGENTS_ADMIN_API_TOKEN|OPENAGENTS_APP_URL|OPENAUTH_CLIENT_ID|OPENAUTH_ISSUER_URL|RESEND_API_KEY|RESEND_FROM_EMAIL|RESEND_REPLY_TO_EMAIL|SHC_CONTROL_API_BEARER_TOKEN|SHC_CONTROL_API_URL|SHC_DISPATCH_MODE|SHC_RUNNER_CALLBACK_TOKEN)\b/g,
    ),
    name: 'direct migrated Worker config Env reads',
  },
  {
    budget: 0,
    description:
      'Worker modules must schedule background work and access sync runtime bindings through the runtime capability services.',
    details: countByFile(
      workerFiles.filter(path => !path.endsWith('workers/api/src/runtime.ts')),
      /\bctx\.waitUntil\(|\benv\.(OPENAGENTS_DB|SYNC_ROOM|RUNNER_EVENTS)\b|\bthis\.env\.OPENAGENTS_DB\b|\bscopeIdFromName\b/g,
    ),
    name: 'direct Worker runtime capability access',
  },
  {
    budget: 0,
    description:
      'Production Worker logging must go through redacted Effect observability helpers, not raw console calls.',
    details: countByFile(workerFiles, /console\.(error|warn|log)\(/g),
    name: 'raw Worker console logging',
  },
  {
    budget: 0,
    description:
      'Service and domain modules must return typed values/errors, leaving HTTP response mapping to route and HTTP modules.',
    details: countByFile(
      workerServiceDomainFiles,
      /\b(noStoreJsonResponse|redirectResponse|methodNotAllowed|forbidden\(|unauthorized\(|serverError\()/g,
    ),
    name: 'service/domain HTTP response helper usage',
  },
  {
    budget: 80,
    description:
      'Worker domain and route modules may not grow Response-returning surfaces while route mappers are extracted.',
    details: countByFile(
      workerFiles.filter(path => !path.includes('/http/')),
      /Promise<Response>|:\s*Response\b|Effect\.Effect<Response/g,
    ),
    name: 'Worker Response return surfaces',
  },
  {
    budget: 0,
    description:
      'Converted browser domains must branch on tagged model state or Option, not raw DTO nulls.',
    details: countByFile(
      convertedBrowserDomainFiles,
      /\.agentRunId\s*(?:===|!==)\s*null|\.durationSeconds\s*(?:===|!==)\s*null|detail\.file\.teamId|\.teamId\s*(?:===|!==)\s*null/g,
    ),
    name: 'converted browser domain raw null branches',
  },
  {
    budget: 0,
    description:
      'Browser product permission checks must go through product-policy.ts instead of calling session helpers directly.',
    details: countByFile(
      browserPolicyConsumerFiles,
      /authHasCoreTeamAccess\(/g,
    ),
    name: 'direct browser Core Team permission checks',
  },
  {
    budget: 0,
    description:
      'Project workroom visibility must be owned by product-policy.ts, not reintroduced as a local UI flag.',
    details: countByFile(sourceFiles, /\bPROJECT_WORKROOMS_ENABLED\b/g),
    name: 'legacy project workroom flag',
  },
  {
    budget: 0,
    description:
      'The deleted local login page and simulated auth flow must not return; root owns the public login surface and /login/github owns auth.',
    details: countByFile(
      sourceFiles,
      /\b(LoginRoute|loginRouter|StartupRedirectToLogin|RedirectToLogin|SimulateAuthRequest|SaveSession|SucceededSaveSession|FailedSaveSession)\b/g,
    ),
    name: 'deleted local login route symbols',
  },
  {
    budget: 0,
    description:
      'The deleted personal /chat alias must not be reintroduced as a Worker redirect to root.',
    details: countByFile(
      workerFiles,
      /path:\s*['"]\/chat['"](?:(?!path:\s*['"])[\s\S])*redirectResponse\(\s*['"]\/['"]\s*\)/g,
    ),
    name: 'legacy Worker /chat redirect alias',
  },
]

const runPromiseAllowlist = new Map([
  ['workers/api/src/index.ts', 5],
  ['workers/api/src/observability.ts', 1],
  ['workers/api/src/omni-handlers.ts', 7],
  ['workers/api/src/thread-access.ts', 1],
  ['workers/api/src/onboarding/repository.ts', 1],
  ['packages/sync-worker/src/index.ts', 1],
])

const runPromiseDetails = countByFile(sourceFiles, /Effect\.runPromise\(/g)

const runPromiseProblems = runPromiseDetails.flatMap(result => {
  const budget = runPromiseAllowlist.get(result.path)

  if (budget === undefined) {
    return [
      `${result.path}: ${result.count} Effect.runPromise call(s) are outside the named temporary bridge allowlist.`,
    ]
  }

  return result.count > budget
    ? [
        `${result.path}: ${result.count} Effect.runPromise call(s), budget ${budget}.`,
      ]
    : []
})

const lineBudgetChecks = [
  {
    budget: 228,
    count: lineCount('apps/web/src/page/loggedIn/update.ts'),
    description:
      'The parent logged-in update must remain a dispatcher over domain command and transition modules.',
    name: 'loggedIn/update.ts lines',
    path: 'apps/web/src/page/loggedIn/update.ts',
  },
  {
    budget: 0,
    count: countMatches(
      read('apps/web/src/page/loggedIn/update.ts'),
      /Command\.define\(/g,
    ),
    description:
      'The parent logged-in update may not define request commands; command definitions belong in domain command modules.',
    name: 'loggedIn/update.ts Command.define count',
    path: 'apps/web/src/page/loggedIn/update.ts',
  },
  {
    budget: 0,
    count: countMatches(
      read('apps/web/src/page/loggedIn/update.ts'),
      /\bevo\(/g,
    ),
    description:
      'The parent logged-in update may not perform direct struct mutations; pure transitions belong in domain transition modules.',
    name: 'loggedIn/update.ts evo count',
    path: 'apps/web/src/page/loggedIn/update.ts',
  },
  {
    budget: 606,
    count: lineCount('apps/web/src/page/loggedIn/page/chat.ts'),
    description:
      'The chat page must stay focused on rendering; run timeline projection belongs in run-timeline/projection.ts.',
    name: 'loggedIn/page/chat.ts lines',
    path: 'apps/web/src/page/loggedIn/page/chat.ts',
  },
  {
    budget: 0,
    count: countMatches(
      read('apps/web/src/page/loggedIn/page/chat.ts'),
      /JSON\.parse\(/g,
    ),
    description:
      'The chat page may not parse runner payload JSON directly; projection parsing belongs in the typed run-timeline module.',
    name: 'loggedIn/page/chat.ts JSON.parse count',
    path: 'apps/web/src/page/loggedIn/page/chat.ts',
  },
]

const deletedFileChecks = [
  {
    description:
      'UI implementations must live in typed family modules rather than the deleted compatibility registry.',
    name: 'ui/registry.ts deleted',
    path: 'apps/web/src/ui/registry.ts',
  },
  {
    description:
      'The deleted local login demo must not return; root plus /login/github are the public auth surface.',
    name: 'loggedOut/page/login.ts deleted',
    path: 'apps/web/src/page/loggedOut/page/login.ts',
  },
  {
    description:
      'The deleted local login demo story must not return with the removed simulated auth flow.',
    name: 'loggedOut/page/login.story.test.ts deleted',
    path: 'apps/web/src/page/loggedOut/page/login.story.test.ts',
  },
]

const budgetProblems = budgetChecks.flatMap(check => {
  const count = totalCount(check.details)

  return count > check.budget
    ? [
        `${check.name}: count ${count}, budget ${check.budget}.\n${formatDetails(check.details)}`,
      ]
    : []
})

const lineBudgetProblems = lineBudgetChecks.flatMap(check =>
  check.count > check.budget
    ? [
        `${check.name}: ${check.path} has ${check.count}, budget ${check.budget}.`,
      ]
    : [],
)

const deletedFileProblems = deletedFileChecks.flatMap(check =>
  existsSync(check.path)
    ? [`${check.name}: ${check.path} must not exist.`]
    : [],
)

console.log('Zero-debt architecture budget report')
console.log('')

budgetChecks.forEach(check => {
  const count = totalCount(check.details)
  console.log(`${check.name}: ${count}/${check.budget}`)
  console.log(`  ${check.description}`)
  console.log(formatDetails(check.details) || '  none')
  console.log('')
})

console.log('Effect.runPromise temporary bridge allowlist:')
runPromiseDetails.forEach(result => {
  const budget = runPromiseAllowlist.get(result.path)
  const budgetText = budget === undefined ? 'not allowed' : `budget ${budget}`
  console.log(`${result.path}: ${result.count} (${budgetText})`)
})
console.log('')

lineBudgetChecks.forEach(check => {
  console.log(`${check.name}: ${check.count}/${check.budget}`)
  console.log(`  ${check.description}`)
})
console.log('')

deletedFileChecks.forEach(check => {
  console.log(
    `${check.name}: ${existsSync(check.path) ? 'present' : 'deleted'}`,
  )
  console.log(`  ${check.description}`)
})
console.log('')

const problems = [
  ...budgetProblems,
  ...runPromiseProblems,
  ...lineBudgetProblems,
  ...deletedFileProblems,
]

if (problems.length > 0) {
  console.error('Zero-debt architecture check failed:')
  problems.forEach(problem => console.error(`- ${problem}`))
  process.exit(1)
}

console.log('Zero-debt architecture check passed.')
