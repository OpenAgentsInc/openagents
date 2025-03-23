if (__DEV__) {
  require("./devtools/ReactotronConfig.ts")
}

import React from 'react'
import "@/utils/ignore-warnings"
import "@/utils/polyfills"
import { AppNavigator } from '@/navigators/AppNavigator';
import { useFonts } from 'expo-font';
import * as SplashScreen from 'expo-splash-screen';
import { StatusBar } from 'expo-status-bar';
import { KeyboardProvider } from 'react-native-keyboard-controller';
import { useInitialRootStore } from './models';
import { customFontsToLoad } from './theme';

SplashScreen.preventAutoHideAsync();

export function App() {
  console.log("[App] Starting...")
  const [loaded, error] = useFonts(customFontsToLoad);

  const { rehydrated } = useInitialRootStore(() => {
    console.log("[App] Root store initialized")
    setTimeout(SplashScreen.hideAsync, 500)
  })

  if (!rehydrated || !loaded && !error) {
    return null;
  }

  return (
    <>
      <StatusBar style='light' />
      <KeyboardProvider>
        <AppNavigator />
      </KeyboardProvider>
    </>
  )
}
