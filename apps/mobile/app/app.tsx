/* eslint-disable import/first */
/**
 * Welcome to the main entry point of the app. In this file, we'll
 * be kicking off our app.
 *
 * Most of this file is boilerplate and you shouldn't need to modify
 * it very often. But take some time to look through and understand
 * what is going on here.
 *
 * The app navigation resides in ./app/navigators, so head over there
 * if you're interested in adding screens and navigators.
 */
if (__DEV__) {
  // Load Reactotron in development only.
  // Note that you must be using metro's `inlineRequires` for this to work.
  // If you turn it off in metro.config.js, you'll have to manually import it.
  require("./devtools/ReactotronConfig.ts")
}
import "./utils/gestureHandler"

import { useEffect, useState } from "react"
import { useFonts } from "expo-font"
import * as Linking from "expo-linking"
import * as SplashScreen from "expo-splash-screen"
import { ConvexProviderWithAuth } from "convex/react"
import { ConvexReactClient } from "convex/react"
import { KeyboardProvider } from "react-native-keyboard-controller"
import { initialWindowMetrics, SafeAreaProvider } from "react-native-safe-area-context"

import Config from "./config"
import { AuthProvider } from "./context/AuthContext"
import { useConvexAuthFromContext } from "./convex/useConvexAuthFromContext"
import { initI18n } from "./i18n"
import { AppNavigator } from "./navigators/AppNavigator"
import { useNavigationPersistence } from "./navigators/navigationUtilities"
import { ThemeProvider } from "./theme/context"
import { customFontsToLoad } from "./theme/typography"
import { loadDateFnsLocale } from "./utils/formatDate"
import * as storage from "./utils/storage"

export const NAVIGATION_PERSISTENCE_KEY = "NAVIGATION_STATE"

// Web linking configuration
const prefix = Linking.createURL("/")
const config = {
  screens: {
    Login: {
      path: "",
    },
    Welcome: "welcome",
    Demo: {
      screens: {
        Feed: "feed",
        Codex: "codex",
        DemoShowroom: {
          path: "showroom/:queryIndex?/:itemIndex?",
        },
        DemoDebug: "debug",
        DemoPodcastList: "podcast",
        DemoCommunity: "community",
      },
    },
  },
}

/**
 * This is the root component of our app.
 * @param {AppProps} props - The props for the `App` component.
 * @returns {JSX.Element} The rendered `App` component.
 */
export function App() {
  console.log("[App] App() render")
  const {
    initialNavigationState,
    onNavigationStateChange,
    isRestored: isNavigationStateRestored,
  } = useNavigationPersistence(storage, NAVIGATION_PERSISTENCE_KEY)

  const [areFontsLoaded, fontLoadError] = useFonts(customFontsToLoad)
  const [isI18nInitialized, setIsI18nInitialized] = useState(false)
  const [bootTimeout, setBootTimeout] = useState(false)

  useEffect(() => {
    console.log("[App] initI18n start")
    initI18n()
      .then(() => {
        console.log("[App] initI18n done")
        setIsI18nInitialized(true)
      })
      .then(() => loadDateFnsLocale())
      .catch((err) => {
        console.warn("[App] initI18n error", err)
        setIsI18nInitialized(true)
      })
  }, [])

  useEffect(() => {
    const t = setTimeout(() => {
      console.log("[App] boot timeout (3s) – forcing ready")
      setBootTimeout(true)
    }, 3000)
    return () => clearTimeout(t)
  }, [])

  const ready =
    bootTimeout ||
    (isNavigationStateRestored && isI18nInitialized && (areFontsLoaded || !!fontLoadError))

  console.log("[App] render", {
    isNavigationStateRestored,
    isI18nInitialized,
    areFontsLoaded,
    fontLoadError: !!fontLoadError,
    bootTimeout,
    ready,
    blocking: !ready
      ? [
          !isNavigationStateRestored && "nav",
          !isI18nInitialized && "i18n",
          !areFontsLoaded && !fontLoadError && "fonts",
        ].filter(Boolean)
      : null,
  })

  useEffect(() => {
    if (!ready) return
    SplashScreen.hideAsync().catch(() => {})
  }, [ready])

  if (!ready) {
    return null
  }

  console.log("[App] past splash – rendering app tree")

  const linking = {
    prefixes: [prefix],
    config,
  }

  const shouldBootConvex = Config.khalaSyncEnabled !== true
  const convexClient = shouldBootConvex
    ? new ConvexReactClient(
        (typeof Config.convexUrl === "string" && Config.convexUrl) ||
          (__DEV__
            ? "https://quaint-leopard-209.convex.cloud"
            : "https://aware-caterpillar-962.convex.cloud"),
        {
          unsavedChangesWarning: false,
          logger: false,
        },
      )
    : null

  return (
    <SafeAreaProvider initialMetrics={initialWindowMetrics}>
      <KeyboardProvider>
        <AuthProvider>
          {convexClient ? (
            <ConvexProviderWithAuth client={convexClient} useAuth={useConvexAuthFromContext}>
              <ThemeProvider>
                <AppNavigator
                  linking={linking}
                  initialState={initialNavigationState}
                  onStateChange={onNavigationStateChange}
                />
              </ThemeProvider>
            </ConvexProviderWithAuth>
          ) : (
            <ThemeProvider>
              <AppNavigator
                linking={linking}
                initialState={initialNavigationState}
                onStateChange={onNavigationStateChange}
              />
            </ThemeProvider>
          )}
        </AuthProvider>
      </KeyboardProvider>
    </SafeAreaProvider>
  )
}
