export type SessionRunState = "running" | "paused" | "interrupted" | "completed"

export type SteerVerb = "steer" | "interrupt" | "pause" | "resume"

export type SteerResult = {
  next: SessionRunState
  accepted: boolean
  reason: string
}

export function applySteer(
  current: SessionRunState,
  verb: SteerVerb,
): SteerResult {
  switch (verb) {
    case "steer":
      if (current === "running" || current === "paused") {
        return {
          next: current,
          accepted: true,
          reason: `Steer accepted while session is ${current}.`,
        }
      }

      return reject(current, verb, "steer is only accepted while running or paused")

    case "pause":
      if (current === "running") {
        return {
          next: "paused",
          accepted: true,
          reason: "Pause accepted from running.",
        }
      }

      return reject(current, verb, "pause is only accepted from running")

    case "resume":
      if (current === "paused") {
        return {
          next: "running",
          accepted: true,
          reason: "Resume accepted from paused.",
        }
      }

      return reject(current, verb, "resume is only accepted from paused")

    case "interrupt":
      if (current === "running" || current === "paused") {
        return {
          next: "interrupted",
          accepted: true,
          reason: `Interrupt accepted from ${current}.`,
        }
      }

      return reject(
        current,
        verb,
        "interrupt is only accepted from running or paused",
      )
  }
}

function reject(
  current: SessionRunState,
  verb: SteerVerb,
  rule: string,
): SteerResult {
  return {
    next: current,
    accepted: false,
    reason: `Illegal steer transition: ${verb} from ${current}; ${rule}.`,
  }
}
