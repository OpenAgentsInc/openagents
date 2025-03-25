import { observer } from "mobx-react-lite"
import { FC, useState } from "react"
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
import { useEarnings } from "@/models/earnings/EarningsContext"
import { useNavigation } from "@react-navigation/native"
import { CompositeNavigationProp } from "@react-navigation/native"
import { BottomTabNavigationProp } from "@react-navigation/bottom-tabs"
import { NativeStackNavigationProp } from "@react-navigation/native-stack"
import { WalletStackParamList } from "@/navigators/WalletNavigator"
import { TabParamList } from "@/navigators/TabNavigator"

type NavigationProp = CompositeNavigationProp<
  BottomTabNavigationProp<TabParamList>,
  NativeStackNavigationProp<WalletStackParamList>
>

export const AgentEarningsScreen: FC = observer(function AgentEarningsScreen() {
  const { earnings, getTotalForPeriod, getEarningsForPeriod } = useEarnings();
  const [activePeriod, setActivePeriod] = useState<'week' | 'month' | 'year' | 'all'>('week');
  const navigation = useNavigation<NavigationProp>();

  // Get filtered earnings and total for the selected period
  const filteredEarnings = getEarningsForPeriod(activePeriod);
  const periodTotal = getTotalForPeriod(activePeriod);

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
          <Text text={`${activePeriod.charAt(0).toUpperCase() + activePeriod.slice(1)} Earnings`} style={$totalLabel} />
          <View style={$totalMoneyContainer}>
            <Money sats={periodTotal} symbol={true} size="display" />
          </View>
          <View style={$periodSelector}>
            <Button
              label="Week"
              variant="primary"
              size="small"
              style={[
                $periodButton,
                activePeriod === 'week' ? $periodButtonActive : null
              ]}
              onPress={() => setActivePeriod('week')}
            />
            <Button
              label="Month"
              variant="primary"
              size="small"
              style={[
                $periodButton,
                activePeriod === 'month' ? $periodButtonActive : null
              ]}
              onPress={() => setActivePeriod('month')}
            />
            <Button
              label="Year"
              variant="primary"
              size="small"
              style={[
                $periodButton,
                activePeriod === 'year' ? $periodButtonActive : null
              ]}
              onPress={() => setActivePeriod('year')}
            />
            <Button
              label="All"
              variant="primary"
              size="small"
              style={[
                $periodButton,
                activePeriod === 'all' ? $periodButtonActive : null
              ]}
              onPress={() => setActivePeriod('all')}
            />
          </View>
        </Card>

        <Text text="Earnings Breakdown" style={$sectionHeader} />
        <ScrollView style={$categoriesList}>
          {/* Group earnings by category and calculate totals for current period */}
          {[
            { category: 'compute', icon: "computer", name: "MCP Server Usage" },
            { category: 'plugin', icon: "extension", name: "Agent Plugin" },
            { category: 'referral', icon: "people", name: "Referral Rewards" },
            { category: 'content', icon: "edit", name: "Content Creation" }
          ].map((item) => {
            // Calculate total for this category for the selected time period
            const categoryTotal = filteredEarnings
              .filter(earning => earning.category === item.category)
              .reduce((sum, earning) => sum + earning.amount, 0);

            // Only show categories that have earnings in the selected period
            if (categoryTotal === 0) return null;

            return (
              <Card key={item.category} padding="medium" style={$categoryCard}>
                <View style={$categoryHeader}>
                  <View style={$categoryLeft}>
                    <Icon icon={item.icon as any} color="white" size={24} style={$icon} />
                    <View style={$categoryInfo}>
                      <Text text={item.name} style={$categoryName} />
                    </View>
                  </View>
                  <View style={$moneyContainer}>
                    <MoneySmall sats={categoryTotal} symbol={true} size="bodyMSB" />
                  </View>
                </View>
              </Card>
            );
          })
            .filter(Boolean) /* Remove null items */
          }

          {/* Show message if no earnings in selected period */}
          {filteredEarnings.length === 0 && (
            <Card padding="medium" style={$categoryCard}>
              <View style={$categoryHeader}>
                <View style={$categoryLeft}>
                  <Ionicons name="information-circle-outline" color="white" size={24} style={$icon} />
                  <View style={$categoryInfo}>
                    <Text text="No earnings found" style={$categoryName} />
                  </View>
                </View>
              </View>
            </Card>
          )}
        </ScrollView>

        <View style={$actionsContainer}>
          <Button
            label="Go to Wallet"
            variant="primary"
            size="medium"
            style={$actionButton}
            leftIcon="wallet-outline"
            renderIcon={renderIcon}
            onPress={() => navigation.navigate("Wallet")}
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
  marginBottom: 24,
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
  marginTop: 16,
}

const $actionButton: ViewStyle = {
  width: "100%",
}

const $moneyContainer: ViewStyle = {
  marginTop: -4,
}
