export type EvalCaseRef = string

export type EvalBucketResult = {
  readonly caseRef: EvalCaseRef
  readonly passed: boolean
}

export type EvalBucketSummary = {
  readonly total: number
  readonly passed: number
  readonly failed: number
  readonly passRate: number
}

export type EvalRegressionResult = {
  readonly regressed: EvalCaseRef[]
  readonly newlyPassing: EvalCaseRef[]
  readonly isRegression: boolean
}

export function summarizeBucket(
  results: readonly EvalBucketResult[],
): EvalBucketSummary {
  const passed = results.filter((result) => result.passed).length
  const total = results.length
  const failed = total - passed

  return {
    total,
    passed,
    failed,
    passRate: total === 0 ? 0 : passed / total,
  }
}

export function detectRegression(
  baseline: readonly EvalBucketResult[],
  current: readonly EvalBucketResult[],
): EvalRegressionResult {
  const currentByCaseRef = new Map(
    current.map((result) => [result.caseRef, result.passed]),
  )
  const baselineByCaseRef = new Map(
    baseline.map((result) => [result.caseRef, result.passed]),
  )

  const regressed = baseline
    .filter(
      (result) =>
        result.passed === true && currentByCaseRef.get(result.caseRef) === false,
    )
    .map((result) => result.caseRef)

  const newlyPassing = current
    .filter(
      (result) =>
        result.passed === true && baselineByCaseRef.get(result.caseRef) === false,
    )
    .map((result) => result.caseRef)

  return {
    regressed,
    newlyPassing,
    isRegression: regressed.length > 0,
  }
}
