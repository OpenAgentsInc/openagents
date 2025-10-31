import React, { Component, ErrorInfo, ReactNode } from 'react'
import { View, Text, StyleSheet, ScrollView, Pressable } from 'react-native'
import * as Clipboard from 'expo-clipboard'
import * as Updates from 'expo-updates'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

type CatchMode = 'always' | 'dev' | 'prod' | 'never'

export class ErrorBoundary extends Component<{ children: ReactNode; catchErrors?: CatchMode }, { error: Error | null; info: ErrorInfo | null }> {
  state = { error: null as Error | null, info: null as ErrorInfo | null }

  componentDidCatch(error: Error, info: ErrorInfo) {
    if (!this.isEnabled()) return
    try { console.error('[ErrorBoundary] JS error:', error, info?.componentStack) } catch {}
    this.setState({ error, info })
  }

  isEnabled(): boolean {
    const mode: CatchMode = this.props.catchErrors ?? 'always'
    return (
      mode === 'always' ||
      (mode === 'dev' && __DEV__) ||
      (mode === 'prod' && !__DEV__)
    )
  }

  handleCopy = async () => {
    const text = `${this.state.error?.stack || this.state.error?.message || this.state.error}`
    try { await Clipboard.setStringAsync(text) } catch {}
  }

  handleRestart = async () => {
    try { await Updates.reloadAsync() } catch {}
  }

  render() {
    if (!this.isEnabled() || !this.state.error) return this.props.children
    const message = String(this.state.error?.message || this.state.error)
    const stack = String(this.state.error?.stack || this.state.info?.componentStack || '')
    return (
      <View style={styles.container}>
        <Text style={styles.title}>Something went wrong</Text>
        <Text style={styles.subtitle}>A runtime error occurred. Details below.</Text>
        <ScrollView style={styles.box} contentContainerStyle={{ padding: 12 }}>
          {!!message && <Text style={styles.errorText}>{message}</Text>}
          {!!stack && <Text selectable style={styles.stackText}>{stack}</Text>}
        </ScrollView>
        <View style={{ flexDirection: 'row', gap: 12 }}>
          <Pressable onPress={this.handleCopy} style={styles.btn}><Text style={styles.btnText}>Copy details</Text></Pressable>
          <Pressable onPress={() => this.setState({ error: null, info: null })} style={styles.btnAlt}><Text style={styles.btnAltText}>Dismiss</Text></Pressable>
          <Pressable onPress={this.handleRestart} style={styles.btnAlt}><Text style={styles.btnAltText}>Restart</Text></Pressable>
        </View>
      </View>
    )
  }
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: Colors.background, padding: 24, alignItems: 'center' },
  title: { color: Colors.danger, fontFamily: Typography.bold, fontSize: 18 },
  subtitle: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 6, marginBottom: 12 },
  box: { alignSelf: 'stretch', backgroundColor: Colors.card, borderWidth: 1, borderColor: Colors.border, maxHeight: '70%' },
  errorText: { color: Colors.danger, fontFamily: Typography.bold, marginBottom: 8 },
  stackText: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11 },
  btn: { backgroundColor: Colors.quaternary, paddingHorizontal: 14, paddingVertical: 10 },
  btnText: { color: Colors.foreground, fontFamily: Typography.bold },
  btnAlt: { borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 14, paddingVertical: 10 },
  btnAltText: { color: Colors.secondary, fontFamily: Typography.bold },
})
