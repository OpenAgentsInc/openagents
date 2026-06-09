# Autopilot First-Run Onboarding Planning Prompt

You are helping plan a first-run onboarding experience for the `Autopilot` desktop app.

Your job is to produce a detailed implementation plan for product, UX, engineering, analytics, and rollout. Do not write code. Do not write generic advice. Produce a concrete plan that a product and engineering team could review and execute.

### Primary Goal

Improve the onboarding experience for first-time users who download the `Autopilot` app for the first time.

### North-Star Metric

The onboarding flow must map directly to our current top-line metric:

`Total number of Bitcoin earned by the user base via Autopilot`

That means the onboarding should optimize for reducing friction between install and the user reaching the point where they can go online, receive jobs, and earn Bitcoin.

### Product Background

The current primary GUI view is the `Mission Control` screen.

In Mission Control, the dominant call to action is a highlighted button labeled `Go Online`.

When a user goes online, they become eligible to receive jobs their AI can do, which in turn earns them Bitcoin.

This means the onboarding should not distract from Mission Control. It should guide the user into it, explain just enough, and move them toward pressing `Go Online` as efficiently as possible.

### Planning Task

Create a plan for a simple onboarding flow with two primary stages:

1. `User Setup`
2. `Mission Control Tour`

The plan should treat this as a first-run experience for brand-new users.

## Functional Requirements

### 1. User Setup

When a new user installs and opens the app for the first time:

- Show a centered popup window frame.
- The popup should follow the same visual design pattern as the `Sell Compute` container in the `Mission Control` view.
- Add a dark blurred overlay above the Mission Control background so the popup is clearly in focus.
- Mission Control should remain visible behind the overlay, but visually de-emphasized.

#### Popup Title

Use this exact title:

`\\ Initializing User Account`

#### Popup Layout

The popup should use two columns:

- A larger column on the left
- A smaller column on the right

#### Left Column Content

Show three setup steps with clear visual status indicators:

1. `Lightning wallet setup`
2. `Network configuration`
3. `Establishing connection`

Behavior requirements:

- Only one step should appear actively highlighted at a time.
- The active step should be visually distinct so the user clearly understands what is currently in progress.
- When a step completes, show a green checkmark to the left of the step label.
- Then move the highlight to the next step.
- Continue this pattern until all three steps are complete.

#### Primary CTA

Below the setup steps, include a large button labeled:

`Start Earning Bitcoin`

Button behavior:

- It should begin in an inactive or grayed-out state.
- It should remain inactive until all three setup steps are complete.
- Once the steps complete, it should become active.
- The user must click this button to dismiss the popup.

#### Right Column Content

Reserve this area for an animation using `lottiefiles.com`.

This planning exercise should assume animation support is desirable, but the plan should call out implementation considerations and risk if the current app stack does not support Lottie natively.

### 2. Mission Control Tour

After the user dismisses the setup popup, guide them through a short product tour inside Mission Control.

This tour should highlight two areas:

1. `Hotkeys`
2. `Sell Compute`

#### Shared Tour Behavior

- Use a dark blurred overlay to cover or de-emphasize the rest of the GUI.
- Only the currently highlighted target should remain visually in focus.
- Each step should have a small floating explanatory container.
- The tour should feel brief, clear, and skippable.

#### Tour Step 1: Hotkeys

Focus the bottom-left hotkey commands.

Add a floating container centered over the Hotkeys area with:

- Label: `Hotkeys`
- A right-facing arrow that advances the user to the next tour step

#### Tour Step 2: Sell Compute

Highlight the `Sell Compute` container and de-emphasize the rest of the UI with the same dark blur overlay.

Place a floating container to the right of the `Sell Compute` container with:

- Title: `Earn Bitcoin`
- Supporting text: `Sell your compute to other agents and stack sats in the process.`
- An `X` icon in the top-right corner of the floating container

Closing behavior:

- Clicking the `X` should end the tour
- The overlay should be removed
- Full Mission Control should become visible again

## Style Requirements

### Tour Floating Container

The floating container used in the tour should have:

- White background
- Black text
- Border radius: `3px`

### Caret Direction

The floating container should include a caret or pointer element:

- In tour step 1, the caret should point downward toward the Hotkeys area
- In tour step 2, the caret should point left toward the `Sell Compute` container

## Design and UX Intent

The resulting onboarding should:

- Feel lightweight and fast
- Keep the user oriented inside Mission Control
- Explain only the minimum needed to get the user earning
- Reinforce that the product outcome is earning Bitcoin
- Build confidence without making the app feel complex or technical

## Constraints and Planning Assumptions

Please assume the following:

- This is for the existing `Autopilot` desktop app
- `Mission Control` is already the main app surface
- The plan should work with the current design language rather than proposing a total redesign
- The onboarding should feel additive to the current product, not like a new product shell
- The plan should identify any dependency on animation runtime support, modal/overlay infrastructure, local setup state, persistence, first-run detection, and analytics instrumentation

## What the Plan Must Include

Produce a response with the following sections:

1. `Executive Summary`
2. `User Journey`
3. `Product Requirements Breakdown`
4. `UX and Interaction Plan`
5. `Technical Architecture Considerations`
6. `State Management and Persistence Requirements`
7. `Analytics and Success Metrics`
8. `Edge Cases and Failure States`
9. `Implementation Phases`
10. `Testing Strategy`
11. `Rollout Strategy`
12. `Open Questions and Risks`

## Additional Guidance

Please be opinionated and practical.

- Prioritize time-to-value
- Tie decisions back to the Bitcoin-earned metric
- Distinguish between must-have scope and nice-to-have scope
- Call out where animation or visual polish may be deferred if it threatens delivery speed
- If appropriate, propose instrumentation for:
  - first app open
  - setup started
  - setup completed
  - CTA clicked
  - tour started
  - tour completed
  - `Go Online` clicked after onboarding
  - first successful earning event

The output should read like a planning document that product, design, and engineering can use to align on execution.

---

## Notes for the Human Using This Prompt

- This prompt is intentionally framed to generate a plan, not implementation code.
- If needed, attach screenshots of the current `Mission Control` UI and `Sell Compute` styling so the planning AI can better map the proposal to the current interface.
- If you want a stronger growth orientation, ask the AI to explicitly optimize for `time-to-first-sat`.
