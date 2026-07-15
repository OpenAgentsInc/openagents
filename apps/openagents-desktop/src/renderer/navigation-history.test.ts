import { describe, expect, test } from "vite-plus/test";

import {
  commitDesktopNavigationTraversal,
  desktopNavigationTarget,
  dropUnreachableDesktopNavigationTarget,
  initialDesktopNavigationHistory,
  projectDesktopNavigation,
  pushDesktopNavigationDestination,
  type DesktopNavigationDestination,
} from "./navigation-history.ts";

const workspace = (title: string): DesktopNavigationDestination => ({
  kind: "workspace",
  workspace: title === "Files" ? "files" : title === "Home" ? "home" : "chat",
  title,
});

describe("Effect-owned desktop navigation history", () => {
  test("pushes, deduplicates adjacent destinations, traverses, and preserves forward history", () => {
    const chat = workspace("Chat");
    const home = workspace("Home");
    const files = workspace("Files");
    const pushed = pushDesktopNavigationDestination(
      pushDesktopNavigationDestination(initialDesktopNavigationHistory(chat), home),
      files,
    );
    expect(pushDesktopNavigationDestination(pushed, files)).toBe(pushed);
    const back = commitDesktopNavigationTraversal(pushed, "back");
    expect(desktopNavigationTarget(back, "back")).toEqual(chat);
    expect(desktopNavigationTarget(back, "forward")).toEqual(files);
    expect(projectDesktopNavigation(back)).toEqual({
      canGoBack: true,
      canGoForward: true,
      backTitle: "Chat",
      forwardTitle: "Files",
    });
    expect(commitDesktopNavigationTraversal(back, "forward").cursor).toBe(2);
  });

  test("truncates forward history only when a successful new navigation branches", () => {
    const chat = workspace("Chat");
    const home = workspace("Home");
    const files = workspace("Files");
    const review: DesktopNavigationDestination = {
      kind: "workspace",
      workspace: "review",
      title: "Review",
    };
    const atHome = commitDesktopNavigationTraversal(
      pushDesktopNavigationDestination(
        pushDesktopNavigationDestination(initialDesktopNavigationHistory(chat), home),
        files,
      ),
      "back",
    );
    // A failed selection performs no push, so the forward target survives.
    expect(desktopNavigationTarget(atHome, "forward")).toEqual(files);
    const branched = pushDesktopNavigationDestination(atHome, review);
    expect(branched.entries).toEqual([chat, home, review]);
    expect(projectDesktopNavigation(branched).canGoForward).toBe(false);
  });

  test("bounds entries and can discard an unreachable target without moving the current destination", () => {
    const chat = workspace("Chat");
    const home = workspace("Home");
    const files = workspace("Files");
    const bounded = pushDesktopNavigationDestination(
      pushDesktopNavigationDestination(initialDesktopNavigationHistory(chat), home, 2),
      files,
      2,
    );
    expect(bounded.entries).toEqual([home, files]);
    expect(bounded.cursor).toBe(1);
    const withoutHome = dropUnreachableDesktopNavigationTarget(bounded, "back");
    expect(withoutHome.entries).toEqual([files]);
    expect(withoutHome.cursor).toBe(0);
    expect(projectDesktopNavigation(withoutHome).canGoBack).toBe(false);
  });
});
