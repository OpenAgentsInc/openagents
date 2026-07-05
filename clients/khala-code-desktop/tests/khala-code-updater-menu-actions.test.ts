import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_UPDATER_MENU_ACTION_CHECK_FOR_UPDATES,
  KHALA_CODE_UPDATER_MENU_ACTION_RELEASE_NOTES,
  handleKhalaCodeApplicationMenuAction,
} from "../src/bun/khala-code-updater-menu-actions"
import { khalaCodeDesktopApplicationMenu } from "../src/bun/application-menu"

describe("Khala Code application menu updater actions (#8440)", () => {
  test("the native menu exposes a Help menu with Check for Updates and Release Notes actions", () => {
    const help = khalaCodeDesktopApplicationMenu.find(
      item => "label" in item && item.label === "Help",
    ) as { readonly label: string; readonly submenu: readonly { readonly action?: string }[] } | undefined
    expect(help).toBeDefined()
    const actions = (help?.submenu ?? []).map(item => item.action)
    expect(actions).toContain(KHALA_CODE_UPDATER_MENU_ACTION_CHECK_FOR_UPDATES)
    expect(actions).toContain(KHALA_CODE_UPDATER_MENU_ACTION_RELEASE_NOTES)
  })

  test("dispatches the check-for-updates action", () => {
    let checked = 0
    let opened = 0
    const handled = handleKhalaCodeApplicationMenuAction(KHALA_CODE_UPDATER_MENU_ACTION_CHECK_FOR_UPDATES, {
      checkForUpdates: () => {
        checked += 1
      },
      openReleaseNotes: () => {
        opened += 1
      },
    })
    expect(handled).toBe(true)
    expect(checked).toBe(1)
    expect(opened).toBe(0)
  })

  test("dispatches the release-notes action", () => {
    let opened = 0
    const handled = handleKhalaCodeApplicationMenuAction(KHALA_CODE_UPDATER_MENU_ACTION_RELEASE_NOTES, {
      checkForUpdates: () => {},
      openReleaseNotes: () => {
        opened += 1
      },
    })
    expect(handled).toBe(true)
    expect(opened).toBe(1)
  })

  test("ignores unrelated or missing menu action ids", () => {
    let calls = 0
    const deps = {
      checkForUpdates: () => {
        calls += 1
      },
      openReleaseNotes: () => {
        calls += 1
      },
    }
    expect(handleKhalaCodeApplicationMenuAction("some-other-action", deps)).toBe(false)
    expect(handleKhalaCodeApplicationMenuAction(undefined, deps)).toBe(false)
    expect(handleKhalaCodeApplicationMenuAction(null, deps)).toBe(false)
    expect(calls).toBe(0)
  })
})
