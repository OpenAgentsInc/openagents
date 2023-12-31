import { styled } from '../../styles'

export const NodePanel = styled('div', {
  backgroundColor: '$elevation3',
  borderRadius: '4px',
  display: 'flex',
  alignItems: 'stretch',
  justifyContent: 'space-between',
  flexDirection: 'column',
  height: '100%',
  color: '$highlight3',
  fontFamily: '$sans',
})

export const NodeTitleBar = styled('div', {
  backgroundColor: '$elevation1',
  borderRadius: '4px',
  // display: 'flex',
  // alignItems: 'stretch',
  // justifyContent: 'space-between',
  height: '$titleBarHeight',
  color: '$highlight3',
  // fontFamily: '$mono',
})
