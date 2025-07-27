import { ReactElement } from "react"
import {
  StyleProp,
  TextStyle,
  TouchableOpacity,
  TouchableOpacityProps,
  View,
  ViewStyle,
} from "react-native"
import { useSafeAreaInsets } from "react-native-safe-area-context"

import { Text } from "./core/Text"

export interface HeaderProps {
  /**
   * The layout of the title relative to the action components.
   * - `center` will force the title to always be centered relative to the header.
   * - `flex` will attempt to center the title relative to the action buttons.
   */
  titleMode?: "center" | "flex"
  /**
   * Optional title style override.
   */
  titleStyle?: StyleProp<TextStyle>
  /**
   * Optional outer title container style override.
   */
  titleContainerStyle?: StyleProp<ViewStyle>
  /**
   * Optional inner header wrapper style override.
   */
  style?: StyleProp<ViewStyle>
  /**
   * Optional outer header container style override.
   */
  containerStyle?: StyleProp<ViewStyle>
  /**
   * Background color
   */
  backgroundColor?: string
  /**
   * Title text to display.
   */
  title?: string
  /**
   * Custom ReactElement to replace the title text.
   */
  TitleActionComponent?: () => React.ReactNode
  /**
   * Left action custom ReactElement.
   */
  LeftActionComponent?: ReactElement
  /**
   * Right action custom ReactElement.
   */
  RightActionComponent?: ReactElement
  /**
   * Override the default edges for the safe area.
   */
  safeAreaEdges?: ("top" | "bottom" | "left" | "right")[]
}

/**
 * Header that appears on many screens. Will hold navigation buttons and screen title.
 */
export function Header(props: HeaderProps) {
  const {
    backgroundColor = '#1a1a1a', // Our black theme
    LeftActionComponent,
    RightActionComponent,
    safeAreaEdges = ["top"],
    title,
    titleMode = "center",
    TitleActionComponent,
    titleContainerStyle: $titleContainerStyleOverride,
    style: $styleOverride,
    titleStyle: $titleStyleOverride,
    containerStyle: $containerStyleOverride,
  } = props

  const insets = useSafeAreaInsets()
  
  // Calculate safe area style
  const $containerInsets: ViewStyle = {
    paddingTop: safeAreaEdges.includes("top") ? insets.top : 0,
    paddingBottom: safeAreaEdges.includes("bottom") ? insets.bottom : 0,
    paddingLeft: safeAreaEdges.includes("left") ? insets.left : 0,
    paddingRight: safeAreaEdges.includes("right") ? insets.right : 0,
  }

  return (
    <View style={[$container, $containerInsets, { backgroundColor }, $containerStyleOverride]}>
      <View style={[$row, $wrapper, $styleOverride]}>
        {LeftActionComponent ? (
          LeftActionComponent
        ) : (
          <View style={$actionFillerContainer} />
        )}

        {TitleActionComponent ? (
          <View
            style={[
              $titleWrapperBase,
              titleMode === "center" && $titleWrapperCenter,
              titleMode === "flex" && $titleWrapperFlex,
              $titleContainerStyleOverride,
            ]}
          >
            {TitleActionComponent()}
          </View>
        ) : (
          !!title && (
            <View
              style={[
                $titleWrapperNoPointer,
                titleMode === "center" && $titleWrapperCenter,
                titleMode === "flex" && $titleWrapperFlex,
                $titleContainerStyleOverride,
              ]}
            >
              <Text
                weight="medium"
                size="md"
                text={title}
                style={[$title, $titleStyleOverride]}
              />
            </View>
          )
        )}

        {RightActionComponent ? (
          RightActionComponent
        ) : (
          <View style={$actionFillerContainer} />
        )}
      </View>
    </View>
  )
}

const $row: ViewStyle = {
  flexDirection: "row",
}

const $wrapper: ViewStyle = {
  height: 48,
  alignItems: "center",
  justifyContent: "space-between",
}

const $container: ViewStyle = {
  width: "100%",
}

const $title: TextStyle = {
  textAlign: "center",
}

const $actionFillerContainer: ViewStyle = {
  width: 16,
}

const $titleWrapperBase: ViewStyle = {
  // Base styles for title wrapper
}

const $titleWrapperNoPointer: ViewStyle = {
  ...$titleWrapperBase,
  pointerEvents: "none",
}

const $titleWrapperCenter: ViewStyle = {
  alignItems: "center",
  justifyContent: "center",
  height: "100%",
  width: "100%",
  position: "absolute",
  paddingHorizontal: 32, // xxl spacing equivalent
  zIndex: 1,
}

const $titleWrapperFlex: ViewStyle = {
  justifyContent: "center",
  alignItems: "center",
  flexGrow: 1,
}