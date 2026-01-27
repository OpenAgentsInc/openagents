import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { cx, type AIChildren } from "./utils.js"

export type FileTreeProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const FileTree = ({ className, children }: FileTreeProps): TemplateResult => html`
  <div class="${cx("rounded-lg border bg-background font-mono text-sm", className)}" role="tree">
    <div class="p-2">${children ?? ""}</div>
  </div>
`

export type FileTreeFolderProps = {
  readonly path: string
  readonly name: string
  readonly className?: string
  readonly children?: AIChildren
  readonly expanded?: boolean
}

export const FileTreeFolder = ({ path, name, className, children, expanded = false }: FileTreeFolderProps): TemplateResult =>
  Collapsible({
    open: expanded,
    children: html`
      <div class="${cx("", className)}" role="treeitem" tabindex="0" data-path="${path}">
        ${CollapsibleTrigger({
          children: html`
            <button
              class="${cx("flex w-full items-center gap-1 rounded px-2 py-1 text-left transition-colors hover:bg-muted/50", expanded ? "bg-muted" : "")}" type="button"
            >
              <span class="size-4 shrink-0 text-muted-foreground">${expanded ? "v" : ">"}</span>
              ${FileTreeIcon({ children: expanded ? "dir" : "dir" })}
              ${FileTreeName({ children: name })}
              ${FileTreeActions({})}
            </button>
          `,
        })}
        ${CollapsibleContent({ children })}
      </div>
    `,
  })

export type FileTreeFileProps = {
  readonly path: string
  readonly name: string
  readonly className?: string
  readonly children?: AIChildren
  readonly selected?: boolean
}

export const FileTreeFile = ({ path, name, className, children, selected = false }: FileTreeFileProps): TemplateResult => html`
  <div class="${cx("", className)}" role="treeitem" tabindex="0" data-path="${path}">
    <div class="${cx("flex w-full items-center gap-1 rounded px-2 py-1 text-left transition-colors hover:bg-muted/50", selected ? "bg-muted" : "")}">
      ${FileTreeIcon({ children: "file" })}
      ${FileTreeName({ children: name })}
      ${FileTreeActions({ children })}
    </div>
  </div>
`

export type FileTreeIconProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const FileTreeIcon = ({ className, children }: FileTreeIconProps): TemplateResult => html`
  <span class="${cx("size-4 shrink-0 text-muted-foreground", className)}">${children ?? "file"}</span>
`

export type FileTreeNameProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const FileTreeName = ({ className, children }: FileTreeNameProps): TemplateResult => html`
  <span class="${cx("truncate", className)}">${children ?? ""}</span>
`

export type FileTreeActionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const FileTreeActions = ({ className, children }: FileTreeActionsProps): TemplateResult => html`
  <span class="${cx("ml-auto flex items-center gap-1", className)}">${children ?? ""}</span>
`
