export type Tone = "neutral" | "success" | "warning" | "danger" | "info"

export type VerifyState = Readonly<{
  command: readonly string[]
  status: "pending" | "passed" | "failed"
  requiredArtifacts: ReadonlyArray<
    Readonly<{
      ref: string
      present: boolean
    }>
  >
}>

export type RequiredArtifactViewModel = {
  ref: string
  status: "present" | "missing"
  tone: Extract<Tone, "success" | "danger">
}

export type VerifyViewModel = {
  command: string
  status: VerifyState["status"]
  statusTone: Extract<Tone, "success" | "warning" | "danger">
  requiredArtifacts: RequiredArtifactViewModel[]
}

const verifyStatusTone = (
  status: VerifyState["status"],
): VerifyViewModel["statusTone"] => {
  switch (status) {
    case "passed":
      return "success"
    case "failed":
      return "danger"
    case "pending":
      return "warning"
  }
}

const artifactStatus = (present: boolean): RequiredArtifactViewModel["status"] =>
  present ? "present" : "missing"

const shellQuote = (part: string): string =>
  /^[A-Za-z0-9_./:=@%+-]+$/.test(part) ? part : `'${part.replaceAll("'", "'\\''")}'`

export function verifyViewModel(state: VerifyState): VerifyViewModel {
  return {
    command: state.command.map(shellQuote).join(" "),
    status: state.status,
    statusTone: verifyStatusTone(state.status),
    requiredArtifacts: state.requiredArtifacts.map((artifact) => ({
      ref: artifact.ref,
      status: artifactStatus(artifact.present),
      tone: artifact.present ? "success" : "danger",
    })),
  }
}
