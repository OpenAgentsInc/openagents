import { Effect } from "effect"

const toArray = <T extends Element>(list: NodeListOf<T>): T[] => Array.from(list)

const getState = (el: Element | null): "open" | "closed" =>
  el?.getAttribute("data-state") === "open" ? "open" : "closed"

const setHidden = (el: Element | null, hidden: boolean) => {
  if (!el) return
  if (hidden) {
    el.setAttribute("hidden", "")
  } else {
    el.removeAttribute("hidden")
  }
}

const setState = (el: Element | null, state: "open" | "closed", hideWhenClosed = false) => {
  if (!el) return
  el.setAttribute("data-state", state)
  if (hideWhenClosed) {
    setHidden(el, state === "closed")
  }
}

const resolveTarget = (
  root: Element,
  actionEl: Element,
  targetSpec: string | null
): Element | null => {
  if (!targetSpec || targetSpec === "this") {
    return actionEl
  }

  if (targetSpec.startsWith("closest(") && targetSpec.endsWith(")")) {
    const selector = targetSpec.slice("closest(".length, -1).trim()
    return actionEl.closest(selector)
  }

  if (targetSpec.startsWith("find(") && targetSpec.endsWith(")")) {
    const selector = targetSpec.slice("find(".length, -1).trim()
    return actionEl.querySelector(selector)
  }

  return root.querySelector(targetSpec) ?? document.querySelector(targetSpec)
}

const textValue = (el: Element | null): string =>
  el?.textContent?.replace(/\s+/g, " ").trim() ?? ""

const copyToClipboard = async (value: string) => {
  if (!value) return
  if (navigator?.clipboard?.writeText) {
    await navigator.clipboard.writeText(value)
    return
  }
  const textarea = document.createElement("textarea")
  textarea.value = value
  textarea.style.position = "fixed"
  textarea.style.opacity = "0"
  document.body.appendChild(textarea)
  textarea.select()
  document.execCommand("copy")
  textarea.remove()
}

const applyCopyFeedback = (button: Element, timeoutMs: number) => {
  button.setAttribute("data-copied", "true")
  window.setTimeout(() => {
    button.removeAttribute("data-copied")
  }, timeoutMs)
}

const maskValue = (value: string, max = 20) => "*".repeat(Math.min(value.length, max))

const syncTabs = (root: Element) => {
  const tabs = toArray(root.querySelectorAll<HTMLElement>("[data-slot='tabs']"))
  for (const tabRoot of tabs) {
    const triggers = toArray(
      tabRoot.querySelectorAll<HTMLElement>("[data-slot='tabs-trigger']")
    )
    const contents = toArray(
      tabRoot.querySelectorAll<HTMLElement>("[data-slot='tabs-content']")
    )
    if (triggers.length === 0 || contents.length === 0) continue

    const activeValue = tabRoot.getAttribute("data-value")
    let activeTrigger = triggers.find(
      (trigger) => trigger.getAttribute("data-state") === "active"
    )

    if (!activeTrigger && activeValue) {
      activeTrigger = triggers.find(
        (trigger) => trigger.getAttribute("data-value") === activeValue
      )
    }

    if (!activeTrigger) {
      activeTrigger = triggers[0]
    }

    const resolvedValue =
      activeTrigger.getAttribute("data-value") ??
      `${triggers.indexOf(activeTrigger)}`

    tabRoot.setAttribute("data-value", resolvedValue)

    triggers.forEach((trigger, index) => {
      const value = trigger.getAttribute("data-value") ?? `${index}`
      const isActive = value === resolvedValue
      trigger.setAttribute("data-state", isActive ? "active" : "inactive")
      trigger.setAttribute("aria-selected", isActive ? "true" : "false")
    })

    contents.forEach((content, index) => {
      const value = content.getAttribute("data-value") ?? `${index}`
      const isActive = value === resolvedValue
      content.setAttribute("data-state", isActive ? "active" : "inactive")
      setHidden(content, !isActive)
    })
  }
}

const syncOpenables = (root: Element, selector: string) => {
  const nodes = toArray(root.querySelectorAll<HTMLElement>(selector))
  nodes.forEach((node) => {
    const state = getState(node)
    setHidden(node, state === "closed")
  })
}

const syncCarousel = (root: Element) => {
  const carousels = toArray(root.querySelectorAll<HTMLElement>("[data-slot='carousel']"))
  carousels.forEach((carousel) => {
    const track = carousel.querySelector<HTMLElement>("[data-slot='carousel-track']")
    if (!track) return
    const index = Number.parseInt(carousel.getAttribute("data-index") ?? "0", 10)
    const orientation = carousel.getAttribute("data-orientation") === "vertical" ? "vertical" : "horizontal"
    const offset = -index * 100
    track.style.transform =
      orientation === "vertical" ? `translateY(${offset}%)` : `translateX(${offset}%)`
    track.style.transition = "transform 200ms ease"
  })
}

const syncCollapsibles = (root: Element) => {
  const collapsibles = toArray(root.querySelectorAll<HTMLElement>("[data-slot='collapsible']"))
  collapsibles.forEach((collapsible) => {
    const state = getState(collapsible)
    const content = collapsible.querySelector("[data-slot='collapsible-content']")
    setState(content, state, true)
    const trigger = collapsible.querySelector("[data-slot='collapsible-trigger']")
    if (trigger) {
      trigger.setAttribute("data-state", state)
      trigger.setAttribute("aria-expanded", state === "open" ? "true" : "false")
    }
  })
}

const syncAccordions = (root: Element) => {
  const items = toArray(root.querySelectorAll<HTMLElement>("[data-slot='accordion-item']"))
  items.forEach((item) => {
    const trigger = item.querySelector("[data-slot='accordion-trigger']")
    const content = item.querySelector("[data-slot='accordion-content']")
    const state = getState(content)
    setState(trigger, state, false)
    setState(content, state, true)
  })
}

const syncSelects = (root: Element) => {
  const selects = toArray(root.querySelectorAll<HTMLElement>("[data-slot='select']"))
  selects.forEach((select) => {
    const content = select.querySelector<HTMLElement>("[data-slot='select-content']")
    setState(content, getState(content), true)
    const trigger = select.querySelector<HTMLElement>("[data-slot='select-trigger']")
    if (trigger) {
      trigger.setAttribute("aria-expanded", getState(content) === "open" ? "true" : "false")
    }
  })
}

const updateEnvValues = (root: Element, showValues: boolean) => {
  const values = toArray(
    root.querySelectorAll<HTMLElement>("[data-slot='environment-variable-value']")
  )
  values.forEach((valueEl) => {
    const raw = valueEl.getAttribute("data-value") ?? ""
    valueEl.textContent = showValues ? raw : maskValue(raw)
  })
  const label = root.querySelector<HTMLElement>("[data-slot='environment-variables-toggle-label']")
  if (label) {
    label.textContent = showValues ? "show" : "hide"
  }
}

export const mountUiRuntime = (root: Element) =>
  Effect.gen(function* () {
    const tooltipTimers = new WeakMap<Element, number>()

    const handleCopyClick = (target: Element): boolean => {
      const button = target.closest("[data-ui='copy']") as HTMLElement | null
      if (!button) return false
      const targetSpec = button.getAttribute("data-copy-target")
      const context = resolveTarget(root, button, targetSpec) ?? button
      const directValue = button.getAttribute("data-copy-value")
      const contextValue =
        context?.getAttribute("data-copy-value") ??
        context?.querySelector("[data-copy-value]")?.getAttribute("data-copy-value")
      const value = directValue || contextValue || textValue(context)
      if (!value) return true

      void copyToClipboard(value)
      applyCopyFeedback(button, 1500)
      return true
    }

    const toggleCollapsible = (trigger: HTMLElement) => {
      const rootEl = trigger.closest("[data-slot='collapsible']")
      if (!rootEl) return
      const next = getState(rootEl) === "open" ? "closed" : "open"
      setState(rootEl, next)
      const content = rootEl.querySelector("[data-slot='collapsible-content']")
      setState(content, next, true)
      trigger.setAttribute("data-state", next)
      trigger.setAttribute("aria-expanded", next === "open" ? "true" : "false")
    }

    const setAccordionItemState = (item: Element, state: "open" | "closed") => {
      const trigger = item.querySelector("[data-slot='accordion-trigger']")
      const content = item.querySelector("[data-slot='accordion-content']")
      setState(trigger, state)
      setState(content, state, true)
    }

    const toggleAccordion = (trigger: HTMLElement) => {
      const item = trigger.closest("[data-slot='accordion-item']")
      const rootEl = trigger.closest("[data-slot='accordion']")
      if (!item || !rootEl) return
      const type = rootEl.getAttribute("data-type") ?? "multiple"
      const collapsible = rootEl.getAttribute("data-collapsible") !== "false"
      const isOpen = getState(item.querySelector("[data-slot='accordion-content']")) === "open"
      if (isOpen) {
        if (collapsible) {
          setAccordionItemState(item, "closed")
        }
        return
      }
      if (type === "single") {
        const items = toArray(rootEl.querySelectorAll<HTMLElement>("[data-slot='accordion-item']"))
        items.forEach((other) => {
          if (other === item) return
          setAccordionItemState(other, "closed")
        })
      }
      setAccordionItemState(item, "open")
    }

    const selectTab = (trigger: HTMLElement) => {
      const rootEl = trigger.closest("[data-slot='tabs']")
      if (!rootEl) return
      const triggers = toArray(rootEl.querySelectorAll<HTMLElement>("[data-slot='tabs-trigger']"))
      const contents = toArray(rootEl.querySelectorAll<HTMLElement>("[data-slot='tabs-content']"))
      const triggerIndex = triggers.indexOf(trigger)
      const value = trigger.getAttribute("data-value") ?? `${triggerIndex}`
      rootEl.setAttribute("data-value", value)
      triggers.forEach((tabTrigger, index) => {
        const tabValue = tabTrigger.getAttribute("data-value") ?? `${index}`
        const isActive = tabValue === value
        tabTrigger.setAttribute("data-state", isActive ? "active" : "inactive")
        tabTrigger.setAttribute("aria-selected", isActive ? "true" : "false")
      })
      contents.forEach((content, index) => {
        const contentValue = content.getAttribute("data-value") ?? `${index}`
        const isActive = contentValue === value
        content.setAttribute("data-state", isActive ? "active" : "inactive")
        setHidden(content, !isActive)
      })
    }

    const toggleSelect = (trigger: HTMLElement) => {
      const rootEl = trigger.closest("[data-slot='select']") as HTMLElement | null
      if (!rootEl) return
      const content = rootEl.querySelector<HTMLElement>("[data-slot='select-content']")
      if (!content) return
      const next = getState(content) === "open" ? "closed" : "open"
      setState(content, next, true)
      rootEl.setAttribute("data-state", next)
      trigger.setAttribute("aria-expanded", next === "open" ? "true" : "false")
    }

    const pickSelectItem = (item: HTMLElement) => {
      const rootEl = item.closest("[data-slot='select']") as HTMLElement | null
      if (!rootEl) return
      if (item.getAttribute("data-disabled") === "true") return
      const itemTextEl = item.querySelector<HTMLElement>("[data-slot='select-item-text']")
      const itemText = itemTextEl ? textValue(itemTextEl) : textValue(item)
      const value = item.getAttribute("data-value") ?? itemText
      rootEl.setAttribute("data-value", value)
      const valueEl = rootEl.querySelector<HTMLElement>("[data-slot='select-value']")
      if (valueEl) {
        valueEl.textContent = itemText
      }
      const items = toArray(rootEl.querySelectorAll<HTMLElement>("[data-slot='select-item']"))
      items.forEach((selectItem) => {
        const selected = selectItem === item
        selectItem.setAttribute("data-selected", selected ? "true" : "false")
        selectItem.setAttribute("aria-selected", selected ? "true" : "false")
        const indicator = selectItem.querySelector<HTMLElement>("[data-slot='select-item-indicator']")
        if (indicator) {
          indicator.textContent = selected ? "x" : ""
        }
      })
      const content = rootEl.querySelector<HTMLElement>("[data-slot='select-content']")
      setState(content, "closed", true)
      rootEl.setAttribute("data-state", "closed")
      const trigger = rootEl.querySelector<HTMLElement>("[data-slot='select-trigger']")
      if (trigger) {
        trigger.setAttribute("aria-expanded", "false")
      }
    }

    const toggleDropdown = (trigger: HTMLElement) => {
      const rootEl = trigger.closest("[data-slot='dropdown-menu']") as HTMLElement | null
      if (!rootEl) return
      const content = rootEl.querySelector<HTMLElement>("[data-slot='dropdown-menu-content']")
      if (!content) return
      const next = getState(content) === "open" ? "closed" : "open"
      setState(content, next, true)
      rootEl.setAttribute("data-state", next)
      trigger.setAttribute("aria-expanded", next === "open" ? "true" : "false")
    }

    const closeDropdown = (rootEl: Element | null) => {
      if (!rootEl) return
      const content = rootEl.querySelector<HTMLElement>("[data-slot='dropdown-menu-content']")
      setState(content, "closed", true)
      const subMenus = toArray(
        rootEl.querySelectorAll<HTMLElement>("[data-slot='dropdown-menu-sub-content']")
      )
      subMenus.forEach((submenu) => setState(submenu, "closed", true))
      rootEl.setAttribute("data-state", "closed")
    }

    const handleDropdownItem = (item: HTMLElement) => {
      if (item.getAttribute("data-disabled") === "true") return
      const rootEl = item.closest("[data-slot='dropdown-menu']")
      closeDropdown(rootEl)
    }

    const toggleDropdownCheckbox = (item: HTMLElement) => {
      const checked = item.getAttribute("data-checked") === "true"
      const next = checked ? "false" : "true"
      item.setAttribute("data-checked", next)
      const indicator = item.querySelector<HTMLElement>("[data-slot='dropdown-menu-checkbox-indicator']")
      if (indicator) {
        indicator.textContent = next === "true" ? "x" : ""
      }
    }

    const selectDropdownRadio = (item: HTMLElement) => {
      const group = item.closest("[data-slot='dropdown-menu-radio-group']")
      if (!group) return
      const items = toArray(group.querySelectorAll<HTMLElement>("[data-slot='dropdown-menu-radio-item']"))
      items.forEach((radio) => {
        const selected = radio === item
        radio.setAttribute("data-checked", selected ? "true" : "false")
        const indicator = radio.querySelector<HTMLElement>("[data-slot='dropdown-menu-radio-indicator']")
        if (indicator) {
          indicator.textContent = selected ? "o" : ""
        }
      })
    }

    const toggleDropdownSub = (trigger: HTMLElement) => {
      const rootEl = trigger.closest("[data-slot='dropdown-menu-sub']")
      if (!rootEl) return
      const content = rootEl.querySelector<HTMLElement>("[data-slot='dropdown-menu-sub-content']")
      if (!content) return
      const next = getState(content) === "open" ? "closed" : "open"
      setState(content, next, true)
      trigger.setAttribute("data-state", next)
    }

    const togglePopover = (trigger: HTMLElement) => {
      const rootEl = trigger.closest("[data-slot='popover']")
      if (!rootEl) return
      const content = rootEl.querySelector<HTMLElement>("[data-slot='popover-content']")
      if (!content) return
      const next = getState(content) === "open" ? "closed" : "open"
      setState(content, next, true)
      rootEl.setAttribute("data-state", next)
      trigger.setAttribute("aria-expanded", next === "open" ? "true" : "false")
    }

    const setTooltipState = (trigger: HTMLElement, state: "open" | "closed") => {
      const rootEl = trigger.closest("[data-slot='tooltip']")
      if (!rootEl) return
      const content = rootEl.querySelector<HTMLElement>("[data-slot='tooltip-content']")
      setState(content, state, true)
      rootEl.setAttribute("data-state", state)
    }

    const setHoverCardState = (trigger: HTMLElement, state: "open" | "closed") => {
      const rootEl = trigger.closest("[data-slot='hover-card']")
      if (!rootEl) return
      const content = rootEl.querySelector<HTMLElement>("[data-slot='hover-card-content']")
      setState(content, state, true)
      rootEl.setAttribute("data-state", state)
    }

    const toggleDialog = (trigger: HTMLElement, state: "open" | "closed") => {
      const rootEl = trigger.closest("[data-slot='dialog']") as HTMLElement | null
      if (!rootEl) return
      const overlay = rootEl.querySelector<HTMLElement>("[data-slot='dialog-overlay']")
      const content = rootEl.querySelector<HTMLElement>("[data-slot='dialog-content']")
      rootEl.setAttribute("data-state", state)
      setState(overlay, state, true)
      setState(content, state, true)
    }

    const toggleSwitch = (switchEl: HTMLElement) => {
      const next = switchEl.getAttribute("data-state") === "checked" ? "unchecked" : "checked"
      switchEl.setAttribute("data-state", next)
      switchEl.setAttribute("aria-checked", next === "checked" ? "true" : "false")
      const thumb = switchEl.querySelector<HTMLElement>("[data-slot='switch-thumb']")
      if (thumb) {
        thumb.setAttribute("data-state", next)
      }
    }

    const handleCommandInput = (input: HTMLInputElement) => {
      const rootEl = input.closest("[data-slot='command']")
      if (!rootEl) return
      const query = input.value.trim().toLowerCase()
      const items = toArray(rootEl.querySelectorAll<HTMLElement>("[data-slot='command-item']"))
      let visibleCount = 0
      items.forEach((item) => {
        const text = textValue(item).toLowerCase()
        const visible = !query || text.includes(query)
        setHidden(item, !visible)
        if (visible) visibleCount += 1
      })
      const groups = toArray(rootEl.querySelectorAll<HTMLElement>("[data-slot='command-group']"))
      groups.forEach((group) => {
        const groupItems = toArray(group.querySelectorAll<HTMLElement>("[data-slot='command-item']"))
        const anyVisible = groupItems.some((item) => !item.hasAttribute("hidden"))
        setHidden(group, !anyVisible)
      })
      const empty = rootEl.querySelector<HTMLElement>("[data-slot='command-empty']")
      if (empty) {
        setHidden(empty, visibleCount !== 0)
      }
    }

    const handleCarouselNav = (button: HTMLElement, direction: -1 | 1) => {
      const rootEl = button.closest("[data-slot='carousel']") as HTMLElement | null
      if (!rootEl) return
      const track = rootEl.querySelector<HTMLElement>("[data-slot='carousel-track']")
      if (!track) return
      const items = toArray(track.querySelectorAll<HTMLElement>("[data-slot='carousel-item']"))
      if (items.length === 0) return
      const current = Number.parseInt(rootEl.getAttribute("data-index") ?? "0", 10)
      const next = Math.min(Math.max(current + direction, 0), items.length - 1)
      rootEl.setAttribute("data-index", `${next}`)
      const orientation = rootEl.getAttribute("data-orientation") === "vertical" ? "vertical" : "horizontal"
      const offset = -next * 100
      track.style.transform =
        orientation === "vertical" ? `translateY(${offset}%)` : `translateX(${offset}%)`
      track.style.transition = "transform 200ms ease"
      const prev = rootEl.querySelector<HTMLElement>("[data-role='carousel-prev']")
      const nextButton = rootEl.querySelector<HTMLElement>("[data-role='carousel-next']")
      if (prev) prev.toggleAttribute("disabled", next === 0)
      if (nextButton) nextButton.toggleAttribute("disabled", next === items.length - 1)
    }

    const handleClick = (event: Event) => {
      const target = event.target as Element | null
      if (!target) return

      if (handleCopyClick(target)) {
        event.stopPropagation()
        return
      }

      if (target.closest("[data-ui-stop='true']")) {
        return
      }

      const collapsibleTrigger = target.closest("[data-slot='collapsible-trigger']") as HTMLElement | null
      if (collapsibleTrigger) {
        toggleCollapsible(collapsibleTrigger)
        return
      }

      const accordionTrigger = target.closest("[data-slot='accordion-trigger']") as HTMLElement | null
      if (accordionTrigger) {
        toggleAccordion(accordionTrigger)
        return
      }

      const tabsTrigger = target.closest("[data-slot='tabs-trigger']") as HTMLElement | null
      if (tabsTrigger) {
        selectTab(tabsTrigger)
        return
      }

      const selectTrigger = target.closest("[data-slot='select-trigger']") as HTMLElement | null
      if (selectTrigger) {
        toggleSelect(selectTrigger)
        return
      }

      const selectItem = target.closest("[data-slot='select-item']") as HTMLElement | null
      if (selectItem) {
        pickSelectItem(selectItem)
        return
      }

      const dropdownTrigger = target.closest("[data-slot='dropdown-menu-trigger']") as HTMLElement | null
      if (dropdownTrigger) {
        toggleDropdown(dropdownTrigger)
        return
      }

      const dropdownCheckbox = target.closest("[data-slot='dropdown-menu-checkbox-item']") as HTMLElement | null
      if (dropdownCheckbox) {
        toggleDropdownCheckbox(dropdownCheckbox)
        return
      }

      const dropdownRadio = target.closest("[data-slot='dropdown-menu-radio-item']") as HTMLElement | null
      if (dropdownRadio) {
        selectDropdownRadio(dropdownRadio)
        return
      }

      const dropdownSub = target.closest("[data-slot='dropdown-menu-sub-trigger']") as HTMLElement | null
      if (dropdownSub) {
        toggleDropdownSub(dropdownSub)
        return
      }

      const dropdownItem = target.closest("[data-slot='dropdown-menu-item']") as HTMLElement | null
      if (dropdownItem) {
        handleDropdownItem(dropdownItem)
        return
      }

      const popoverTrigger = target.closest("[data-slot='popover-trigger']") as HTMLElement | null
      if (popoverTrigger) {
        togglePopover(popoverTrigger)
        return
      }

      const dialogTrigger = target.closest("[data-slot='dialog-trigger']") as HTMLElement | null
      if (dialogTrigger) {
        toggleDialog(dialogTrigger, "open")
        return
      }

      const dialogClose = target.closest("[data-slot='dialog-close']") as HTMLElement | null
      if (dialogClose) {
        toggleDialog(dialogClose, "closed")
        return
      }

      const dialogOverlay = target.closest("[data-slot='dialog-overlay']") as HTMLElement | null
      if (dialogOverlay) {
        toggleDialog(dialogOverlay, "closed")
        return
      }

      const popoverContent = target.closest("[data-slot='popover-content']")
      if (popoverContent) {
        return
      }

      const switchEl = target.closest("[data-slot='switch']") as HTMLElement | null
      if (switchEl) {
        toggleSwitch(switchEl)
        const envToggle = switchEl.closest("[data-slot='environment-variables-toggle']")
        if (envToggle) {
          const envRoot = switchEl.closest("[data-slot='environment-variables']")
          if (envRoot) {
            const showValues = switchEl.getAttribute("data-state") === "checked"
            envRoot.setAttribute("data-show-values", showValues ? "true" : "false")
            updateEnvValues(envRoot, showValues)
          }
        }
        return
      }

      const carouselPrev = target.closest("[data-role='carousel-prev']") as HTMLElement | null
      if (carouselPrev) {
        handleCarouselNav(carouselPrev, -1)
        return
      }

      const carouselNext = target.closest("[data-role='carousel-next']") as HTMLElement | null
      if (carouselNext) {
        handleCarouselNav(carouselNext, 1)
        return
      }
    }

    const handleInput = (event: Event) => {
      const target = event.target as Element | null
      if (!target) return
      if (target.matches("[data-slot='command-input']") && target instanceof HTMLInputElement) {
        handleCommandInput(target)
      }
    }

    const handleMouseOver = (event: MouseEvent) => {
      const target = event.target as Element | null
      if (!target) return

      const tooltipTrigger = target.closest("[data-slot='tooltip-trigger']") as HTMLElement | null
      if (tooltipTrigger) {
        const rootEl = tooltipTrigger.closest("[data-slot='tooltip']")
        if (rootEl) {
          const existing = tooltipTimers.get(rootEl)
          if (existing) window.clearTimeout(existing)
          const provider = rootEl.closest("[data-slot='tooltip-provider']")
          const delayAttr = provider?.getAttribute("data-delay-duration") ?? ""
          const delay = Number.parseInt(delayAttr, 10)
          const resolvedDelay = Number.isFinite(delay) ? delay : 150
          const timer = window.setTimeout(
            () => setTooltipState(tooltipTrigger, "open"),
            resolvedDelay
          )
          tooltipTimers.set(rootEl, timer)
        }
        return
      }

      const hoverTrigger = target.closest("[data-slot='hover-card']") as HTMLElement | null
      if (hoverTrigger) {
        const trigger = hoverTrigger.querySelector<HTMLElement>("[data-slot='hover-card-trigger']")
        if (trigger) {
          setHoverCardState(trigger, "open")
        }
      }
    }

    const handleMouseOut = (event: MouseEvent) => {
      const target = event.target as Element | null
      if (!target) return
      const related = event.relatedTarget as Element | null

      const tooltipTrigger = target.closest("[data-slot='tooltip-trigger']") as HTMLElement | null
      if (tooltipTrigger) {
        const rootEl = tooltipTrigger.closest("[data-slot='tooltip']")
        if (rootEl && related && rootEl.contains(related)) {
          return
        }
        if (rootEl) {
          const existing = tooltipTimers.get(rootEl)
          if (existing) {
            window.clearTimeout(existing)
            tooltipTimers.delete(rootEl)
          }
        }
        setTooltipState(tooltipTrigger, "closed")
      }

      const hoverTrigger = target.closest("[data-slot='hover-card-trigger']") as HTMLElement | null
      if (hoverTrigger) {
        const rootEl = hoverTrigger.closest("[data-slot='hover-card']")
        if (rootEl && related && rootEl.contains(related)) {
          return
        }
        setHoverCardState(hoverTrigger, "closed")
      }

      const hoverContent = target.closest("[data-slot='hover-card-content']") as HTMLElement | null
      if (hoverContent) {
        const rootEl = hoverContent.closest("[data-slot='hover-card']")
        if (rootEl && related && rootEl.contains(related)) {
          return
        }
        const trigger = rootEl?.querySelector<HTMLElement>("[data-slot='hover-card-trigger']")
        if (trigger) {
          setHoverCardState(trigger, "closed")
        }
      }

      const tooltipContent = target.closest("[data-slot='tooltip-content']") as HTMLElement | null
      if (tooltipContent) {
        const rootEl = tooltipContent.closest("[data-slot='tooltip']")
        if (rootEl && related && rootEl.contains(related)) {
          return
        }
        const trigger = rootEl?.querySelector<HTMLElement>("[data-slot='tooltip-trigger']")
        if (trigger) {
          setTooltipState(trigger, "closed")
        }
      }
    }

    const handleKeydown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return
      const dialogs = toArray(root.querySelectorAll<HTMLElement>("[data-slot='dialog']"))
      dialogs.forEach((dialog) => {
        const overlay = dialog.querySelector<HTMLElement>("[data-slot='dialog-overlay']")
        const content = dialog.querySelector<HTMLElement>("[data-slot='dialog-content']")
        dialog.setAttribute("data-state", "closed")
        setState(overlay, "closed", true)
        setState(content, "closed", true)
      })
      const selects = toArray(root.querySelectorAll<HTMLElement>("[data-slot='select']"))
      selects.forEach((select) => {
        const content = select.querySelector<HTMLElement>("[data-slot='select-content']")
        setState(content, "closed", true)
        select.setAttribute("data-state", "closed")
      })
      const dropdowns = toArray(root.querySelectorAll<HTMLElement>("[data-slot='dropdown-menu']"))
      dropdowns.forEach((dropdown) => {
        const content = dropdown.querySelector<HTMLElement>("[data-slot='dropdown-menu-content']")
        setState(content, "closed", true)
        dropdown.setAttribute("data-state", "closed")
      })
      const popovers = toArray(root.querySelectorAll<HTMLElement>("[data-slot='popover']"))
      popovers.forEach((popover) => {
        const content = popover.querySelector<HTMLElement>("[data-slot='popover-content']")
        setState(content, "closed", true)
        popover.setAttribute("data-state", "closed")
      })
    }

    const handleDocumentClick = (event: MouseEvent) => {
      const target = event.target as Element | null
      if (!target) return
      const shouldClose = (selector: string) => {
        const roots = toArray(root.querySelectorAll<HTMLElement>(selector))
        roots.forEach((node) => {
          if (!node.contains(target)) {
            if (selector === "[data-slot='select']") {
              const content = node.querySelector<HTMLElement>("[data-slot='select-content']")
              setState(content, "closed", true)
              node.setAttribute("data-state", "closed")
            }
            if (selector === "[data-slot='dropdown-menu']") {
              const content = node.querySelector<HTMLElement>("[data-slot='dropdown-menu-content']")
              setState(content, "closed", true)
              node.setAttribute("data-state", "closed")
            }
            if (selector === "[data-slot='popover']") {
              const content = node.querySelector<HTMLElement>("[data-slot='popover-content']")
              setState(content, "closed", true)
              node.setAttribute("data-state", "closed")
            }
          }
        })
      }
      shouldClose("[data-slot='select']")
      shouldClose("[data-slot='dropdown-menu']")
      shouldClose("[data-slot='popover']")
    }

    root.addEventListener("click", handleClick)
    root.addEventListener("input", handleInput)
    root.addEventListener("mouseover", handleMouseOver)
    root.addEventListener("mouseout", handleMouseOut)
    document.addEventListener("click", handleDocumentClick)
    document.addEventListener("keydown", handleKeydown)

    syncOpenables(root, "[data-slot='select-content']")
    syncOpenables(root, "[data-slot='dropdown-menu-content']")
    syncOpenables(root, "[data-slot='dropdown-menu-sub-content']")
    syncOpenables(root, "[data-slot='popover-content']")
    syncOpenables(root, "[data-slot='tooltip-content']")
    syncOpenables(root, "[data-slot='hover-card-content']")
    syncOpenables(root, "[data-slot='dialog-overlay']")
    syncOpenables(root, "[data-slot='dialog-content']")
    syncCollapsibles(root)
    syncAccordions(root)
    syncTabs(root)
    syncSelects(root)
    syncCarousel(root)

    const envRoots = toArray(root.querySelectorAll<HTMLElement>("[data-slot='environment-variables']"))
    envRoots.forEach((envRoot) => {
      const showValues = envRoot.getAttribute("data-show-values") === "true"
      updateEnvValues(envRoot, showValues)
    })

    yield* Effect.addFinalizer(() =>
      Effect.sync(() => {
        root.removeEventListener("click", handleClick)
        root.removeEventListener("input", handleInput)
        root.removeEventListener("mouseover", handleMouseOver)
        root.removeEventListener("mouseout", handleMouseOut)
        document.removeEventListener("click", handleDocumentClick)
        document.removeEventListener("keydown", handleKeydown)
      })
    )
  })
