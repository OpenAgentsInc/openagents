import { styled } from '../../styles'

export const NodePanel = styled('div', {
  position: 'absolute',
  backgroundColor: '$elevation2',
  borderRadius: '6px',
  display: 'flex',
  alignItems: 'stretch',
  justifyContent: 'space-between',
  flexDirection: 'column',
  color: '$highlight3',
  fontFamily: '$sans',
  maxWidth: '350px',
})

export const StyledTitleBar = styled('div', {
  touchAction: 'none',
  $flexCenter: '',
  flex: 1,
  backgroundColor: '$elevation1',
  borderRadius: '6px',
  height: '$titleBarHeight',
  color: '$highlight3',
  fontFamily: '$mono',
  fontSize: '13px',
  textAlign: 'center',
  cursor: 'grab',
  userSelect: 'none',
  padding: '8px 8px',
})

export const StyledTitleBarWithBalance = styled(StyledTitleBar, {
  justifyContent: 'space-between',
})

export const NodeContent = styled('div', {
  textAlign: 'center',
  padding: '10px 18px 8px',
  flexGrow: 1,
  overflow: 'auto',
  fontSize: '13px',
})
