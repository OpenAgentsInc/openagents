import type { ReactNode } from "react";
import { KeyboardAvoidingView, Platform, type StyleProp, View, type ViewStyle } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { StatusBar } from "expo-status-bar";

import { colors } from "./theme";

/**
 * The page frame: safe area, status bar, and the keyboard behaviour a composer
 * needs, in the arcade idiom.
 *
 * `preset="fixed"` gives a full-height column a list can fill. A screen that
 * scrolls its own content passes its list instead, because a list inside a
 * scroll view scrolls twice.
 */
export const Screen = ({
  children,
  style,
  keyboardOffset = 0,
  edges = ["top", "bottom"],
}: {
  readonly children: ReactNode;
  readonly style?: StyleProp<ViewStyle>;
  readonly keyboardOffset?: number;
  readonly edges?: ReadonlyArray<"top" | "bottom">;
}) => {
  const insets = useSafeAreaInsets();
  return (
    <View style={$root}>
      <StatusBar style="light" />
      <KeyboardAvoidingView
        behavior={Platform.OS === "ios" ? "padding" : undefined}
        keyboardVerticalOffset={keyboardOffset}
        style={$flex}
      >
        <View
          style={[
            $flex,
            {
              paddingTop: edges.includes("top") ? insets.top : 0,
              paddingBottom: edges.includes("bottom") ? insets.bottom : 0,
            },
            style,
          ]}
        >
          {children}
        </View>
      </KeyboardAvoidingView>
    </View>
  );
};

const $root: ViewStyle = { flex: 1, backgroundColor: colors.background };
const $flex: ViewStyle = { flex: 1 };
