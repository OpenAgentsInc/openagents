export type AssignmentClaimValidationResult = {
  ok: boolean
  assignmentRef: string
  errors: string[]
}

export function validateAssignmentClaim(input: {
  assignmentRef: unknown
  state: unknown
}): AssignmentClaimValidationResult {
  const errors: string[] = []
  const assignmentRef = parseAssignmentRef(input.assignmentRef, errors)

  if (input.state !== "open") {
    errors.push("state must be open")
  }

  return {
    ok: errors.length === 0,
    assignmentRef,
    errors,
  }
}

function parseAssignmentRef(value: unknown, errors: string[]): string {
  if (typeof value !== "string") {
    errors.push("assignmentRef must be a non-empty string")
    return ""
  }

  const assignmentRef = value.trim()
  if (assignmentRef.length === 0) {
    errors.push("assignmentRef must be a non-empty string")
  }

  return assignmentRef
}
