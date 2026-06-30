import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  initToastModel,
  toastDefaultDuration,
  toaster,
  updateToast,
  type ToastItem,
  type ToastMessage,
} from './toast'
import { renderHtml } from './test-helpers'

const message = (input: ToastMessage): ToastMessage => input

const toasts: ReadonlyArray<ToastItem> = [
  {
    id: 'toast-success',
    category: 'success',
    title: ['Success'],
    description: ['A success toast called from Foldkit.'],
    action: { label: 'View', href: '/agents' },
    cancel: { label: 'Dismiss' },
  },
  {
    id: 'toast-error',
    category: 'error',
    title: ['Error'],
    description: ['Something needs attention.'],
  },
]

describe('basecoat toast component', () => {
  test('renders Basecoat toaster and toast markup', () => {
    const model = initToastModel({
      toasts,
      focus: { toastId: 'toast-success', control: 'action' },
    })

    const rendered = renderHtml(
      toaster({
        model,
        align: 'center',
        toMessage: message,
      }),
    )

    expect(rendered).toContain('id="toaster"')
    expect(rendered).toContain('class="toaster"')
    expect(rendered).toContain('data-align="center"')
    expect(rendered).toContain('class="toast"')
    expect(rendered).toContain('class="toast-content"')
    expect(rendered).toContain('role="status"')
    expect(rendered).toContain('aria-atomic="true"')
    expect(rendered).toContain('aria-hidden="false"')
    expect(rendered).toContain('data-toast-id="toast-success"')
    expect(rendered).toContain('data-category="success"')
    expect(rendered).toContain('<h2>Success</h2>')
    expect(rendered).toContain('<p>A success toast called from Foldkit.</p>')
    expect(rendered).toContain('href="/agents"')
    expect(rendered).toContain('data-toast-action=""')
    expect(rendered).toContain('data-toast-cancel=""')
    expect(rendered).toContain('class="btn"')
  })

  test('initializes toasts open and applies Basecoat default durations', () => {
    const model = initToastModel({ toasts })

    expect(model.toasts.map(toast => toast.open)).toEqual([true, true])
    expect(toastDefaultDuration(model.toasts[0] as ToastItem)).toBe(3000)
    expect(toastDefaultDuration(model.toasts[1] as ToastItem)).toBe(5000)
    expect(toastDefaultDuration({ id: 'sticky', duration: -1 })).toBe(-1)
  })

  test('opens, closes, removes, and closes all toasts', () => {
    const model = initToastModel({ toasts: [toasts[0] as ToastItem] })
    const opened = updateToast(model, {
      _tag: 'ToastOpened',
      toast: toasts[1] as ToastItem,
    })

    expect(opened.toasts.map(toast => toast.id)).toEqual([
      'toast-success',
      'toast-error',
    ])

    const closed = updateToast(opened, {
      _tag: 'ToastClosed',
      toastId: 'toast-success',
    })
    expect(closed.toasts[0]?.open).toBe(false)

    const removed = updateToast(closed, {
      _tag: 'ToastRemoved',
      toastId: 'toast-success',
    })
    expect(removed.toasts.map(toast => toast.id)).toEqual(['toast-error'])

    const closedAll = updateToast(opened, { _tag: 'ToastClosedAll' })
    expect(closedAll.toasts.every(toast => toast.open === false)).toBe(true)
  })

  test('pauses and resumes the toaster without mutating toasts', () => {
    const model = initToastModel({ toasts })

    const paused = updateToast(model, { _tag: 'ToastPaused' })
    expect(paused.paused).toBe(true)
    expect(paused.toasts).toEqual(model.toasts)

    const resumed = updateToast(paused, { _tag: 'ToastResumed' })
    expect(resumed.paused).toBe(false)
  })

  test('tracks focus and selects footer controls by closing the toast', () => {
    const model = initToastModel({ toasts })

    const focused = updateToast(model, {
      _tag: 'ToastFocused',
      toastId: 'toast-success',
      control: 'cancel',
    })
    expect(focused.focus).toEqual({
      toastId: 'toast-success',
      control: 'cancel',
    })

    const selected = updateToast(focused, {
      _tag: 'ToastSelected',
      toastId: 'toast-success',
      control: 'cancel',
    })
    expect(selected.selection).toEqual({
      toastId: 'toast-success',
      control: 'cancel',
    })
    expect(selected.toasts[0]?.open).toBe(false)
    expect(selected.focus).toBeNull()
  })

  test('moves focus with keyboard navigation and selects with Enter', () => {
    const model = initToastModel({ toasts })

    const first = updateToast(model, {
      _tag: 'ToastKeyDown',
      key: 'ArrowDown',
    })
    expect(first.focus).toEqual({
      toastId: 'toast-success',
      control: 'action',
    })

    const next = updateToast(first, {
      _tag: 'ToastKeyDown',
      key: 'ArrowRight',
    })
    expect(next.focus).toEqual({
      toastId: 'toast-success',
      control: 'cancel',
    })

    const end = updateToast(next, { _tag: 'ToastKeyDown', key: 'End' })
    expect(end.focus).toEqual({
      toastId: 'toast-error',
      control: null,
    })

    const home = updateToast(end, { _tag: 'ToastKeyDown', key: 'Home' })
    expect(home.focus).toEqual({
      toastId: 'toast-success',
      control: 'action',
    })

    const selected = updateToast(home, { _tag: 'ToastKeyDown', key: 'Enter' })
    expect(selected.selection).toEqual({
      toastId: 'toast-success',
      control: 'action',
    })
    expect(selected.toasts[0]?.open).toBe(false)
  })

  test('closes the focused toast with Escape', () => {
    const model = initToastModel({
      toasts,
      focus: { toastId: 'toast-success', control: 'action' },
    })

    const closed = updateToast(model, { _tag: 'ToastKeyDown', key: 'Escape' })

    expect(closed.toasts[0]?.open).toBe(false)
    expect(closed.focus).toBeNull()
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.initToastModel).toBe(initToastModel)
    expect(Basecoat.updateToast).toBe(updateToast)
    expect(Basecoat.toaster).toBe(toaster)
  })
})
