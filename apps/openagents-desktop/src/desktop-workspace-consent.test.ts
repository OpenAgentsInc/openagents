import { describe, expect, test } from "vite-plus/test"

import {
  DESKTOP_WORKSPACE_CONSENT_SCHEMA_ID,
  DESKTOP_WORKSPACE_CONSENT_VERSION,
  desktopWorkspaceConsentPlan,
  type DesktopWorkspaceConsent,
} from "./desktop-workspace-consent.ts"

const grant = (workspaceRoot: string): DesktopWorkspaceConsent => ({
  schemaId: DESKTOP_WORKSPACE_CONSENT_SCHEMA_ID,
  version: DESKTOP_WORKSPACE_CONSENT_VERSION,
  status: "granted",
  workspaceRoot,
  decidedAt: "2026-07-21T00:00:00.000Z",
})

describe("desktopWorkspaceConsentPlan", () => {
  test("never prompts on non-interactive automation launches", () => {
    expect(desktopWorkspaceConsentPlan({
      interactiveLaunch: false,
      consent: null,
      launchFallbackRoot: "/Users/owner",
      isDirectory: () => true,
    })).toEqual({ _tag: "SkipOnboarding", workspaceRoot: "/Users/owner" })
  })

  test("requests consent once on first interactive run", () => {
    expect(desktopWorkspaceConsentPlan({
      interactiveLaunch: true,
      consent: null,
      launchFallbackRoot: "/Users/owner",
      isDirectory: () => true,
    })).toEqual({ _tag: "RequestConsent", defaultPath: "/Users/owner" })
  })

  test("reuses a still-valid granted workspace without re-prompting", () => {
    expect(desktopWorkspaceConsentPlan({
      interactiveLaunch: true,
      consent: grant("/work/openagents"),
      launchFallbackRoot: "/Users/owner",
      isDirectory: candidate => candidate === "/work/openagents",
    })).toEqual({ _tag: "UseConsentedWorkspace", workspaceRoot: "/work/openagents" })
  })

  test("re-asks when a granted workspace folder has vanished", () => {
    expect(desktopWorkspaceConsentPlan({
      interactiveLaunch: true,
      consent: grant("/work/deleted"),
      launchFallbackRoot: "/Users/owner",
      isDirectory: () => false,
    })).toEqual({ _tag: "RequestConsent", defaultPath: "/Users/owner" })
  })

  test("respects a prior decline with the safe fallback and never nags", () => {
    expect(desktopWorkspaceConsentPlan({
      interactiveLaunch: true,
      consent: {
        schemaId: DESKTOP_WORKSPACE_CONSENT_SCHEMA_ID,
        version: DESKTOP_WORKSPACE_CONSENT_VERSION,
        status: "declined",
        workspaceRoot: null,
        decidedAt: "2026-07-21T00:00:00.000Z",
      },
      launchFallbackRoot: "/Users/owner",
      isDirectory: () => true,
    })).toEqual({ _tag: "UseFallbackWorkspace", workspaceRoot: "/Users/owner" })
  })
})
