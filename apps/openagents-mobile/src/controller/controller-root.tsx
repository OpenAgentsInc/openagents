import { NavigationContainer, type Theme } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { ActivityIndicator, View, useWindowDimensions, type ViewStyle } from "react-native";

import { Button } from "../ui/button";
import { Text } from "../ui/text";
import { colors, spacing } from "../ui/theme";
import { controllerLayout } from "./layout";
import { controllerLinking, type ControllerRouteParams } from "./routes";
import {
  ControllerConnectionsScreen,
  ControllerHomeScreen,
  ControllerNewTaskScreen,
  ControllerSarahVoiceScreen,
  ControllerSettingsScreen,
  ControllerSurfaceScreen,
  ControllerThreadScreen,
} from "./screens";
import { useControllerSession } from "./session-provider";
import { ControllerSignInScreen } from "./sign-in-screen";

const Stack = createNativeStackNavigator<ControllerRouteParams>();

const TerminalScreen = () => (
  <ControllerSurfaceScreen
    title="Terminal"
    description="The bounded JavaScript output view lands before a native terminal island. Live terminal control remains deliberately unavailable in this frame."
  />
);

const ReviewScreen = () => (
  <ControllerSurfaceScreen
    title="Review"
    description="Review will render bounded file changes and verification receipts from the same work projection."
  />
);

const FilesScreen = () => (
  <ControllerSurfaceScreen
    title="Files"
    description="Files will open bounded read projections; raw repositories never enter the controller state plane."
  />
);

const GitScreen = () => (
  <ControllerSurfaceScreen
    title="Git"
    description="Git status is observation-only here. Destructive operations require a fresh live disclosure and confirmation."
  />
);

const navigationTheme: Theme = {
  dark: true,
  colors: {
    primary: colors.accent,
    background: colors.background,
    card: colors.surfaceSunken,
    text: colors.text,
    border: colors.border,
    notification: colors.warn,
  },
  fonts: {
    regular: { fontFamily: "IBMPlexSans-Regular", fontWeight: "400" },
    medium: { fontFamily: "IBMPlexSans-SemiBold", fontWeight: "600" },
    bold: { fontFamily: "IBMPlexSans-SemiBold", fontWeight: "600" },
    heavy: { fontFamily: "Lilex-Bold", fontWeight: "700" },
  },
};

const ControllerNavigator = () => {
  const { width, height } = useWindowDimensions();
  const layout = controllerLayout(width, height);
  const presentation = layout.useSheets ? "formSheet" : "card";
  return (
    <NavigationContainer linking={controllerLinking} theme={navigationTheme}>
      <Stack.Navigator
        screenOptions={{
          headerStyle: { backgroundColor: colors.surfaceSunken },
          headerTintColor: colors.text,
          headerShadowVisible: false,
          contentStyle: { backgroundColor: colors.background },
        }}
      >
        <Stack.Screen
          name="Home"
          component={ControllerHomeScreen}
          options={{ headerShown: false }}
        />
        <Stack.Screen
          name="Thread"
          component={ControllerThreadScreen}
          options={({ route }) => ({ title: route.params.label })}
        />
        <Stack.Screen
          name="Terminal"
          component={TerminalScreen}
          options={{ title: "Terminal", presentation }}
        />
        <Stack.Screen
          name="Review"
          component={ReviewScreen}
          options={{ title: "Review", presentation }}
        />
        <Stack.Screen
          name="Files"
          component={FilesScreen}
          options={{ title: "Files", presentation }}
        />
        <Stack.Screen name="Git" component={GitScreen} options={{ title: "Git", presentation }} />
        <Stack.Screen
          name="Connections"
          component={ControllerConnectionsScreen}
          options={{ presentation }}
        />
        <Stack.Screen
          name="NewTask"
          component={ControllerNewTaskScreen}
          options={{ title: "New task", presentation }}
        />
        <Stack.Screen
          name="Settings"
          component={ControllerSettingsScreen}
          options={{ presentation }}
        />
        <Stack.Screen
          name="SarahVoice"
          component={ControllerSarahVoiceScreen}
          options={{ title: "Sarah", presentation: "fullScreenModal" }}
        />
      </Stack.Navigator>
    </NavigationContainer>
  );
};

export const ControllerRoot = () => {
  const session = useControllerSession();
  if (session.phase === "initializing") {
    return (
      <View style={$center}>
        <ActivityIndicator color={colors.accent} />
        <Text preset="caption">Opening your controller…</Text>
      </View>
    );
  }
  if (session.phase === "signed_out") {
    return <ControllerSignInScreen onCredential={session.acceptCredential} />;
  }
  if (session.phase === "failed") {
    return (
      <View style={$center} accessibilityRole="alert">
        <Text preset="heading">Controller unavailable</Text>
        <Text preset="body" color={colors.textDim}>
          {session.message}
        </Text>
        <Button label="Sign in again" onPress={() => void session.signOut()} />
      </View>
    );
  }
  return <ControllerNavigator />;
};

const $center: ViewStyle = {
  flex: 1,
  alignItems: "center",
  justifyContent: "center",
  gap: spacing.medium,
  padding: spacing.large,
  backgroundColor: colors.background,
};
