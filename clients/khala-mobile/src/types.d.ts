import "react-native"

declare module "react-native" {
  interface ViewProps {
    readonly className?: string
  }

  interface TextProps {
    readonly className?: string
  }

  interface PressableProps {
    readonly className?: string
  }

  interface TextInputProps {
    readonly className?: string
  }
}
