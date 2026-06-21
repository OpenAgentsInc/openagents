import {
  detectExistingPylonIdentity,
  loadIdentityChoice,
} from "./identity-choice.js"

export type FirstRunLaunchChoice = {
  readonly choiceMade: boolean
  readonly chosenExistingHome: string | null
  readonly chosenDisplayName: string | null
}

export type ResolveFirstRunLaunchChoiceOptions = Parameters<
  typeof loadIdentityChoice
>[0] &
  Parameters<typeof detectExistingPylonIdentity>[0]

export const resolveFirstRunLaunchChoice = (
  options: ResolveFirstRunLaunchChoiceOptions = {},
): FirstRunLaunchChoice => {
  const choice = loadIdentityChoice(options)
  if (choice === null) {
    return {
      choiceMade: false,
      chosenExistingHome: null,
      chosenDisplayName: null,
    }
  }

  if (choice.kind === "create_new") {
    return {
      choiceMade: true,
      chosenExistingHome: null,
      chosenDisplayName: choice.displayName,
    }
  }

  if (choice.home === null) {
    return {
      choiceMade: false,
      chosenExistingHome: null,
      chosenDisplayName: null,
    }
  }

  const detected = detectExistingPylonIdentity(options)
  if (detected === null || detected.home !== choice.home) {
    return {
      choiceMade: false,
      chosenExistingHome: null,
      chosenDisplayName: null,
    }
  }

  return {
    choiceMade: true,
    chosenExistingHome: choice.home,
    chosenDisplayName: null,
  }
}
