import { StyleSheet, View, Text } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';

export default function SettingsScreen() {
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Settings</Text>
      <Text style={styles.body}>Add options here.</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    padding: 16,
    gap: 8,
    backgroundColor: Colors.background,
  },
  title: {
    fontSize: 20,
    fontFamily: Typography.bold,
    color: Colors.textPrimary,
  },
  body: {
    color: Colors.textSecondary,
    fontFamily: Typography.primary,
  },
});

