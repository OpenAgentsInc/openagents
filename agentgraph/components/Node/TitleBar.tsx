import { useDrag } from "../../hooks";
import { StyledTitleBar } from "./Node.styles";

interface TitleBarProps {
  from: { x?: number; y?: number }
  onDrag: (point: { x?: number; y?: number }) => void
  title: string
}

export const TitleBar = ({ from, onDrag, title }: TitleBarProps) => {

  const bind = useDrag(
    ({ offset: [x, y], first, last }) => {
      onDrag({ x, y })
    },
    {
      filterTaps: true,
      from: ({ offset: [x, y] }) => [from?.x || x, from?.y || y],
    }
  )

  return (
    <StyledTitleBar {...bind()}>
      <p>{title}</p>
    </StyledTitleBar>
  )
}
