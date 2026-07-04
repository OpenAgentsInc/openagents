import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'

type TreeNode = Readonly<{
  children: ReadonlyArray<TreeNode>
  detail: string
  label: string
  ref: string
  state: 'blocked' | 'ready' | 'running'
}>

const tree: TreeNode = {
  label: 'FrlmConductor',
  ref: 'program_signature.frlm_conductor.v1',
  state: 'ready',
  detail: 'Environment, scheduler, budget policy, trace emitter',
  children: [
    {
      label: 'Run.Init',
      ref: 'trace.rlm.run_init',
      state: 'ready',
      detail: 'Context fragments and root task refs are bounded before fanout',
      children: [],
    },
    {
      label: 'SubQuery.Submit',
      ref: 'program_signature.rlm_leaf_executor.v1',
      state: 'running',
      detail: 'Typed leaf calls may target local, swarm, remote, or Codex lanes',
      children: [
        {
          label: 'Local',
          ref: 'executor.local.ref_only',
          state: 'ready',
          detail: 'Same-device deterministic work and public-safe adapters',
          children: [],
        },
        {
          label: 'Swarm',
          ref: 'executor.nip90.ref_only',
          state: 'running',
          detail: 'Federated work is quorum-scored before composition',
          children: [],
        },
        {
          label: 'Codex',
          ref: 'executor.pylon_codex.ref_only',
          state: 'running',
          detail: 'Owner-local coding turns stay private; token rows are exact',
          children: [],
        },
      ],
    },
    {
      label: 'SubQuery.Return',
      ref: 'evidence.rlm_trace.redacted_operator_projection',
      state: 'ready',
      detail: 'Only refs, counts, states, and gate evidence reach this page',
      children: [],
    },
    {
      label: 'Run.Done',
      ref: 'release_gate.rlm_trace.redacted_operator_projection',
      state: 'blocked',
      detail: 'Final composition stays blocked until every evidence ref exists',
      children: [],
    },
  ],
}

const signatureRefs = [
  'program_signature.frlm_conductor.v1',
  'program_signature.rlm_leaf_executor.v1',
  'program_signature.blueprint_action_submission.evidence_only.v1',
  'autonomous-ops-v1.signature-4.command-execution-source-verified',
] as const

const authorityRows = [
  ['Execution', 'No direct execution authority'],
  ['Payment', 'No payout or settlement authority'],
  ['Claims', 'No public-promise promotion authority'],
  ['Training', 'No checkpoint or training-promotion authority'],
] as const

const eyebrowClass =
  'm-0 font-mono text-xs font-semibold uppercase leading-none tracking-wide text-khala-warning'
const bodyClass = 'm-0 max-w-4xl text-sm/6 text-khala-text-muted'
const codeClass =
  'break-all bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-khala-text'

const stateVariant = (state: TreeNode['state']): 'ready' | 'running' | 'warning' =>
  state === 'ready'
    ? 'ready'
    : state === 'running'
      ? 'running'
      : 'warning'

function CodeInline({ children }: Readonly<{ children: string }>) {
  return <code className={codeClass}>{children}</code>
}

function TreeItem({ node, depth = 0 }: Readonly<{ depth?: number; node: TreeNode }>) {
  const hasChildren = node.children.length > 0

  return (
    <li className="grid gap-3">
      <div
        className={`grid gap-2 border border-khala-border bg-black/25 p-3 ${
          depth === 0 ? '' : 'ml-4 sm:ml-6'
        }`}
      >
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-semibold leading-5 text-white">
            {node.label}
          </span>
          <Badge variant={stateVariant(node.state)}>{node.state}</Badge>
        </div>
        <CodeInline>{node.ref}</CodeInline>
        <p className="m-0 text-sm/5 text-khala-text-muted">{node.detail}</p>
      </div>
      {hasChildren ? (
        <ol className="grid gap-3 border-l border-khala-border pl-3 sm:pl-4">
          {node.children.map(child => (
            <TreeItem depth={depth + 1} key={child.ref} node={child} />
          ))}
        </ol>
      ) : (
        <span className="sr-only">Leaf node</span>
      )}
    </li>
  )
}

export function ArtanisTracesPage() {
  return (
    <main
      aria-label="Artanis RLM traces"
      className="min-h-dvh bg-black text-khala-text"
      data-route="artanis-traces"
    >
      <div className="mx-auto grid min-h-dvh w-full max-w-7xl gap-6 px-4 py-6 font-mono sm:px-6 lg:px-8">
        <Card className="grid gap-5 p-5 sm:p-6">
          <p className={eyebrowClass}>RLM trace visualizer</p>
          <h1 className="m-0 max-w-4xl text-3xl font-semibold leading-tight tracking-normal text-white sm:text-4xl">
            Artanis execution tree
          </h1>
          <p className={bodyClass}>
            A ref-only view of the Recursive Language Model shape behind
            Artanis: conductor, fanout, typed leaf executors, returned evidence,
            and composition gates.
          </p>
          <div className="grid gap-3 border border-khala-border bg-black/25 p-3 text-sm/5 text-khala-text-muted sm:grid-cols-3">
            <div>
              <span className="block text-khala-text-faint">Source</span>
              <CodeInline>/api/operator/rlm/traces</CodeInline>
            </div>
            <div>
              <span className="block text-khala-text-faint">Projection</span>
              <CodeInline>openagents.operator.rlm_traces.v1</CodeInline>
            </div>
            <div>
              <span className="block text-khala-text-faint">Privacy</span>
              <CodeInline>operator_refs_only</CodeInline>
            </div>
          </div>
        </Card>
        <section className="grid gap-6 lg:grid-cols-[1.45fr_0.9fr]">
          <Card className="p-4 sm:p-5">
            <div className="grid gap-4">
              <p className={eyebrowClass}>Execution tree</p>
              <ol className="m-0 grid gap-3 p-0">
                <TreeItem node={tree} />
              </ol>
            </div>
          </Card>
          <aside className="grid content-start gap-6">
            <Card className="p-4 sm:p-5">
              <div className="grid gap-4">
                <p className={eyebrowClass}>Blueprint signatures</p>
                <ul className="m-0 grid gap-2 p-0">
                  {signatureRefs.map(ref => (
                    <li
                      className="border border-khala-border bg-white/[0.035] p-3 text-sm/5 text-khala-text-muted"
                      key={ref}
                    >
                      <CodeInline>{ref}</CodeInline>
                    </li>
                  ))}
                </ul>
              </div>
            </Card>
            <Card className="p-4 sm:p-5">
              <div className="grid gap-4">
                <p className={eyebrowClass}>Authority boundary</p>
                <div className="grid overflow-hidden border border-khala-border text-sm/5">
                  {authorityRows.map(([label, value]) => (
                    <div
                      className="grid gap-2 border-b border-khala-border p-3 last:border-b-0 sm:grid-cols-[8rem_1fr]"
                      key={label}
                    >
                      <span className="font-semibold text-khala-text">
                        {label}
                      </span>
                      <span className="text-khala-text-muted">{value}</span>
                    </div>
                  ))}
                </div>
              </div>
            </Card>
          </aside>
        </section>
      </div>
    </main>
  )
}
