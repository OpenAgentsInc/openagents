export interface PaneHeaderMenu {
  icon?: React.ReactNode
  label: string
  onClick: () => void
}

export interface PaneContent {
  type: string
  data?: any
}