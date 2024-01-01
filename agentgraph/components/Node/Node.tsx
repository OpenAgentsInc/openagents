import React from 'react'
import { NodeProps } from "./Node.props"
import { NodeContent, NodePanel, NodeTitleBar } from "./Node.styles"
import { useTransform } from '../../hooks'
import { TitleBar } from './TitleBar'

export const Node = ({ position, step }: NodeProps) => {
  const [rootRef, set, currentPos] = useTransform<HTMLDivElement>()
  React.useEffect(() => {
    set({ x: position?.x, y: position?.y })
  }, [position, set])

  // Only Step nodes supported for now, so return null if no step
  if (!step) return null

  return (
    <NodePanel ref={rootRef}>
      <TitleBar
        from={currentPos}
        onDrag={(point) => {
          set(point)
        }}
        title={`${step.order}. ${step.name}`}
      />
      <NodeContent>
        <p>{step.description}</p>
      </NodeContent >
    </NodePanel >
  )
}
