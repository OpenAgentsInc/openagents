import { NavigationContainer } from "@react-navigation/native"
import { createNativeStackNavigator } from "@react-navigation/native-stack"

import DecisionsScreen from "./app/decisions"
import NodesScreen from "./app/nodes"
import SessionDetailScreen from "./app/session-detail"
import SessionsScreen from "./app/sessions"
import SettingsScreen from "./app/settings"
import SpawnScreen from "./app/spawn"

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
    <NavigationContainer>
      <Stack.Navigator initialRouteName="Nodes">
        <Stack.Screen name="Nodes" component={NodesScreen} />
        <Stack.Screen name="Sessions" component={SessionsScreen} />
        <Stack.Screen name="SessionDetail" component={SessionDetailScreen} options={{ title: "Session Detail" }} />
        <Stack.Screen name="Decisions" component={DecisionsScreen} />
        <Stack.Screen name="Spawn" component={SpawnScreen} />
        <Stack.Screen name="Settings" component={SettingsScreen} />
      </Stack.Navigator>
    </NavigationContainer>
  )
}
