import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'

const statusStages = [
  ['request', 'Khala request emitted assignment ref'],
  ['run', 'Pylon run-no-spend claimed local Codex work'],
  ['status', 'Trace-status shows private chunks and lifecycle'],
  ['proof', 'Proof checklist validates exact owner-capacity evidence'],
] as const

const eyebrowClass =
  'm-0 font-mono text-xs font-semibold uppercase leading-none tracking-wide text-khala-energy-soft'
const bodyClass = 'm-0 max-w-3xl text-sm/6 text-khala-text-muted'
const commandClass =
  'overflow-x-auto border border-khala-border bg-black/45 p-3 text-xs/6 text-khala-energy-soft'
const codeClass =
  'break-all bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-khala-text'

const statusCopy = (assignmentRef: string): readonly [string, string] => [
  `pylon khala status --assignment-ref ${JSON.stringify(assignmentRef)} --json`,
  `pylon khala proof ${JSON.stringify(assignmentRef)} --json`,
]

function Stage({
  label,
  copy,
}: Readonly<{ label: string; copy: string }>) {
  return (
    <li className="grid gap-2 border border-khala-border bg-white/[0.035] p-3">
      <Badge>{label}</Badge>
      <span className="text-sm/5 text-khala-text-muted">{copy}</span>
    </li>
  )
}

function CommandBlock({ command }: Readonly<{ command: string }>) {
  return (
    <pre className={commandClass}>
      <code>{command}</code>
    </pre>
  )
}

export function PylonCodexAssignmentStatusPage({
  assignmentRef,
}: Readonly<{ assignmentRef: string }>) {
  const [statusCommand, proofCommand] = statusCopy(assignmentRef)

  return (
    <main
      aria-label="Pylon Codex assignment status"
      className="min-h-dvh bg-black text-khala-text"
      data-route="pylon-codex-assignment-status"
    >
      <div className="mx-auto grid min-h-dvh w-full max-w-6xl gap-8 px-4 py-6 font-mono sm:px-6 lg:px-8">
        <Card className="grid gap-5 p-5 sm:p-6">
          <p className={eyebrowClass}>Owner capacity status</p>
          <h1 className="m-0 max-w-3xl text-3xl font-semibold leading-tight tracking-normal text-white sm:text-4xl">
            Pylon Codex assignment
          </h1>
          <p className={bodyClass}>
            This page is the stable operator surface for one Khala coding
            delegation. The live evidence remains owner-scoped: use an agent
            token through the CLI/API to read private trace chunks, final token
            usage, and proof metadata.
          </p>
          <div className="grid gap-2 border border-khala-border bg-black/25 p-3">
            <span className="text-xs font-semibold uppercase leading-none tracking-wide text-khala-text-faint">
              Assignment
            </span>
            <code className={codeClass}>{assignmentRef}</code>
          </div>
        </Card>

        <Card className="grid gap-4 p-4 shadow-xl shadow-black/20 sm:p-5">
          <p className={eyebrowClass}>Closeout path</p>
          <ol className="m-0 grid gap-3 p-0 sm:grid-cols-4">
            {statusStages.map(([label, copy]) => (
              <Stage copy={copy} key={label} label={label} />
            ))}
          </ol>
        </Card>

        <Card className="grid gap-4 p-4 shadow-xl shadow-black/20 sm:p-5">
          <p className={eyebrowClass}>Owner-scoped commands</p>
          <p className={bodyClass}>
            The browser page does not ask for or store an agent token. Run these
            locally from the owning Pylon environment for live data.
          </p>
          <CommandBlock command={statusCommand} />
          <CommandBlock command={proofCommand} />
        </Card>

        <Card className="grid gap-4 p-4 shadow-xl shadow-black/20 sm:p-5">
          <p className={eyebrowClass}>Green evidence</p>
          <p className={bodyClass}>
            The promise can go green for this assignment only when status shows
            closeout-ready or closed-out lifecycle evidence and proof returns an
            empty <code className={codeClass}>proofChecklist.blockerRefs</code>{' '}
            array with exact{' '}
            <code className={codeClass}>pylon-codex-own-capacity</code> token
            rows.
          </p>
        </Card>
      </div>
    </main>
  )
}
