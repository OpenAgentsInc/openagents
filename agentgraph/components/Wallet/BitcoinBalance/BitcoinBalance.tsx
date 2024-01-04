
import { BalanceContainer, Lightning } from "./BitcoinBalance.styles"

export const BitcoinBalance = ({ balance }) => {
  return (
    <BalanceContainer>
      <Lightning /> {balance}
    </BalanceContainer>
  )
}
