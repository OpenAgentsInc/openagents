import { useRef } from "react";
import { useDrag } from "../../hooks";
import { StyledTitleBar, StyledTitleBarWithBalance } from "./Node.styles";
import { BitcoinBalance } from "../Wallet";

interface TitleBarProps {
  balance?: number
  from: { x?: number; y?: number }
  onDrag: (point: { x?: number; y?: number }) => void
  title: string
}

export const TitleBar = ({ balance, from, onDrag, title }: TitleBarProps) => {
  const titleBarRef = useRef<any>(null);
  const bind = useDrag(
    ({ offset: [x, y], first, last }) => {
      onDrag({ x, y })

      if (first) {
        if (titleBarRef.current) {
          titleBarRef.current.style.cursor = 'grabbing';
        }
      }

      if (last) {
        if (titleBarRef.current) {
          titleBarRef.current.style.cursor = 'grab';
        }
      }
    },
    {
      filterTaps: true,
      from: ({ offset: [x, y] }) => [from?.x || x, from?.y || y],
    }
  )

  if (!balance) {
    return (
      <StyledTitleBar {...bind()} ref={titleBarRef}>
        <div>{title}</div>
      </StyledTitleBar>
    )
  }

  return (
    <StyledTitleBarWithBalance {...bind()} ref={titleBarRef}>
      <div style={{ width: 40 }}></div> {/* bad hack */}
      <div>{title}</div>
      <BitcoinBalance sats={balance} />
    </StyledTitleBarWithBalance>
  )
}
