import { useEffect } from 'react'
import { useTransform } from '../../hooks'
import { NodeProps } from "./Node.props"
import { NodeContent, NodePanel } from "./Node.styles"
import { TitleBar } from './TitleBar'
import { Label, Row } from '../UI'
import { String } from '../String/String'

export const Node = ({ agent, position, step }: NodeProps) => {
  // Handle canvas position & dragging
  const [rootRef, set, currentPos] = useTransform<HTMLDivElement>()
  useEffect(() => {
    set({ x: position?.x, y: position?.y })
  }, [position, set])

  if (!!agent) {
    return (
      <NodePanel ref={rootRef}>
        <TitleBar
          from={currentPos}
          onDrag={(point) => set(point)}
          title={agent.name}
        />
        <NodeContent>
          <p>Task: {agent.tasks[0].description}</p>
          {/* <p>Steps: {agent.tasks[0].steps.length}</p> */}
        </NodeContent>
      </NodePanel>
    )
  }

  // Only Step nodes supported for now, so return null if no step
  if (!step) return null

  const fieldsToRender = ['category', 'entry_type', 'success_action', 'created_at']

  return (
    <NodePanel ref={rootRef}>
      <TitleBar
        from={currentPos}
        onDrag={(point) => set(point)}
        title={`${step.order}. ${step.name}`}
      />
      <NodeContent>
        <p>{step.description}</p>
        {/* For every fieldToRender, render a Field */}
        {fieldsToRender.map((field) => {
          const onUpdate = (update) => {
            console.log('update:', update)
          }
          const onChange = (change) => {
            console.log('change:', change)
          }
          return (
            <Row input key={field}>
              <Label>{field}</Label>
              <String displayValue={step[field]} onUpdate={onUpdate} onChange={onChange} />
            </Row>
          )
        })}
      </NodeContent>
    </NodePanel >
  )
}
