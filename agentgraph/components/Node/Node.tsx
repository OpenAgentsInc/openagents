import { useEffect } from 'react'
import { useTransform } from '../../hooks'
import { NodeProps } from "./Node.props"
import { NodeContent, NodePanel } from "./Node.styles"
import { TitleBar } from './TitleBar'

export const Node = ({ position, step }: NodeProps) => {
  // Handle canvas position & dragging
  const [rootRef, set, currentPos] = useTransform<HTMLDivElement>()
  useEffect(() => {
    set({ x: position?.x, y: position?.y })
  }, [position, set])

  // Only Step nodes supported for now, so return null if no step
  if (!step) return null

  return (
    <NodePanel ref={rootRef}>
      <TitleBar
        from={currentPos}
        onDrag={(point) => set(point)}
        title={`${step.order}. ${step.name}`}
      />
      <NodeContent>
        <p>{step.description}</p>
      </NodeContent >
    </NodePanel >
  )
}
