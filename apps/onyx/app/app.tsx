if (__DEV__) {
  require("./devtools/ReactotronConfig.ts")
}

import React from 'react'
import { AppNavigator } from '@/navigators/AppNavigator';
import { useFonts } from 'expo-font';
import * as SplashScreen from 'expo-splash-screen';
import { StatusBar } from 'expo-status-bar';
import { useEffect } from 'react';
import { KeyboardProvider } from 'react-native-keyboard-controller';

SplashScreen.preventAutoHideAsync();

export function App() {
  const [loaded, error] = useFonts({
    'Berkeley Mono': require('../assets/fonts/BerkeleyMonoVariable-Regular.ttf'),
  });

  useEffect(() => {
    if (loaded || error) {
      SplashScreen.hideAsync();
    }
  }, [loaded, error]);

  if (!loaded && !error) {
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
