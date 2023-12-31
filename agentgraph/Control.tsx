import React from 'react'
import { String } from './components/String/String'
import { Label, Row } from './components/UI'
import { StyledInputWrapper } from './components/UI/StyledUI'
import { InputContext } from './context'

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

  const onUpdate = () => { }
  const onChange = () => { }
  const disabled = false

  if (!data) return <></>

  // And let's start with String
  if (type === 'text') {
    return (
      <InputContext.Provider
        value={{
          // key: valueKey,
          path,
          id: '' + path,
          // label,
          // displayValue,
          // value,
          onChange,
          onUpdate,
          // settings,
          // setValue,
          disabled,
          // ...rest,
          emitOnEditStart: () => { },
          emitOnEditEnd: () => { },
        }}>
        <StyledInputWrapper disabled={disabled}>
          <Row input>
            <Label>{path}</Label>
            <String displayValue={data} onUpdate={onUpdate} onChange={onChange} />
          </Row>
        </StyledInputWrapper>
      </InputContext.Provider>
    )
  }

  return <></>
})
