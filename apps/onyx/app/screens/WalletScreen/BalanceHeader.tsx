import { memo, ReactElement, useEffect, useMemo } from "react"
import { ActivityIndicator, StyleSheet, View } from "react-native"
import { Text } from "@/components"
import { useStores } from "@/models"
import Money from "./Money"

/**
 * Displays the total available balance for the current wallet & network.
 */
const BalanceHeader = (): ReactElement => {
  const { walletStore } = useStores()
  const { balanceSat, pendingSendSat, pendingReceiveSat, isInitialized, error, fetchBalanceInfo } =
    walletStore

  const totalBalance = useMemo(
    () => balanceSat + pendingSendSat + pendingReceiveSat,
    [balanceSat, pendingSendSat, pendingReceiveSat],
  )

  console.log("total balance: ", totalBalance)

  // Fetch balance on mount and every 15 seconds
  useEffect(() => {
    if (isInitialized && !error) {
      console.log("[BalanceHeader] Initial balance fetch")
      fetchBalanceInfo()

      // Set up periodic refresh
      const interval = setInterval(() => {
        console.log("[BalanceHeader] Periodic balance fetch")
        fetchBalanceInfo()
      }, 5000) //

      return () => clearInterval(interval)
    }
  }, [isInitialized, error, fetchBalanceInfo])

  if (error) {
    return (
      <View style={styles.pendingContainer}>
        <Text text={error} />
      </View>
    )
  }

  if (!isInitialized) {
    return (
      <View style={[styles.loading]}>
        <ActivityIndicator size="large" color="#ffffff" />
      </View>
    )
  }

  return (
    <View style={styles.container}>
      <View style={styles.balance}>
        <Money sats={totalBalance} symbol={true} />

        {/* {(pendingSendSat > 0 || pendingReceiveSat > 0) && (
          <View style={styles.pendingContainer}>
            {pendingSendSat > 0 && (
              <Text style={styles.pendingText}>
                Sending: {pendingSendSat.toLocaleString()} sats
              </Text>
            )}
            {pendingReceiveSat > 0 && (
              <Text style={styles.pendingText}>
                Receiving: {pendingReceiveSat.toLocaleString()} sats
              </Text>
            )}
          </View>
        )} */}
      </View>
    </View>
  )
}

const styles = StyleSheet.create({
  container: {
    width: "100%",
    marginVertical: 48,
  },
  balance: {
    alignItems: "center",
    justifyContent: "center",
  },
  loading: {
    alignItems: "center",
    justifyContent: "center",
  },
  pendingContainer: {
    marginTop: 12,
    alignItems: "center",
  },
  pendingText: {
    color: "#888",
    fontSize: 14,
    fontFamily: "JetBrainsMono-Regular",
    marginVertical: 2,
  },
})

export default memo(BalanceHeader)
