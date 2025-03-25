// ₿
import { observer } from "mobx-react-lite"
import { FC } from "react"
import { View, ViewStyle } from "react-native"
import { Icon, Screen } from "@/components"
import { Button } from "@openagents/ui"
import { useHeader } from "@/hooks/useHeader"
import { goBack } from "@/navigators/navigationUtilities"
import { WalletStackParamList } from "@/navigators/WalletNavigator"
import { useNavigation } from "@react-navigation/native"
import { NativeStackNavigationProp } from "@react-navigation/native-stack"
import BalanceHeader from "./BalanceHeader"
import { TransactionsList } from "./TransactionsList"
import { useEarnings } from "@/models/earnings/EarningsContext"
import { Ionicons } from "@expo/vector-icons"

type WalletScreenNavigationProp = NativeStackNavigationProp<WalletStackParamList, "WalletMain">

export const WalletScreen: FC = observer(function WalletScreen() {
  const navigation = useNavigation<WalletScreenNavigationProp>()
  const { totalEarnings, withdrawEarnings } = useEarnings()
  
  useHeader({
    title: "Wallet",
    leftIcon: "back",
    onLeftPress: goBack,
    rightIcon: "key",
    onRightPress: () => {
      navigation.navigate("BackupWallet")
    },
  })

  const renderIcon = (iconName: string) => {
    if (iconName === "wallet-outline") {
      return <Ionicons name="wallet-outline" size={20} color="white" />
    }
    return <Icon icon={iconName as any} color="white" size={20} />
  }

  return (
    <Screen style={$root} contentContainerStyle={$contentContainer} preset="fixed">
      <View style={$topSection}>
        <BalanceHeader />
        <View style={$buttonsContainer}>
          <View style={$buttonRow}>
            <Button
              label="Send"
              onPress={() => {
                navigation.navigate("Send")
              }}
              style={$actionButton}
              variant="primary"
              size="medium"
              leftIcon="arrow-upward"
              renderIcon={renderIcon}
            />
            <Button
              label="Receive"
              onPress={() => {
                navigation.navigate("Receive")
              }}
              style={$actionButton}
              variant="primary"
              size="medium"
              leftIcon="arrow-downward"
              renderIcon={renderIcon}
            />
          </View>
          
          {/* Only show the Withdraw button if there are earnings */}
          {totalEarnings > 0 && (
            <Button
              label={`Withdraw ₿${totalEarnings.toLocaleString()} to Wallet`}
              onPress={() => {
                // Withdraw the earnings
                const amount = withdrawEarnings();
                
                // In a real app, this would add the amount to the wallet balance
                // and create a transaction record
                
                // Show success toast or message (you could use your Toast component here)
                alert(`Successfully withdrawn ₿${amount.toLocaleString()} to your wallet!`);
              }}
              style={$withdrawButton}
              variant="primary"
              size="medium"
              leftIcon="wallet-outline"
              renderIcon={renderIcon}
            />
          )}
        </View>
      </View>

      <TransactionsList />
    </Screen>
  )
})

const $root: ViewStyle = {
  flex: 1,
}

const $contentContainer: ViewStyle = {
  flex: 1,
  alignItems: "center",
}

const $topSection: ViewStyle = {
  width: "100%",
}

const $buttonRow: ViewStyle = {
  flexDirection: "row",
  justifyContent: "center",
  gap: 20,
}

const $actionButton: ViewStyle = {
  flex: 1,
  minWidth: 130,
}

const $earningsButton: ViewStyle = {
  flex: 1,
  minWidth: 280,
}

const $iconContainer: ViewStyle = {
  marginRight: 8,
}

const $buttonsContainer: ViewStyle = {
  width: '100%',
  paddingHorizontal: 20,
  marginBottom: 12,
  gap: 16,
}

const $withdrawButton: ViewStyle = {
  width: '100%',
}
