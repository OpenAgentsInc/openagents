import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { PylonCodexAssignmentStatusPage } from './-pylon-codex-assignment-status-page'

const assignmentRef = 'assignment.public.khala_coding.chatcmpl_example'

describe('Start Pylon Codex assignment status route', () => {
  test('renders the assignment ref and owner-scoped closeout commands', () => {
    const html = renderToStaticMarkup(
      <PylonCodexAssignmentStatusPage assignmentRef={assignmentRef} />,
    )

    expect(html).toContain('data-route="pylon-codex-assignment-status"')
    expect(html).toContain('Pylon Codex assignment')
    expect(html).toContain(assignmentRef)
    expect(html).toContain('pylon khala status --assignment-ref')
    expect(html).toContain('pylon khala proof')
    expect(html).toContain('proofChecklist.blockerRefs')
    expect(html).toContain('pylon-codex-own-capacity')
    expect(html).not.toMatch(/rawEventsJson|safe_metadata_json|bearer/i)
  })
})
