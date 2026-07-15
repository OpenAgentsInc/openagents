/**
 * Fail-soft compare-mode observability.
 *
 * The storage/domain read path owns the sample; the runtime may inject any
 * Google Cloud-compatible sink. An absent or failing sink never affects the
 * served read.
 */

export type CompareSoakOutcome = "match" | "mismatch" | "error"

export type CompareSoakSample = Readonly<{
  domain: string
  readKind: string
  outcome: CompareSoakOutcome
}>

export type CompareSoakMetrics = Readonly<{
  record: (sample: CompareSoakSample) => void
}>

export type CompareSoakSink = Readonly<{
  write: (sample: CompareSoakSample) => void
}>

export const makeCompareSoakMetrics = (
  sink: CompareSoakSink | undefined,
): CompareSoakMetrics => ({
  record: sample => {
    try {
      sink?.write(sample)
    } catch {
      // Observability never changes the underlying read result.
    }
  },
})

export const noopCompareSoakMetrics: CompareSoakMetrics = {
  record: () => {},
}
