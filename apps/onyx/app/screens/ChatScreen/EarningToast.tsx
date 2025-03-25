import React from 'react';
import { View, Text, StyleSheet } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { typography } from '@/theme';

interface EarningToastProps {
  amount: number;
  exchangeRate: number;
}

export const EarningToast = ({ amount, exchangeRate }: EarningToastProps) => {
  // Calculate USD value (1 BTC = exchangeRate USD, with amount in satoshis)
  const usdValue = (amount / 100000000) * exchangeRate;
  
  return (
    <View style={styles.container}>
      <Ionicons name="wallet-outline" size={20} color="white" style={styles.icon} />
      <Text style={styles.text}>
        Earned {amount.toLocaleString()} ₿ (${usdValue.toFixed(2)})
      </Text>
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  icon: {
    marginRight: 8,
  },
  text: {
    color: 'white',
    fontFamily: typography.primary.medium,
    fontSize: 14,
  },
});
