import { useEffect } from "react"
import View from "react-native/Libraries/Components/View/View"
import * as LogBoxData from "react-native/Libraries/LogBox/Data/LogBoxData"
import LogBoxButton from "react-native/Libraries/LogBox/UI/LogBoxButton"
import LogBoxNotificationCountBadge from "react-native/Libraries/LogBox/UI/LogBoxNotificationCountBadge"
import LogBoxNotificationDismissButton from "react-native/Libraries/LogBox/UI/LogBoxNotificationDismissButton"
import LogBoxNotificationMessage from "react-native/Libraries/LogBox/UI/LogBoxNotificationMessage"
import StyleSheet from "react-native/Libraries/StyleSheet/StyleSheet"

export default function LogBoxNotification(props) {
  const { totalLogCount, level, log } = props

  useEffect(() => {
    LogBoxData.symbolicateLogLazy(log)
  }, [log])

  return (
    <View id="logbox_notification" style={styles.container}>
      <LogBoxButton
        id={`logbox_open_button_${level}`}
        backgroundColor={{
          default: "rgba(15, 23, 34, 1)",
          pressed: "rgba(8, 13, 20, 1)",
        }}
        onFocusChange={props.onFocusChange}
        onPress={props.onPressOpen}
        style={styles.press}
      >
        <View style={styles.content}>
          <LogBoxNotificationCountBadge count={totalLogCount} level={level} />
          <LogBoxNotificationMessage message={log.message} />
          <LogBoxNotificationDismissButton
            id={`logbox_dismiss_button_${level}`}
            onPress={props.onPressDismiss}
          />
        </View>
      </LogBoxButton>
    </View>
  )
}

const styles = StyleSheet.create({
  container: {
    backgroundColor: "rgba(15, 23, 34, 1)",
    height: 48,
    justifyContent: "center",
    marginTop: 0.5,
    position: "relative",
    width: "100%",
  },
  press: {
    height: 48,
    justifyContent: "center",
    marginTop: 0.5,
    paddingHorizontal: 12,
    position: "relative",
    width: "100%",
  },
  content: {
    alignItems: "flex-start",
    borderRadius: 8,
    flexBasis: "auto",
    flexDirection: "row",
    flexGrow: 0,
    flexShrink: 0,
  },
})
