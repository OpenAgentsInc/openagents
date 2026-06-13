import "react-native-gesture-handler"
import { NavigationContainer } from "@react-navigation/native"
import { createNativeStackNavigator } from "@react-navigation/native-stack"
import { StatusBar } from "react-native"
import { GestureHandlerRootView } from "react-native-gesture-handler"

import DecisionsScreen from "./app/decisions"
import NodesScreen from "./app/nodes"
import SessionDetailScreen from "./app/session-detail"
import SessionsScreen from "./app/sessions"
import SettingsScreen from "./app/settings"
import SpawnScreen from "./app/spawn"
import { UpdateGate } from "./src/update-gate"

export type RootStackParamList = {
  Nodes: undefined
  Sessions: undefined
  SessionDetail: undefined
  Decisions: undefined
  Spawn: undefined
  Settings: undefined
}

const Stack = createNativeStackNavigator<RootStackParamList>()

export default function App() {
  return (
    <GestureHandlerRootView style={{ flex: 1, backgroundColor: "#000000" }}>
      <StatusBar barStyle="light-content" backgroundColor="#000000" />
      <NavigationContainer>
        <Stack.Navigator initialRouteName="Nodes" screenOptions={{ headerShown: false }}>
          <Stack.Screen name="Nodes" component={NodesScreen} />
          <Stack.Screen name="Sessions" component={SessionsScreen} />
          <Stack.Screen name="SessionDetail" component={SessionDetailScreen} />
          <Stack.Screen name="Decisions" component={DecisionsScreen} />
          <Stack.Screen name="Spawn" component={SpawnScreen} />
          <Stack.Screen name="Settings" component={SettingsScreen} />
        </Stack.Navigator>
      </NavigationContainer>
      <UpdateGate />
    </GestureHandlerRootView>
  )
}
