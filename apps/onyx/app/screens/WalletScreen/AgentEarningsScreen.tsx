import { observer } from "mobx-react-lite"
import { FC } from "react"
import { ScrollView, StyleSheet, TextStyle, TouchableOpacity, View, ViewStyle } from "react-native"
import { Button, Icon, Screen, Text } from "@/components"
// Import our shared UI Button
import { Button as SharedButton } from "@openagents/ui"
import { useHeader } from "@/hooks/useHeader"
import { goBack } from "@/navigators/navigationUtilities"
import { typography } from "@/theme"
import { colors } from "@/theme/colorsDark"
import Money from "./Money"
import MoneySmall from "./MoneySmall"

interface EarningsCategory {
  id: string
  name: string
  amount: number
  percentage: number
  icon: string
  description: string
}

const mockEarnings: EarningsCategory[] = [
  {
    id: "mcp",
    name: "MCP Server Usage",
    amount: 2000,
    percentage: 40,
    icon: "computer",
    description: "Earnings from providing compute resources",
  },
  {
    id: "plugin",
    name: "Agent Plugin",
    amount: 1500,
    percentage: 30,
    icon: "extension",
    description: "Earnings from your agent plugins",
  },
  {
    id: "referral",
    name: "Referral Rewards",
    amount: 1000,
    percentage: 20,
    icon: "people",
    description: "Earnings from referred users",
  },
  {
    id: "content",
    name: "Content Creation",
    amount: 500,
    percentage: 10,
    icon: "edit",
    description: "Earnings from content contributions",
  },
]

export const AgentEarningsScreen: FC = observer(function AgentEarningsScreen() {
  useHeader({
    title: "Agent Earnings",
    leftIcon: "back",
    onLeftPress: goBack,
  })

  const totalEarnings = mockEarnings.reduce((sum, category) => sum + category.amount, 0)

  return (
    <Screen style={$root} preset="scroll">
      <View style={$container}>
        <View style={$totalSection}>
          <Text text="Total Earnings" style={$totalLabel} />
          <Money sats={totalEarnings} symbol={true} size="display" style={$totalMoneyStyle} />
          <View style={$periodSelector}>
            <Button
              text="Week"
              style={[$periodButton, $periodButtonActive]}
              textStyle={$periodButtonText}
            />
            <Button text="Month" style={$periodButton} textStyle={$periodButtonText} />
            <Button text="Year" style={$periodButton} textStyle={$periodButtonText} />
            <Button text="All" style={$periodButton} textStyle={$periodButtonText} />
          </View>
        </View>

        <Text text="Earnings Breakdown" style={$sectionHeader} />
        <ScrollView style={$categoriesList}>
          {mockEarnings.map((category) => (
            <TouchableOpacity key={category.id} style={$categoryItem}>
              <View style={$categoryHeader}>
                <View style={$categoryLeft}>
                  <Icon icon={category.icon as any} color="white" size={24} />
                  <View style={$categoryInfo}>
                    <Text text={category.name} style={$categoryName} />
                    <Text text={category.description} style={$categoryDescription} />
                  </View>
                </View>
                <View style={$categoryAmountContainer}>
                  <MoneySmall
                    sats={category.amount}
                    symbol={true}
                    style={$moneyStyle}
                  />
                </View>
              </View>
              <View style={$progressContainer}>
                <View style={[$progressBar, { width: `${category.percentage}%` }]} />
              </View>
            </TouchableOpacity>
          ))}
        </ScrollView>

        <View style={$actionsContainer}>
          <Button
            text="View Details"
            style={$actionButton}
            LeftAccessory={() => <Icon icon="analytics" color="white" size={20} />}
          />
          <Button
            text="Withdraw"
            style={$actionButton}
            LeftAccessory={() => <Icon icon="download" color="white" size={20} />}
          />
        </View>
        
        {/* Example of shared UI components */}
        <View style={$sharedUIContainer}>
          <Text text="Shared UI Components" style={$sectionHeader} />
          
          <View style={$sharedButtonsRow}>
            <SharedButton 
              label="Primary"
              variant="primary"
              size="small"
              onPress={() => {}}
              style={$sharedButton}
            />
            
            <SharedButton 
              label="Secondary"
              variant="secondary"
              size="small"
              onPress={() => {}}
              style={$sharedButton}
            />
          </View>
          
          <SharedButton 
            label="Tertiary Button (Large)"
            variant="tertiary"
            size="large"
            onPress={() => {}}
            style={$sharedFullButton}
          />
          
          <SharedButton 
            label="Loading State"
            loading={true}
            onPress={() => {}}
            style={$sharedFullButton}
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

const $totalSection: ViewStyle = {
  alignItems: "center",
  marginBottom: 32,
}

const $totalLabel: TextStyle = {
  color: colors.palette.accent100,
  fontSize: 16,
  marginBottom: 8,
  fontFamily: typography.primary.medium,
}

const $periodSelector: ViewStyle = {
  flexDirection: "row",
  marginTop: 16,
  gap: 8,
}

const $periodButton: ViewStyle = {
  paddingHorizontal: 16,
  paddingVertical: 8,
  backgroundColor: colors.palette.neutral100,
  borderRadius: 20,
  minWidth: 70,
  borderWidth: 1,
  borderColor: colors.palette.neutral300,
}

const $periodButtonActive: ViewStyle = {
  backgroundColor: colors.palette.neutral200,
  borderColor: colors.palette.neutral400,
}

const $periodButtonText: TextStyle = {
  fontSize: 14,
  fontFamily: typography.primary.medium,
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

const $categoryItem: ViewStyle = {
  backgroundColor: colors.palette.neutral100,
  borderRadius: 12,
  padding: 16,
  marginBottom: 12,
  borderWidth: 1,
  borderColor: "white",
}

const $categoryHeader: ViewStyle = {
  flexDirection: "row",
  justifyContent: "space-between",
  alignItems: "center",
  marginBottom: 12,
}

const $categoryLeft: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
  flex: 1,
}

const $categoryInfo: ViewStyle = {
  marginLeft: 12,
  flex: 1,
}

const $categoryName: TextStyle = {
  color: "white",
  fontSize: 16,
  fontFamily: typography.primary.medium,
  marginBottom: 4,
}

const $categoryDescription: TextStyle = {
  color: colors.palette.neutral600,
  fontSize: 12,
  fontFamily: typography.primary.normal,
}

const $categoryAmountContainer: ViewStyle = {
  flexDirection: "row",
  alignItems: "center",
}

const $moneyStyle: ViewStyle = {
  marginTop: 0,
}

const $progressContainer: ViewStyle = {
  height: 4,
  backgroundColor: colors.palette.neutral200,
  borderRadius: 2,
  overflow: "hidden",
}

const $progressBar: ViewStyle = {
  height: "100%",
  backgroundColor: colors.palette.accent200,
}

const $actionsContainer: ViewStyle = {
  flexDirection: "row",
  justifyContent: "space-between",
  gap: 16,
  marginTop: 24,
}

const $actionButton: ViewStyle = {
  flex: 1,
}

const $totalMoneyStyle: ViewStyle = {
  marginTop: 0,
}

// Shared UI component styles
const $sharedUIContainer: ViewStyle = {
  marginTop: 32,
  paddingTop: 24,
  borderTopWidth: 1,
  borderTopColor: colors.palette.neutral200,
}

const $sharedButtonsRow: ViewStyle = {
  flexDirection: 'row',
  gap: 16,
  marginBottom: 16,
}

const $sharedButton: ViewStyle = {
  flex: 1,
}

const $sharedFullButton: ViewStyle = {
  marginBottom: 16,
}
