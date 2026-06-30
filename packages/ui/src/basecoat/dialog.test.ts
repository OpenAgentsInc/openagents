import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  Closed,
  Opened,
  RequestedClose,
  RequestedOpen,
  alertDialog,
  alertDialogView,
  close,
  descriptionId,
  dialog,
  dialogBody,
  dialogClose,
  dialogDescription,
  dialogFooter,
  dialogHeader,
  dialogSurface,
  dialogTitle,
  dialogTrigger,
  init,
  open,
  titleId,
  update,
  view,
} from './dialog'
import { renderHtml } from './test-helpers'

describe('basecoat dialog component', () => {
  test('updates open and close state with command and out-message evidence', () => {
    const model = init({
      id: 'profile-dialog',
      focusSelector: '#profile-name',
    })

    const [openedModel, openCommands, opened] = update(model, RequestedOpen())

    expect(openedModel.isOpen).toBe(true)
    expect(openCommands).toHaveLength(1)
    expect(openCommands[0]?.name).toBe('BasecoatShowDialog')
    expect(openCommands[0]?.args).toEqual({
      id: 'profile-dialog',
      focusSelector: '#profile-name',
    })
    expect(opened).toEqual(Opened())

    const [closedModel, closeCommands, closed] = update(
      openedModel,
      RequestedClose(),
    )

    expect(closedModel.isOpen).toBe(false)
    expect(closeCommands).toHaveLength(1)
    expect(closeCommands[0]?.name).toBe('BasecoatCloseDialog')
    expect(closeCommands[0]?.args).toEqual({ id: 'profile-dialog' })
    expect(closed).toEqual(Closed())
  })

  test('open and close helpers are no-ops for matching current state', () => {
    const closedModel = init({ id: 'confirm-dialog' })
    const [stillClosed, closeCommands, closeOut] = close(closedModel)

    expect(stillClosed).toBe(closedModel)
    expect(closeCommands).toEqual([])
    expect(closeOut).toBeNull()

    const [openedModel] = open(closedModel)
    const [stillOpen, openCommands, openOut] = open(openedModel)

    expect(stillOpen).toBe(openedModel)
    expect(openCommands).toEqual([])
    expect(openOut).toBeNull()
  })

  test('renders Basecoat dialog markup with title, description, body, footer, and close button', () => {
    const model = init({ id: 'edit-profile', isOpen: true })
    const rendered = renderHtml(
      view({
        model,
        surfaceClassName: 'sm:max-w-sm',
        title: ['Edit profile'],
        description: ['Make changes to your profile here.'],
        body: ['Profile fields'],
        footer: ['Actions'],
      }),
    )

    expect(rendered).toContain('<dialog')
    expect(rendered).toContain('id="edit-profile"')
    expect(rendered).toContain('class="dialog"')
    expect(rendered).toContain('open')
    expect(rendered).toContain('aria-modal="true"')
    expect(rendered).toContain('aria-labelledby="edit-profile-title"')
    expect(rendered).toContain('aria-describedby="edit-profile-description"')
    expect(rendered).toContain('data-close-on-overlay-click="true"')
    expect(rendered).toContain('<div class="sm:max-w-sm">')
    expect(rendered).toContain('<header')
    expect(rendered).toContain('<h2 id="edit-profile-title">Edit profile</h2>')
    expect(rendered).toContain(
      '<p id="edit-profile-description">Make changes to your profile here.</p>',
    )
    expect(rendered).toContain('<section')
    expect(rendered).toContain('Profile fields')
    expect(rendered).toContain('<footer')
    expect(rendered).toContain('Actions')
    expect(rendered).toContain('data-variant="ghost"')
    expect(rendered).toContain('data-size="icon-sm"')
    expect(rendered).toContain('aria-label="Close dialog"')
    expect(rendered).toContain('lucide lucide-x-icon lucide-x')
  })

  test('renders alert dialog variant without default escape or overlay close affordance', () => {
    const model = init({ id: 'delete-chat', isOpen: true })
    const rendered = renderHtml(
      alertDialogView({
        model,
        title: ['Delete this chat?'],
        description: ['This action cannot be undone.'],
        footer: ['Cancel Continue'],
      }),
    )

    expect(rendered).toContain('<dialog')
    expect(rendered).toContain('class="alert-dialog"')
    expect(rendered).toContain('aria-labelledby="delete-chat-title"')
    expect(rendered).toContain('aria-describedby="delete-chat-description"')
    expect(rendered).not.toContain('data-close-on-overlay-click')
    expect(rendered).not.toContain('aria-label="Close dialog"')
  })

  test('renders primitive slots independently', () => {
    const model = init({ id: 'share-dialog' })
    const rendered = renderHtml(
      dialog({
        model,
        labelledBy: titleId(model),
        describedBy: descriptionId(model),
        children: [
          dialogSurface({
            children: [
              dialogHeader({
                children: [
                  dialogTitle({ model, children: ['Share link'] }),
                  dialogDescription({
                    model,
                    children: ['Anyone with this link can view it.'],
                  }),
                ],
              }),
              dialogBody({ className: 'flex gap-2', children: ['Link input'] }),
              dialogFooter({ children: ['Close'] }),
              dialogClose({ ariaLabel: 'Dismiss share dialog' }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('class="dialog"')
    expect(rendered).toContain('aria-labelledby="share-dialog-title"')
    expect(rendered).toContain('aria-describedby="share-dialog-description"')
    expect(rendered).toContain('<h2 id="share-dialog-title">Share link</h2>')
    expect(rendered).toContain('class="flex gap-2"')
    expect(rendered).toContain('aria-label="Dismiss share dialog"')
  })

  test('renders trigger and raw alert dialog helpers', () => {
    const model = init({ id: 'raw-alert' })
    const trigger = renderHtml(
      dialogTrigger({
        className: 'inline-flex',
        children: ['Open dialog'],
      }),
    )
    const rawAlert = renderHtml(
      alertDialog({
        model,
        children: [dialogSurface({ children: ['Alert body'] })],
      }),
    )

    expect(trigger).toContain('<button')
    expect(trigger).toContain('type="button"')
    expect(trigger).toContain('class="btn inline-flex"')
    expect(trigger).toContain('Open dialog')
    expect(rawAlert).toContain('class="alert-dialog"')
    expect(rawAlert).toContain('Alert body')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.init).toBe(init)
    expect(Basecoat.update).toBe(update)
    expect(Basecoat.open).toBe(open)
    expect(Basecoat.close).toBe(close)
    expect(Basecoat.view).toBe(view)
    expect(Basecoat.alertDialogView).toBe(alertDialogView)
    expect(Basecoat.dialog).toBe(dialog)
    expect(Basecoat.alertDialog).toBe(alertDialog)
    expect(Basecoat.dialogTrigger).toBe(dialogTrigger)
    expect(Basecoat.dialogClose).toBe(dialogClose)
  })
})
