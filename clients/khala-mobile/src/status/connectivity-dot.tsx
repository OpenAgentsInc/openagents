import { View } from "react-native"

import { useKhalaCodeConnectivity } from "./use-khala-code-connectivity"

/** Tiny header/title-bar connectivity indicator — green when a Khala Code
 * desktop instance is reachable, red otherwise, gray while checking. */
export const ConnectivityDot = () => {
  const status = useKhalaCodeConnectivity()
  const reachable = status?.reachable === true
  const checking = status === null
  const color = checking ? "#7e8a98" : reachable ? "#22c55e" : "#ef4444"

  return (
    <View
      accessibilityLabel={
        checking
          ? "Khala Code connectivity: checking"
          : reachable
            ? "Khala Code connectivity: connected"
            : "Khala Code connectivity: disconnected"
      }
      style={{
        backgroundColor: color,
        borderRadius: 5,
        height: 10,
        width: 10
      }}
    />
  )
}
