export type InputContextProps = {
  id: string
  label: string | JSX.Element
  hint?: string
  path: string
  key: string
  optional: boolean
  disabled: boolean
  disable: (flag: boolean) => void
  storeId: string
  value: unknown
  displayValue: unknown
  onChange: React.Dispatch<any>
  emitOnEditStart: () => void
  emitOnEditEnd: () => void
  onUpdate: (v: any | ((v: any) => any)) => void
  settings: unknown
  setSettings: (v: any) => void
}
