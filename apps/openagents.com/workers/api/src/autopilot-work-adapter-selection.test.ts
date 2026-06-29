import { describe, expect, it } from 'vitest'
import {
  CLAUDE_AGENT_ADAPTER,
  CODEX_ADAPTER,
  adapterCapabilityRefs,
  selectCodingAdapter,
} from './autopilot-work-adapter-selection'

const claudeRef = adapterCapabilityRefs[CLAUDE_AGENT_ADAPTER]
const codexRef = adapterCapabilityRefs[CODEX_ADAPTER]

describe('selectCodingAdapter', () => {
  it('honors a codex requirement on a codex-capable pylon', () => {
    const selection = selectCodingAdapter({
      pylonCapabilityRefs: [claudeRef, codexRef],
      requiredCapabilityRefs: [codexRef],
    })
    expect(selection).toMatchObject({
      adapter: CODEX_ADAPTER,
      jobKind: 'codex_agent_task',
      reasonRef: 'adapter_selection.requester_required',
      selected: true,
    })
  })

  it('honors a claude requirement on a claude-capable pylon', () => {
    const selection = selectCodingAdapter({
      pylonCapabilityRefs: [claudeRef],
      requiredCapabilityRefs: [claudeRef],
    })
    expect(selection).toMatchObject({
      adapter: CLAUDE_AGENT_ADAPTER,
      jobKind: 'claude_agent_task',
      selected: true,
    })
  })

  it('refuses, never substitutes, when the required adapter is missing', () => {
    const selection = selectCodingAdapter({
      pylonCapabilityRefs: [claudeRef],
      requiredCapabilityRefs: [codexRef],
    })
    expect(selection).toMatchObject({ selected: false })
    expect(
      selection.selected === false ? selection.blockerRefs : [],
    ).toContain('blocker.adapter_selection.required_adapter_unavailable.codex')
  })

  it('honors an explicit requestedAdapter the same as a capability requirement', () => {
    const selection = selectCodingAdapter({
      pylonCapabilityRefs: [claudeRef, codexRef],
      requestedAdapter: CODEX_ADAPTER,
    })
    expect(selection).toMatchObject({
      adapter: CODEX_ADAPTER,
      reasonRef: 'adapter_selection.requester_required',
      selected: true,
    })
  })

  it('refuses conflicting adapter requirements rather than picking one', () => {
    const selection = selectCodingAdapter({
      pylonCapabilityRefs: [claudeRef, codexRef],
      requestedAdapter: CLAUDE_AGENT_ADAPTER,
      requiredCapabilityRefs: [codexRef],
    })
    expect(selection).toMatchObject({ selected: false })
    expect(
      selection.selected === false ? selection.blockerRefs : [],
    ).toContain('blocker.adapter_selection.conflicting_adapter_requirements')
  })

  it('agnostic + single capability selects that adapter (owner preference via declaration)', () => {
    expect(
      selectCodingAdapter({ pylonCapabilityRefs: [codexRef] }),
    ).toMatchObject({
      adapter: CODEX_ADAPTER,
      jobKind: 'codex_agent_task',
      reasonRef: 'adapter_selection.single_capability',
      selected: true,
    })
    expect(
      selectCodingAdapter({ pylonCapabilityRefs: [claudeRef] }),
    ).toMatchObject({
      adapter: CLAUDE_AGENT_ADAPTER,
      reasonRef: 'adapter_selection.single_capability',
      selected: true,
    })
  })

  it('agnostic + dual capability takes the documented default (claude_agent)', () => {
    const selection = selectCodingAdapter({
      pylonCapabilityRefs: [codexRef, claudeRef],
    })
    expect(selection).toMatchObject({
      adapter: CLAUDE_AGENT_ADAPTER,
      jobKind: 'claude_agent_task',
      reasonRef: 'adapter_selection.dual_capability_default',
      selected: true,
    })
  })

  it('agnostic + no adapter capability refuses with a typed blocker', () => {
    const selection = selectCodingAdapter({
      pylonCapabilityRefs: ['capability.pylon.assignment_ready'],
    })
    expect(selection).toMatchObject({ selected: false })
    expect(
      selection.selected === false ? selection.blockerRefs : [],
    ).toContain('blocker.adapter_selection.no_coding_adapter_capability')
  })
})
