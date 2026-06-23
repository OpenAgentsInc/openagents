// Autopilot onboarding - bounded vertical selection (issue #6148).
//
// The shared onboarding flow threads the optional `/autopilot/{vertical}` segment
// to the onboarding turn transport as a bounded enum. The server owns all
// vertical guidance text; the browser only says which known vertical the route
// selected.

export type AutopilotOnboardingVertical = 'general' | 'legal'

export const onboardingVerticalForSegment = (
  vertical: string | null,
): AutopilotOnboardingVertical => (vertical === 'legal' ? 'legal' : 'general')
