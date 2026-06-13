export type DeployPollInput = {
  state: string
  attempts: number
}

export type DeployPollState = {
  shouldPoll: boolean
  delayMs: number
  done: boolean
}

const terminalStates = new Set(["deployed", "failed"])

export function nextDeployPoll(input: DeployPollInput): DeployPollState {
  const state = input.state.trim().toLowerCase()

  if (terminalStates.has(state)) {
    return {
      shouldPoll: false,
      delayMs: 0,
      done: true,
    }
  }

  return {
    shouldPoll: true,
    delayMs: Math.min(2000 * input.attempts, 15000),
    done: false,
  }
}
