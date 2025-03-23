import { createNativeStackNavigator } from "@react-navigation/native-stack"
import { WalletScreen } from "@/screens/WalletScreen/WalletScreen"
import { SendScreen } from "@/screens/WalletScreen/SendScreen"
import { ReceiveScreen } from "@/screens/WalletScreen/ReceiveScreen"
import { BackupWalletScreen } from "@/screens/WalletScreen/BackupWalletScreen"
import { RestoreWalletScreen } from "@/screens/WalletScreen/RestoreWalletScreen"
import { AgentEarningsScreen } from "@/screens/WalletScreen/AgentEarningsScreen"

export type WalletStackParamList = {
  WalletMain: undefined
  Send: undefined
  Receive: undefined
  BackupWallet: undefined
  RestoreWallet: undefined
  AgentEarnings: undefined
}

const Stack = createNativeStackNavigator<WalletStackParamList>()

export const WalletNavigator = () => {
  return (
    <Stack.Navigator
      screenOptions={{
        headerShown: false,
      }}
      initialRouteName="WalletMain"
    >
      <Stack.Screen name="WalletMain" component={WalletScreen} />
      <Stack.Screen name="Send" component={SendScreen} />
      <Stack.Screen name="Receive" component={ReceiveScreen} />
      <Stack.Screen name="BackupWallet" component={BackupWalletScreen} />
      <Stack.Screen name="RestoreWallet" component={RestoreWalletScreen} />
      <Stack.Screen name="AgentEarnings" component={AgentEarningsScreen} />
    </Stack.Navigator>
  )
}