import { table, tbody, td, tfoot, th, thead, tr } from "@typed/ui/hyperscript"

export type TableProps = {
  children: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export type TableHeadProps = {
  children: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export type TableBodyProps = {
  children: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export type TableFootProps = {
  children: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export type TableRowProps = {
  children: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export type TableHeaderProps = {
  children: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export type TableCellProps = {
  children: any
  className?: string | undefined
  style?: string | undefined
  [key: string]: any
}

export const Table = (props: TableProps): any => {
  const { children, className, style, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  if (className) {
    attributes.className = className
  }

  if (style) {
    attributes.style = style
  }

  return table(attributes, children)
}

export const TableHead = (props: TableHeadProps): any => {
  const { children, className, style, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  if (className) {
    attributes.className = className
  }

  if (style) {
    attributes.style = style
  }

  return thead(attributes, children)
}

export const TableBody = (props: TableBodyProps): any => {
  const { children, className, style, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  if (className) {
    attributes.className = className
  }

  if (style) {
    attributes.style = style
  }

  return tbody(attributes, children)
}

export const TableFoot = (props: TableFootProps): any => {
  const { children, className, style, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  if (className) {
    attributes.className = className
  }

  if (style) {
    attributes.style = style
  }

  return tfoot(attributes, children)
}

export const TableRow = (props: TableRowProps): any => {
  const { children, className, style, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  if (className) {
    attributes.className = className
  }

  if (style) {
    attributes.style = style
  }

  return tr(attributes, children)
}

export const TableHeader = (props: TableHeaderProps): any => {
  const { children, className, style, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  if (className) {
    attributes.className = className
  }

  if (style) {
    attributes.style = style
  }

  return th(attributes, children)
}

export const TableCell = (props: TableCellProps): any => {
  const { children, className, style, ...otherProps } = props

  const attributes: Record<string, any> = {
    ...otherProps
  }

  if (className) {
    attributes.className = className
  }

  if (style) {
    attributes.style = style
  }

  return td(attributes, children)
}
