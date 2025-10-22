import { StyleSheet, View, Text } from 'react-native';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';

export default function HomeScreen() {
  return (
    <View style={styles.container}>
      <Text style={styles.title}>Home</Text>
      <Text style={styles.body}>Welcome to the home tab.</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: 24,
    gap: 8,
  },
  title: {
    fontSize: 24,
    fontFamily: Typography.bold,
    color: Colors.textPrimary,
  },
  body: {
    color: Colors.textSecondary,
  },
});
