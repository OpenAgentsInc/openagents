import React from 'react'
import { String } from './components/String/String'
import { Label, Row } from './components/UI'

type ControlProps = { path: string, data: any }

export const Control = React.memo(({ path, data }: ControlProps) => {
  // For now we'll select a ControlInput type based on the data type: number/boolean/string etc.
  let type
  switch (typeof data) {
    case 'number':
      type = 'number'
      break
    case 'boolean':
      type = 'checkbox'
      break
    case 'string':
      type = 'text'
      break
    default:
      type = 'text'
  }

  // console.log(type, data)

  const onUpdate = () => { }
  const onChange = () => { }

  if (!data) return <></>

  // And let's start with String
  if (type === 'text') {
    return (
      <Row input>
        <Label>test label</Label>
        <String displayValue={data} onUpdate={onUpdate} onChange={onChange} />
      </Row>
    )
  }

  return <></>
})
