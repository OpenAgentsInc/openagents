export type AssignmentAcceptCommand = {
  type: "assignments.accept"
  leaseRef: string
}

export type AssignmentAcceptCommandResult = {
  ok: boolean
  command: AssignmentAcceptCommand | null
  errors: string[]
}

export function buildAssignmentAccept(input: {
  leaseRef: unknown
  state: unknown
}): AssignmentAcceptCommandResult {
  const errors: string[] = []
  const leaseRef = parseLeaseRef(input.leaseRef, errors)

  if (input.state !== "open") {
    errors.push("state must be open")
  }

  if (errors.length > 0) {
    return {
      ok: false,
      command: null,
      errors,
    }
  }

  return {
    ok: true,
    command: {
      type: "assignments.accept",
      leaseRef,
    },
    errors,
  }
}

function parseLeaseRef(value: unknown, errors: string[]): string {
  if (typeof value !== "string") {
    errors.push("leaseRef must be a non-empty string")
    return ""
  }

  const leaseRef = value.trim()
  if (leaseRef.length === 0) {
    errors.push("leaseRef must be a non-empty string")
  }

  return leaseRef
}
