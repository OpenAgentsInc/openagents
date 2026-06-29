/**
 * Coding-adapter selection policy (CX5 #4792, epic #4793).
 *
 * One rule set for choosing which local coding adapter a placed Pylon
 * assignment uses, made deterministic and testable:
 *
 * 1. Requester intent wins: an explicit requested adapter, or a
 *    required capability ref naming one adapter, selects that adapter
 *    and refuses placement on a Pylon that cannot honor it.
 * 2. Adapter-agnostic orders fall to the owner's declared capabilities
 *    (the owner's config preference manifests as which lanes the Pylon
 *    declares: disabling a lane in `~/.pylon/config.json` strips its
 *    capability at go-online). A single-capability Pylon gets its one
 *    adapter.
 * 3. Dual-capability Pylons get the documented platform default,
 *    `claude_agent` — matching the Pylon executor chain order.
 * 4. No silent substitution, ever: the selection names the adapter,
 *    the emitted work class is adapter-specific, and the Pylon-side
 *    gates execute only their own work class, so the closeout's run
 *    and result refs always name the adapter that actually ran.
 */

export const CLAUDE_AGENT_ADAPTER = 'claude_agent' as const
export const CODEX_ADAPTER = 'codex' as const
export type AutopilotCodingAdapter =
  | typeof CLAUDE_AGENT_ADAPTER
  | typeof CODEX_ADAPTER

export const adapterCapabilityRefs: Readonly<
  Record<AutopilotCodingAdapter, string>
> = {
  [CLAUDE_AGENT_ADAPTER]: 'capability.pylon.local_claude_agent',
  [CODEX_ADAPTER]: 'capability.pylon.local_codex',
}

export const adapterJobKinds: Readonly<
  Record<AutopilotCodingAdapter, 'claude_agent_task' | 'codex_agent_task'>
> = {
  [CLAUDE_AGENT_ADAPTER]: 'claude_agent_task',
  [CODEX_ADAPTER]: 'codex_agent_task',
}

export const DEFAULT_CODING_ADAPTER: AutopilotCodingAdapter =
  CLAUDE_AGENT_ADAPTER

export type AdapterSelectionInput = Readonly<{
  /** Explicit requester adapter ask, when the order names one. */
  requestedAdapter?: AutopilotCodingAdapter | undefined
  /**
   * Required capability refs carried by the order; an adapter
   * capability ref here is requester intent, same as requestedAdapter.
   */
  requiredCapabilityRefs?: ReadonlyArray<string> | undefined
  /** The placed Pylon's declared capability refs. */
  pylonCapabilityRefs: ReadonlyArray<string>
}>

export type AdapterSelection =
  | Readonly<{
      selected: true
      adapter: AutopilotCodingAdapter
      capabilityRef: string
      jobKind: 'claude_agent_task' | 'codex_agent_task'
      reasonRef:
        | 'adapter_selection.requester_required'
        | 'adapter_selection.single_capability'
        | 'adapter_selection.dual_capability_default'
    }>
  | Readonly<{
      selected: false
      blockerRefs: ReadonlyArray<string>
    }>

const adapterFromCapabilityRef = (
  ref: string,
): AutopilotCodingAdapter | undefined => {
  if (ref === adapterCapabilityRefs[CLAUDE_AGENT_ADAPTER]) {
    return CLAUDE_AGENT_ADAPTER
  }
  if (ref === adapterCapabilityRefs[CODEX_ADAPTER]) {
    return CODEX_ADAPTER
  }
  return undefined
}

export const selectCodingAdapter = (
  input: AdapterSelectionInput,
): AdapterSelection => {
  const pylonAdapters = new Set<AutopilotCodingAdapter>()
  for (const ref of input.pylonCapabilityRefs) {
    const adapter = adapterFromCapabilityRef(ref)
    if (adapter !== undefined) {
      pylonAdapters.add(adapter)
    }
  }

  const requiredAdapters = new Set<AutopilotCodingAdapter>()
  if (input.requestedAdapter !== undefined) {
    requiredAdapters.add(input.requestedAdapter)
  }
  for (const ref of input.requiredCapabilityRefs ?? []) {
    const adapter = adapterFromCapabilityRef(ref)
    if (adapter !== undefined) {
      requiredAdapters.add(adapter)
    }
  }

  if (requiredAdapters.size > 1) {
    return {
      blockerRefs: [
        'blocker.adapter_selection.conflicting_adapter_requirements',
      ],
      selected: false,
    }
  }

  const required = [...requiredAdapters][0]
  if (required !== undefined) {
    if (!pylonAdapters.has(required)) {
      return {
        blockerRefs: [
          `blocker.adapter_selection.required_adapter_unavailable.${required}`,
        ],
        selected: false,
      }
    }
    return {
      adapter: required,
      capabilityRef: adapterCapabilityRefs[required],
      jobKind: adapterJobKinds[required],
      reasonRef: 'adapter_selection.requester_required',
      selected: true,
    }
  }

  if (pylonAdapters.size === 0) {
    return {
      blockerRefs: ['blocker.adapter_selection.no_coding_adapter_capability'],
      selected: false,
    }
  }

  if (pylonAdapters.size === 1) {
    const adapter = [...pylonAdapters][0]!
    return {
      adapter,
      capabilityRef: adapterCapabilityRefs[adapter],
      jobKind: adapterJobKinds[adapter],
      reasonRef: 'adapter_selection.single_capability',
      selected: true,
    }
  }

  return {
    adapter: DEFAULT_CODING_ADAPTER,
    capabilityRef: adapterCapabilityRefs[DEFAULT_CODING_ADAPTER],
    jobKind: adapterJobKinds[DEFAULT_CODING_ADAPTER],
    reasonRef: 'adapter_selection.dual_capability_default',
    selected: true,
  }
}
