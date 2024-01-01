import { NodeTitleBar } from "./Node.styles";

interface TitleBarProps {
  onDrag: (point: { x?: number; y?: number }) => void
  title: string
}

export const TitleBar = ({ title }: TitleBarProps) => {
  return (
    <NodeTitleBar>
      <p>{title}</p>
    </NodeTitleBar>
  )
}
