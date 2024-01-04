
import { BalanceContainer, Lightning } from "./BitcoinBalance.styles"

export const BitcoinBalance = ({ sats }) => {
  return (
    <BalanceContainer>
      <Lightning /> {sats}
    </BalanceContainer>
  )
}
