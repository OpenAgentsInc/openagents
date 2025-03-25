import { observer } from "mobx-react-lite"
import { FC } from "react"
import { ScrollView, StyleSheet, TextStyle, View, ViewStyle, ImageStyle } from "react-native"
import { Icon, Screen, Text } from "@/components"
import { Button, Card } from "@openagents/ui"
import { useHeader } from "@/hooks/useHeader"
import { goBack } from "@/navigators/navigationUtilities"
import { typography } from "@/theme"
import { colors } from "@/theme/colorsDark"
import Money from "./Money"
import MoneySmall from "./MoneySmall"
import { Ionicons } from "@expo/vector-icons"

export const AgentEarningsScreen: FC = observer(function AgentEarningsScreen() {
  useHeader({
    title: "Agent Earnings",
    leftIcon: "back",
    onLeftPress: goBack,
  })

  const renderIcon = (iconName: string) => {
    if (iconName === "wallet-outline") {
      return <Ionicons name="wallet-outline" size={20} color="white" />
    }
    return <Icon icon={iconName as any} color="white" size={20} />
  }

  return (
    <Screen style={$root} preset="scroll">
      <View style={$container}>
        <Card padding="large" style={$totalCard}>
          <Text text="Total Earnings" style={$totalLabel} />
          <View style={$totalMoneyContainer}>
            <Money sats={5000} symbol={true} size="display" />
          </View>
          <View style={$periodSelector}>
            <Button
              label="Week"
              variant="primary"
              size="small"
              style={[$periodButton, $periodButtonActive]}
            />
            <Button label="Month" variant="primary" size="small" style={$periodButton} />
            <Button label="Year" variant="primary" size="small" style={$periodButton} />
            <Button label="All" variant="primary" size="small" style={$periodButton} />
          </View>
        </Card>

        <Text text="Earnings Breakdown" style={$sectionHeader} />
        <ScrollView style={$categoriesList}>
          <Card padding="medium" style={$categoryCard}>
            <View style={$categoryHeader}>
              <View style={$categoryLeft}>
                <Icon icon="computer" color="white" size={24} style={$icon} />
                <View style={$categoryInfo}>
                  <Text text="MCP Server Usage" style={$categoryName} />
                  <Text text="Earnings from providing compute resources" style={$categoryDescription} />
                </View>
              </View>
              <MoneySmall sats={2000} symbol={true} size="bodyMSB" />
            </View>
          </Card>

          <Card padding="medium" style={$categoryCard}>
            <View style={$categoryHeader}>
              <View style={$categoryLeft}>
                <Icon icon="extension" color="white" size={24} style={$icon} />
                <View style={$categoryInfo}>
                  <Text text="Agent Plugin" style={$categoryName} />
                  <Text text="Earnings from your agent plugins" style={$categoryDescription} />
                </View>
              </View>
              <MoneySmall sats={1500} symbol={true} size="bodyMSB" />
            </View>
          </Card>

          <Card padding="medium" style={$categoryCard}>
            <View style={$categoryHeader}>
              <View style={$categoryLeft}>
                <Icon icon="people" color="white" size={24} style={$icon} />
                <View style={$categoryInfo}>
                  <Text text="Referral Rewards" style={$categoryName} />
                  <Text text="Earnings from referred users" style={$categoryDescription} />
                </View>
              </View>
              <MoneySmall sats={1000} symbol={true} size="bodyMSB" />
            </View>
          </Card>

          <Card padding="medium" style={$categoryCard}>
            <View style={$categoryHeader}>
              <View style={$categoryLeft}>
                <Icon icon="edit" color="white" size={24} style={$icon} />
                <View style={$categoryInfo}>
                  <Text text="Content Creation" style={$categoryName} />
                  <Text text="Earnings from content contributions" style={$categoryDescription} />
                </View>
              </View>
              <MoneySmall sats={500} symbol={true} size="bodyMSB" />
            </View>
          </Card>
        </ScrollView>

        <View style={$actionsContainer}>
          <Button
            label="See All"
            variant="primary"
            size="medium"
            style={$actionButton}
            leftIcon="list"
            renderIcon={renderIcon}
          />
          <Button
            label="Withdraw"
            variant="primary"
            size="medium"
            style={$actionButton}
            leftIcon="wallet-outline"
            renderIcon={renderIcon}
            onPress={() => goBack()}
          />
        </View>
      </View>
    </Screen>
  )
})

const $root: ViewStyle = {
  flex: 1,
}

const $container: ViewStyle = {
  flex: 1,
  padding: 16,
}

const $totalCard: ViewStyle = {
  marginBottom: 32,
}

const $totalLabel: TextStyle = {
  color: "white",
  fontSize: 16,
  marginBottom: 8,
  fontFamily: typography.primary.medium,
  textAlign: "center",
}

const $totalMoneyContainer: ViewStyle = {
  alignItems: "center",
  justifyContent: "center",
}

const $periodSelector: ViewStyle = {
  flexDirection: "row",
  marginTop: 16,
  gap: 8,
}

const $periodButton: ViewStyle = {
  minWidth: 70,
}

const $periodButtonActive: ViewStyle = {
  backgroundColor: colors.palette.neutral200,
}

const $sectionHeader: TextStyle = {
  color: colors.palette.accent100,
  fontSize: 16,
  marginBottom: 16,
  fontFamily: typography.primary.medium,
}

const $categoriesList: ViewStyle = {
  flex: 1,
}

const $categoryCard: ViewStyle = {
  marginBottom: 12,
}

const $categoryHeader: ViewStyle = {
  flexDirection: "row",
  justifyContent: "space-between",
  alignItems: "flex-start",
}

const $categoryLeft: ViewStyle = {
  flexDirection: "row",
  alignItems: "flex-start",
  flex: 1,
}

const $categoryInfo: ViewStyle = {
  marginLeft: 12,
  flex: 1,
}

const $icon: ImageStyle = {
  marginTop: 2,
}

const $categoryName: TextStyle = {
  fontSize: 16,
  fontFamily: typography.primary.medium,
  color: "white",
}

const $categoryDescription: TextStyle = {
  fontSize: 14,
  color: "white",
  marginTop: 4,
}

const $actionsContainer: ViewStyle = {
  flexDirection: "row",
  gap: 12,
  marginTop: 24,
}

const $actionButton: ViewStyle = {
  flex: 1,
}
