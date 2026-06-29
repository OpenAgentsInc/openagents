export type ModelProviderModel = {
  readonly id: string
  readonly capabilities: readonly string[]
}

export type Provider = {
  readonly id: string
  readonly models: readonly ModelProviderModel[]
  readonly available: boolean
}

export type ModelSelectionRequest = {
  readonly requiredCapabilities: readonly string[]
  readonly preferred?: string
}

export type ModelSelection = {
  readonly provider: Provider
  readonly model: ModelProviderModel
}

export type ModelSelectionResult = {
  readonly chosen?: ModelSelection
  readonly fallbackOrder: readonly ModelSelection[]
  readonly reason: string
}

export function selectModel(
  request: ModelSelectionRequest,
  providers: readonly Provider[],
): ModelSelectionResult {
  const fallbackOrder = providers.flatMap((provider) =>
    provider.available
      ? provider.models
          .filter((model) =>
            hasRequiredCapabilities(model, request.requiredCapabilities),
          )
          .map((model) => ({ provider, model }))
      : [],
  )

  if (request.preferred) {
    const preferred = fallbackOrder.find(
      (selection) => selection.model.id === request.preferred,
    )

    if (preferred) {
      return {
        chosen: preferred,
        fallbackOrder,
        reason: "preferred_model_selected",
      }
    }
  }

  const chosen = fallbackOrder[0]

  if (chosen) {
    return {
      chosen,
      fallbackOrder,
      reason: request.preferred
        ? "preferred_model_unavailable_using_fallback"
        : "first_available_capable_model_selected",
    }
  }

  return {
    fallbackOrder,
    reason: "no_available_model_matches_required_capabilities",
  }
}

function hasRequiredCapabilities(
  model: ModelProviderModel,
  requiredCapabilities: readonly string[],
): boolean {
  return requiredCapabilities.every((capability) =>
    model.capabilities.includes(capability),
  )
}
