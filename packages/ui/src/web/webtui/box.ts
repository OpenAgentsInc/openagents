import { div } from "@typed/ui/hyperscript"

export type BoxProps = {
  children: any
  box?: "square" | "round" | "double" | undefined
  shear?: "top" | "bottom" | "both" | undefined
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Box = (props: BoxProps): any => {
  const { box, children, className, shear, style, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  // Add box border attribute if specified
  if (box) {
    attributes[`box-`] = box
  }

  // Add shear attribute if specified
  if (shear) {
    attributes[`shear-`] = shear
  }

  // Add className if provided
  if (className) {
    attributes.className = className
  }

  // Add style if provided
  if (style) {
    attributes.style = style
  }

  return div(attributes, children)
}

// Convenience components for specific box types
export const SquareBox = (props: Omit<BoxProps, "box">): any => Box({ box: "square", ...props } as BoxProps)

export const RoundBox = (props: Omit<BoxProps, "box">): any => Box({ box: "round", ...props } as BoxProps)

export const DoubleBox = (props: Omit<BoxProps, "box">): any => Box({ box: "double", ...props } as BoxProps)
