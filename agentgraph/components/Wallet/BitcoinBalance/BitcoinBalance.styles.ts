import { LightningBoltIcon } from "@radix-ui/react-icons"
import { styled } from '../../../styles'

export const BalanceContainer = styled('div', {
  position: 'relative',
  backgroundColor: '$elevation3',
  padding: '9px',
  borderRadius: '6px',
  color: '$highlight3',
  fontFamily: '$sans',
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
})

export const Lightning = styled(LightningBoltIcon, {
  color: '$vivid1',
  paddingRight: '3px',
});
