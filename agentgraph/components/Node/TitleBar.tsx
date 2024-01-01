import { useRef } from "react";
import { useDrag } from "../../hooks";
import { StyledTitleBar } from "./Node.styles";

interface TitleBarProps {
  from: { x?: number; y?: number }
  onDrag: (point: { x?: number; y?: number }) => void
  title: string
}

export const TitleBar = ({ from, onDrag, title }: TitleBarProps) => {
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

  return (
    <StyledTitleBar {...bind()} ref={titleBarRef}>
      <div>{title}</div>
    </StyledTitleBar>
  )
}
