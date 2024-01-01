import { useDrag } from "../../hooks";
import { NodeTitleBar } from "./Node.styles";

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
    <NodeTitleBar {...bind()}>
      <p>{title}</p>
    </NodeTitleBar>
  )
}
