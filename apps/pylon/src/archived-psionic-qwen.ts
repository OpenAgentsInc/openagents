export const PSIONIC_QWEN_MODEL_REFS = {
  qwen35_0_8b: "model.psionic.qwen35.0_8b.q8_0.archived",
  qwen35_2b: "model.psionic.qwen35.2b.q8_0.archived",
} as const

export type PsionicQwenTaskMode = "coding_agent" | "requires_2b"

export type PsionicQwenModelAdmission = Readonly<{
  rows: ReadonlyArray<unknown>
  admittedModelRefs: ReadonlyArray<string>
  observedModelRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}>

export const selectPsionicQwenModel = (
  admission: PsionicQwenModelAdmission,
  _mode: PsionicQwenTaskMode,
): Readonly<{
  admitted: false
  selectedModelRef: string | null
  blockerRefs: ReadonlyArray<string>
}> => ({
  admitted: false,
  selectedModelRef: null,
  blockerRefs:
    admission.blockerRefs.length > 0
      ? admission.blockerRefs
      : ["blocker.psionic_qwen35.archived_to_backroom"],
})
