import * as Linking from "expo-linking";

import { controllerLinking } from "./routes";
import { readScreenshotLaunch, watchScreenshotLaunch } from "./screenshot-launch";

export const runtimeControllerLinking = {
  ...controllerLinking,
  getInitialURL: async () =>
    (await readScreenshotLaunch())?.url ?? (await Linking.getInitialURL()),
  subscribe: (listener: (url: string) => void) => {
    const subscription = Linking.addEventListener("url", ({ url }) => listener(url));
    const stopScreenshotLaunch = watchScreenshotLaunch(listener);
    return () => {
      subscription.remove();
      stopScreenshotLaunch();
    };
  },
};
