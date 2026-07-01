import type { IconName } from './icon'
import { iconElement } from './icon-dom'

export const BASECOAT_CONTEXT_MENU_ROOT_CLASS = 'dropdown-menu oa-ui-menu-dom'
export const BASECOAT_CONTEXT_MENU_POPOVER_CLASS = 'oa-ui-menu-dom-popover'
export const BASECOAT_CONTEXT_MENU_MENU_CLASS = 'oa-ui-menu-dom-menu'

export type BasecoatMenuDomPoint = Readonly<{
  x: number
  y: number
}>

export type BasecoatMenuDomItem = Readonly<{
  id: string
  label: string
  description?: string
  icon?: IconName
  shortcut?: string
  disabled?: boolean
  destructive?: boolean
  onSelect: () => void
}>

export type BasecoatMenuDomSection = Readonly<{
  label?: string
  items: readonly BasecoatMenuDomItem[]
}>

export type BasecoatMenuDomContent = Readonly<{
  label: string
  header?: Node | string
  sections: readonly BasecoatMenuDomSection[]
}>

export type BasecoatContextMenuOptions = Readonly<{
  id?: string
  ownerDocument?: Document
  className?: string
}>

export type BasecoatContextMenu = Readonly<{
  element: HTMLDivElement
  openAt: (
    point: BasecoatMenuDomPoint,
    content: BasecoatMenuDomContent,
  ) => void
  close: () => void
  destroy: () => void
  isOpen: () => boolean
}>

const clamp = (value: number, min: number, max: number): number =>
  Math.min(Math.max(value, min), max)

const asElementTarget = (target: EventTarget | null): Node | null =>
  target instanceof Node ? target : null

const createText = (
  ownerDocument: Document,
  tag: keyof HTMLElementTagNameMap,
  className: string,
  text: string,
): HTMLElement => {
  const node = ownerDocument.createElement(tag)
  node.className = className
  node.textContent = text
  return node
}

export const createBasecoatContextMenu = (
  options: BasecoatContextMenuOptions = {},
): BasecoatContextMenu => {
  const ownerDocument = options.ownerDocument ?? document
  const ownerWindow = ownerDocument.defaultView ?? window
  const root = ownerDocument.createElement('div')
  root.id = options.id ?? `oa-ui-menu-dom-${Math.random().toString(36).slice(2)}`
  root.className = [
    BASECOAT_CONTEXT_MENU_ROOT_CLASS,
    options.className,
  ].filter((value): value is string => value !== undefined && value.length > 0).join(' ')
  root.dataset.dropdownMenuInitialized = 'true'
  root.dataset.oaUiMenuDom = ''
  root.hidden = true

  const popover = ownerDocument.createElement('div')
  popover.className = BASECOAT_CONTEXT_MENU_POPOVER_CLASS
  popover.dataset.popover = ''
  popover.setAttribute('aria-hidden', 'true')

  const menu = ownerDocument.createElement('div')
  menu.className = BASECOAT_CONTEXT_MENU_MENU_CLASS
  menu.id = `${root.id}-menu`
  menu.setAttribute('role', 'menu')
  menu.tabIndex = -1
  popover.append(menu)
  root.append(popover)
  ownerDocument.body.append(root)

  let activeIndex = -1
  let menuItems: HTMLButtonElement[] = []

  const setActiveIndex = (index: number): void => {
    if (menuItems.length === 0) {
      activeIndex = -1
      return
    }

    activeIndex = clamp(index, 0, menuItems.length - 1)
    menuItems.forEach((item, itemIndex) => {
      const active = itemIndex === activeIndex
      item.classList.toggle('active', active)
      item.dataset.active = active ? 'true' : 'false'
      if (active) item.focus({ preventScroll: true })
    })
  }

  const close = (): void => {
    if (root.hidden) return
    root.hidden = true
    popover.setAttribute('aria-hidden', 'true')
    root.removeAttribute('data-open')
    activeIndex = -1
    menuItems = []
    ownerDocument.removeEventListener('pointerdown', onDocumentPointerDown, true)
    ownerDocument.removeEventListener('keydown', onDocumentKeyDown, true)
  }

  const selectItem = (item: BasecoatMenuDomItem): void => {
    if (item.disabled === true) return
    close()
    item.onSelect()
  }

  const renderItem = (item: BasecoatMenuDomItem): HTMLButtonElement => {
    const button = ownerDocument.createElement('button')
    button.type = 'button'
    button.className = 'oa-ui-menu-dom-item'
    button.id = `${root.id}-${item.id}`
    button.dataset.menuItem = item.id
    button.setAttribute('role', 'menuitem')
    if (item.destructive === true) button.dataset.destructive = 'true'
    if (item.disabled === true) {
      button.disabled = true
      button.setAttribute('aria-disabled', 'true')
    }

    if (item.icon !== undefined) {
      button.append(iconElement(item.icon, {
        className: 'oa-ui-menu-dom-icon',
        dataIcon: item.id,
      }))
    }

    const label = ownerDocument.createElement('span')
    label.className = 'oa-ui-menu-dom-item-label'
    label.textContent = item.label
    const text = ownerDocument.createElement('span')
    text.className = 'oa-ui-menu-dom-item-text'
    text.append(label)
    if (item.description !== undefined) {
      text.append(createText(ownerDocument, 'span', 'oa-ui-menu-dom-item-description', item.description))
    }
    button.append(text)

    if (item.shortcut !== undefined) {
      const shortcut = ownerDocument.createElement('kbd')
      shortcut.className = 'oa-ui-menu-dom-shortcut'
      shortcut.dataset.shortcut = ''
      shortcut.textContent = item.shortcut
      button.append(shortcut)
    }

    button.addEventListener('click', event => {
      event.preventDefault()
      event.stopPropagation()
      selectItem(item)
    })

    return button
  }

  const renderContent = (content: BasecoatMenuDomContent): void => {
    menu.replaceChildren()
    menu.setAttribute('aria-label', content.label)
    menuItems = []
    activeIndex = -1

    if (content.header !== undefined) {
      const header = ownerDocument.createElement('div')
      header.className = 'oa-ui-menu-dom-header'
      if (typeof content.header === 'string') {
        header.textContent = content.header
      } else {
        header.append(content.header)
      }
      menu.append(header)
    }

    for (const [sectionIndex, section] of content.sections.entries()) {
      if (section.items.length === 0) continue
      if (sectionIndex > 0) {
        const separator = ownerDocument.createElement('hr')
        separator.setAttribute('role', 'separator')
        separator.className = 'oa-ui-menu-dom-separator'
        menu.append(separator)
      }

      if (section.label !== undefined) {
        menu.append(createText(ownerDocument, 'div', 'oa-ui-menu-dom-label', section.label))
      }

      for (const item of section.items) {
        const itemElement = renderItem(item)
        menu.append(itemElement)
        if (item.disabled !== true) menuItems.push(itemElement)
      }
    }
  }

  const positionAt = (point: BasecoatMenuDomPoint): void => {
    const margin = 8
    root.style.left = `${point.x}px`
    root.style.top = `${point.y}px`
    root.style.visibility = 'hidden'
    root.hidden = false

    const rect = root.getBoundingClientRect()
    const maxLeft = Math.max(margin, ownerWindow.innerWidth - rect.width - margin)
    const maxTop = Math.max(margin, ownerWindow.innerHeight - rect.height - margin)
    root.style.left = `${clamp(point.x, margin, maxLeft)}px`
    root.style.top = `${clamp(point.y, margin, maxTop)}px`
    root.style.visibility = ''
  }

  function onDocumentPointerDown(event: PointerEvent): void {
    const target = asElementTarget(event.target)
    if (target !== null && root.contains(target)) return
    close()
  }

  function onDocumentKeyDown(event: KeyboardEvent): void {
    if (root.hidden) return
    switch (event.key) {
      case 'Escape':
        event.preventDefault()
        close()
        break
      case 'ArrowDown':
        event.preventDefault()
        setActiveIndex(activeIndex === -1 ? 0 : activeIndex + 1)
        break
      case 'ArrowUp':
        event.preventDefault()
        setActiveIndex(activeIndex === -1 ? menuItems.length - 1 : activeIndex - 1)
        break
      case 'Home':
        event.preventDefault()
        setActiveIndex(0)
        break
      case 'End':
        event.preventDefault()
        setActiveIndex(menuItems.length - 1)
        break
      case 'Enter':
      case ' ':
      case 'Spacebar':
        if (activeIndex >= 0) {
          event.preventDefault()
          menuItems[activeIndex]?.click()
        }
        break
      default:
        break
    }
  }

  return {
    element: root,
    openAt(point, content) {
      renderContent(content)
      popover.setAttribute('aria-hidden', 'false')
      root.dataset.open = 'true'
      positionAt(point)
      ownerDocument.addEventListener('pointerdown', onDocumentPointerDown, true)
      ownerDocument.addEventListener('keydown', onDocumentKeyDown, true)
      menu.focus({ preventScroll: true })
    },
    close,
    destroy() {
      close()
      root.remove()
    },
    isOpen: () => !root.hidden,
  }
}
