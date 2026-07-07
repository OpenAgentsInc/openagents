import { useLayoutEffect, useMemo, useState } from "react"
import {
  Image,
  type ImageProps,
  type ImageResolvedAssetSource,
  type ImageStyle,
  type ImageURISource,
  Platform,
  StyleSheet,
} from "react-native"

export type KhalaAutoImageProps = ImageProps &
  Readonly<{
    maxHeight?: number
    maxWidth?: number
  }>

const resolveStyleSize = (style: ImageProps["style"]) => {
  const flat = StyleSheet.flatten(style) as ImageStyle | undefined
  return {
    height: typeof flat?.height === "number" ? flat.height : undefined,
    width: typeof flat?.width === "number" ? flat.width : undefined,
  }
}

const fitSize = (
  sourceSize: { height: number; width: number },
  bounds: { maxHeight?: number; maxWidth?: number },
) => {
  const { height, width } = sourceSize
  if (height <= 0 || width <= 0) return { height: 0, width: 0 }

  const { maxHeight, maxWidth } = bounds
  if (maxHeight !== undefined && maxWidth !== undefined) {
    const ratio = Math.min(maxWidth / width, maxHeight / height)
    return { height: height * ratio, width: width * ratio }
  }
  if (maxWidth !== undefined) return { height: maxWidth * (height / width), width: maxWidth }
  if (maxHeight !== undefined) return { height: maxHeight, width: maxHeight * (width / height) }
  return { height, width }
}

export const KhalaAutoImage = ({ maxHeight, maxWidth, source, style, ...props }: KhalaAutoImageProps) => {
  const styleSize = useMemo(() => resolveStyleSize(style), [style])
  const [sourceSize, setSourceSize] = useState<{ height: number; width: number }>({ height: 0, width: 0 })

  useLayoutEffect(() => {
    let mounted = true
    const resolved =
      source === undefined ? undefined : (Image.resolveAssetSource(source) as ImageResolvedAssetSource | undefined)
    const uri =
      typeof source === "number"
        ? undefined
        : ((source as ImageURISource | undefined)?.uri ?? (Platform.OS === "web" && typeof source === "string" ? source : undefined))

    if (uri !== undefined) {
      Image.getSize(
        uri,
        (width, height) => {
          if (mounted) setSourceSize({ height, width })
        },
        () => {
          if (mounted && resolved !== undefined) setSourceSize({ height: resolved.height, width: resolved.width })
        },
      )
    } else if (resolved !== undefined) {
      setSourceSize({ height: resolved.height, width: resolved.width })
    }

    return () => {
      mounted = false
    }
  }, [source])

  const fitted = fitSize(sourceSize, {
    maxHeight: styleSize.height ?? maxHeight,
    maxWidth: styleSize.width ?? maxWidth,
  })

  return <Image {...props} source={source} style={[fitted, style]} />
}
