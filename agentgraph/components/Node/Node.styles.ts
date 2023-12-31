import { styled } from '../../styles'

export const NodePanel = styled('div', {
  backgroundColor: '$elevation3',
  borderRadius: '6px',
  display: 'flex',
  alignItems: 'stretch',
  justifyContent: 'space-between',
  flexDirection: 'column',
  height: '100%',
  color: '$highlight3',
  fontFamily: '$sans',
  maxWidth: '350px',
})

export const NodeTitleBar = styled('div', {
  backgroundColor: '$elevation1',
  borderRadius: '6px',
  height: '$titleBarHeight',
  color: '$highlight3',
  fontFamily: '$mono',
  fontSize: '13px',
  textAlign: 'center',
})

export const NodeContent = styled('div', {
  padding: '0 18px',
  flexGrow: 1,
  overflow: 'auto',
})
