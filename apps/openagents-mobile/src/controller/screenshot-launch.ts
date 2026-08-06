import * as FileSystem from "expo-file-system/legacy";

const SCREENSHOT_LAUNCH_FILE = "openagents-screenshot-launch.json";
const SCREENSHOT_LAUNCH_VERSION = "openagents.mobile_screenshot_launch.v1";

type ScreenshotLaunch = Readonly<{
  grant: string;
  url: string;
}>;

let pendingLaunch: Promise<ScreenshotLaunch | null> | null = null;

const decodeLaunch = (input: unknown): ScreenshotLaunch | null => {
  if (typeof input !== "object" || input === null || Array.isArray(input)) return null;
  const value = input as Record<string, unknown>;
  if (
    Object.keys(value).length !== 3 ||
    value.version !== SCREENSHOT_LAUNCH_VERSION ||
    typeof value.grant !== "string" ||
    value.grant.length < 32 ||
    value.grant.length > 4_096 ||
    typeof value.url !== "string" ||
    value.url.length < 1 ||
    value.url.length > 2_048 ||
    !value.url.startsWith("openagents://")
  ) {
    return null;
  }
  return { grant: value.grant, url: value.url };
};

const loadLaunch = async (): Promise<ScreenshotLaunch | null> => {
  if (FileSystem.documentDirectory === null) return null;
  const path = `${FileSystem.documentDirectory}${SCREENSHOT_LAUNCH_FILE}`;
  const info = await FileSystem.getInfoAsync(path);
  if (!info.exists) return null;
  try {
    return decodeLaunch(JSON.parse(await FileSystem.readAsStringAsync(path)));
  } catch {
    return null;
  }
};

/**
 * Release screenshot builds receive a signed, short-lived Pro grant through
 * their simulator sandbox. The file is not an auth bypass: the app still
 * exchanges the grant with Pro and receives the normal read-only Convex JWT.
 */
export const readScreenshotLaunch = () => {
  pendingLaunch ??= loadLaunch();
  return pendingLaunch;
};

/**
 * The disposable screenshot harness updates the private launch file to move
 * between routes without invoking iOS's custom-scheme confirmation sheet.
 * Ordinary installs do not have the signed launch file, so they never poll.
 */
export const watchScreenshotLaunch = (listener: (url: string) => void) => {
  let disposed = false;
  let timer: ReturnType<typeof setInterval> | null = null;
  let reading = false;

  void readScreenshotLaunch().then((initial) => {
    if (disposed || initial === null) return;
    let currentUrl = initial.url;
    timer = setInterval(() => {
      if (reading) return;
      reading = true;
      void loadLaunch()
        .then((next) => {
          if (disposed || next === null || next.url === currentUrl) return;
          currentUrl = next.url;
          listener(next.url);
        })
        .finally(() => {
          reading = false;
        });
    }, 100);
  });

  return () => {
    disposed = true;
    if (timer !== null) clearInterval(timer);
  };
};
