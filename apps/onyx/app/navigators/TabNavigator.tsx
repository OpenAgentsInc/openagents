import { createBottomTabNavigator } from "@react-navigation/bottom-tabs"
import { View, Pressable } from "react-native"
import { Ionicons } from "@expo/vector-icons"
import { MaterialCommunityIcons } from "@expo/vector-icons"
import { WalletNavigator } from "./WalletNavigator"
import { useAppTheme } from "@/utils/useAppTheme"

// Placeholder screen component
const PlaceholderScreen = () => <View style={{ flex: 1 }} />

export type TabParamList = {
  Chat: undefined
  Code: undefined
  Agents: undefined
  Wallet: undefined
  Profile: undefined
}

const Tab = createBottomTabNavigator<TabParamList>()

export const TabNavigator = () => {
  const {
    theme: { colors },
  } = useAppTheme()

  return (
    <Tab.Navigator
      screenOptions={({ route }) => ({
        headerShown: false,
        tabBarShowLabel: false,
        tabBarStyle: {
          backgroundColor: colors.background,
          borderTopColor: "#FFFFFF",
          borderTopWidth: 1,
          paddingTop: 5,
        },
        tabBarActiveTintColor: colors.tint,
        tabBarInactiveTintColor: colors.text,
        tabBarItemStyle: {
          paddingBottom: 12,
        },
        tabBarButton: (props) => {
          const { children, onPress, accessibilityState } = props
          const isActive = accessibilityState?.selected

          return (
            <View style={{ flex: 1, alignItems: 'center' }}>
              <Pressable
                onPress={onPress}
                style={{
                  paddingHorizontal: 12,
                  paddingVertical: 8,
                }}
                android_ripple={{ color: colors.text }}
                android_disableSound={true}
                pressRetentionOffset={{ bottom: 4, left: 4, right: 4, top: 4 }}
              >
                {({ pressed }) => (
                  <View style={{ opacity: pressed ? 0.8 : 1 }}>
                    {children}
                  </View>
                )}
              </Pressable>
              {isActive && (
                <View
                  style={{
                    position: 'absolute',
                    bottom: -15,
                    width: 4,
                    height: 4,
                    borderRadius: 2,
                    backgroundColor: '#FFFFFF',
                  }}
                />
              )}
            </View>
          )
        },
      })}
    >
      <Tab.Screen
        name="Chat"
        component={PlaceholderScreen}
        options={{
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="chatbubble-outline" size={size} color={color} />
          ),
        }}
      />
      <Tab.Screen
        name="Code"
        component={PlaceholderScreen}
        options={{
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="code-slash-outline" size={size} color={color} />
          ),
        }}
      />
      <Tab.Screen
        name="Agents"
        component={PlaceholderScreen}
        options={{
          tabBarIcon: ({ color, size }) => (
            <MaterialCommunityIcons name="robot-happy-outline" size={size} color={color} />
          ),
        }}
      />
      <Tab.Screen
        name="Wallet"
        component={WalletNavigator}
        options={{
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="wallet-outline" size={size} color={color} />
          ),
        }}
      />
      <Tab.Screen
        name="Profile"
        component={PlaceholderScreen}
        options={{
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="person-outline" size={size} color={color} />
          ),
        }}
      />
    </Tab.Navigator>
  )
}
