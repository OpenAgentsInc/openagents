import { describe, expect, test } from "vite-plus/test"

import { projectMobileComposerRunAdmission } from "../src/screens/mobile-composer-run-control"

describe("T3M-B2.3a mobile active-run composer admission", () => {
  test("names idle, queued, running, waiting, confirmation, and stop-pending consequences", () => {
    expect(projectMobileComposerRunAdmission({
      turn: null,
      controlAvailable: true,
      submittingAction: null,
      stopConfirmationRunRef: null,
    })).toMatchObject({ active: false, placeholder: "Continue conversation", stopAvailable: false })

    const queued = projectMobileComposerRunAdmission({
      turn: { runRef: "run.queued", status: "queued" },
      controlAvailable: true,
      submittingAction: null,
      stopConfirmationRunRef: null,
    })
    expect(queued).toMatchObject({
      active: true,
      badge: "Starting",
      placeholder: "Draft while this turn starts",
      stopAvailable: true,
    })
    expect(queued.detail).toContain("exact queued turn")

    const running = projectMobileComposerRunAdmission({
      turn: { runRef: "run.active", status: "running" },
      controlAvailable: true,
      submittingAction: null,
      stopConfirmationRunRef: null,
    })
    expect(running).toMatchObject({
      badge: "Running",
      placeholder: "Queue a follow-up",
      submitLabel: "Queue follow-up",
    })
    expect(running.detail).toContain("after this exact running turn")

    expect(projectMobileComposerRunAdmission({
      turn: { runRef: "run.waiting", status: "waiting_for_input" },
      controlAvailable: true,
      submittingAction: null,
      stopConfirmationRunRef: null,
    }).detail).toContain("after this exact waiting turn")

    expect(projectMobileComposerRunAdmission({
      turn: { runRef: "run.active", status: "running" },
      controlAvailable: true,
      submittingAction: null,
      stopConfirmationRunRef: "run.active",
    })).toMatchObject({ confirming: true, stopAvailable: true })

    expect(projectMobileComposerRunAdmission({
      turn: { runRef: "run.active", status: "running" },
      controlAvailable: true,
      submittingAction: "cancel",
      stopConfirmationRunRef: null,
    })).toMatchObject({ stopping: true, stopAvailable: false })
  })

  test("keeps missing control authority explicit", () => {
    const admission = projectMobileComposerRunAdmission({
      turn: { runRef: "run.remote", status: "running" },
      controlAvailable: false,
      submittingAction: null,
      stopConfirmationRunRef: null,
    })
    expect(admission.stopAvailable).toBe(false)
    expect(admission.detail).toContain("unavailable on this device")
  })
})
