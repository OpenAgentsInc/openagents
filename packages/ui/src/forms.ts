import { clsx } from 'clsx'
import * as stylex from '@stylexjs/stylex'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { eyebrowClass, kitFamily, metaClass, titleClass } from './primitives'
import type { FormOption, ValidationState } from './primitives'
import {
  stylexAttrs,
  stylexFallback,
  stylexRuntimeFallbackEnabled,
} from './stylex-foldkit'

const formStyles = stylexRuntimeFallbackEnabled()
  ? {
      compactButton: stylexFallback('oa-ui-form-compact-button'),
      compactButtonStrong: stylexFallback('oa-ui-form-compact-button-strong'),
      group: stylexFallback('oa-ui-form-group'),
      validationGroup: stylexFallback('oa-ui-form-validation-group'),
      validationHeader: stylexFallback('oa-ui-form-validation-header'),
      validationLabel: stylexFallback('oa-ui-form-validation-label'),
      input: stylexFallback('oa-ui-form-input'),
      textarea: stylexFallback('oa-ui-form-textarea'),
      indicatorInfo: stylexFallback('oa-ui-form-indicator-info'),
      indicatorSuccess: stylexFallback('oa-ui-form-indicator-success'),
      error: stylexFallback('oa-ui-form-error'),
    }
  : stylex.create({
      compactButton: {
        minHeight: 32,
        cursor: 'pointer',
        borderWidth: 1,
        borderStyle: 'solid',
        borderColor: '#333',
        backgroundColor: 'transparent',
        paddingInline: 10,
        fontFamily: 'inherit',
        fontSize: '0.8125rem',
        color: 'rgba(255,255,255,0.6)',
        ':hover': {
          backgroundColor: '#080808',
          color: '#f1efe8',
        },
      },
      compactButtonStrong: {
        borderColor: '#f1efe8',
        backgroundColor: '#f1efe8',
        color: '#000',
        ':hover': {
          backgroundColor: '#f1efe8',
          color: '#000',
        },
      },
      group: {
        display: 'grid',
        gap: 6,
      },
      validationGroup: {
        display: 'grid',
        gap: 4,
      },
      validationHeader: {
        display: 'flex',
        alignItems: 'center',
        gap: 8,
      },
      validationLabel: {
        display: 'block',
        fontSize: '0.875rem',
        fontWeight: 500,
        color: 'rgba(255,255,255,0.6)',
      },
      input: {
        width: '100%',
        minWidth: 0,
        borderWidth: 1,
        borderStyle: 'solid',
        borderColor: '#222',
        backgroundColor: '#030303',
        paddingInline: 12,
        paddingBlock: 10,
        fontFamily:
          'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
        fontSize: '0.8125rem',
        lineHeight: 1.35,
        color: '#f1efe8',
        outlineStyle: 'none',
        '::placeholder': {
          color: 'rgba(255,255,255,0.3)',
        },
        ':focus': {
          borderColor: '#ffb400',
          boxShadow: '0 0 0 1px #ffb400',
        },
      },
      textarea: {
        minHeight: 128,
        resize: 'vertical',
      },
      indicatorInfo: {
        fontSize: '0.875rem',
        color: '#2979ff',
      },
      indicatorSuccess: {
        fontSize: '0.875rem',
        color: '#00c853',
      },
      error: {
        fontSize: '0.875rem',
        color: '#d32f2f',
      },
    })

export const compactButton = <Message>(input: {
  label: string
  variant?: 'ghost' | 'strong'
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...(input.attrs ?? []),
      h.Type('button'),
      ...stylexAttrs<Message>(
        formStyles.compactButton,
        input.variant === 'strong' ? formStyles.compactButtonStrong : null,
      ),
    ],
    [input.label],
  )
}

export const inputClass =
  'w-full min-w-0 border border-[#222] bg-[#030303] px-3 py-2.5 font-mono text-[0.8125rem] leading-[1.35] text-[#f1efe8] outline-none focus:border-[#ffb400] focus:ring-1 focus:ring-[#ffb400]'

export const textareaClass = clsx(inputClass, 'min-h-32 resize-y')

export const selectClass = inputClass

export const inputGroup = <Message>(input: {
  id: string
  name: string
  label: string
  type?: string
  value?: string
  placeholder?: string
  help?: string
  className?: string
  labelClassName?: string
  helpClassName?: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.label(
    [
      kitFamily<Message>('forms/input-groups'),
      h.For(input.id),
      ...stylexAttrs<Message>(formStyles.group),
    ],
    [
      h.span([h.Class(clsx(eyebrowClass, input.labelClassName))], [
        input.label,
      ]),
      h.input([
        ...(input.attrs ?? []),
        h.Id(input.id),
        h.Name(input.name),
        h.Type(input.type ?? 'text'),
        ...(input.value === undefined ? [] : [h.Value(input.value)]),
        ...(input.placeholder === undefined
          ? []
          : [h.Placeholder(input.placeholder)]),
        ...stylexAttrs<Message>(formStyles.input),
        ...(input.className === undefined ? [] : [h.Class(input.className)]),
      ]),
      input.help === undefined
        ? null
        : h.span([h.Class(clsx(metaClass, input.helpClassName))], [
            input.help,
          ]),
    ],
  )
}

const validationBorderClass = (state: ValidationState): string =>
  clsx({
    'border-[#222]': state === 'idle',
    'border-[#2979ff]': state === 'validating',
    'border-[#00c853]': state === 'valid',
    'border-[#d32f2f]': state === 'invalid',
  })

const validationIndicator = <Message>(state: ValidationState): Html => {
  const h = html<Message>()

  if (state === 'validating') {
    return h.span(stylexAttrs<Message>(formStyles.indicatorInfo), ['...'])
  }

  if (state === 'valid') {
    return h.span(stylexAttrs<Message>(formStyles.indicatorSuccess), ['✓'])
  }

  return h.empty
}

export const validatedInputGroup = <Message>(input: {
  id: string
  name: string
  label: string
  state: ValidationState
  type?: string
  value?: string
  placeholder?: string
  error?: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('forms/input-groups'),
      ...stylexAttrs<Message>(formStyles.validationGroup),
    ],
    [
      h.div(
        stylexAttrs<Message>(formStyles.validationHeader),
        [
          h.label(
            [
              h.For(input.id),
              ...stylexAttrs<Message>(formStyles.validationLabel),
            ],
            [input.label],
          ),
          validationIndicator<Message>(input.state),
        ],
      ),
      inputGroup<Message>({
        id: input.id,
        name: input.name,
        label: input.label,
        ...(input.type === undefined ? {} : { type: input.type }),
        ...(input.value === undefined ? {} : { value: input.value }),
        ...(input.placeholder === undefined
          ? {}
          : { placeholder: input.placeholder }),
        className: validationBorderClass(input.state),
        attrs: [h.AriaLabel(input.label), ...(input.attrs ?? [])],
      }),
      input.error === undefined
        ? null
        : h.div(stylexAttrs<Message>(formStyles.error), [input.error]),
    ],
  )
}

export const textareaGroup = <Message>(input: {
  id: string
  name: string
  label: string
  value?: string
  placeholder?: string
  rows?: number
  className?: string
  labelClassName?: string
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.label(
    [
      kitFamily<Message>('forms/textareas'),
      h.For(input.id),
      ...stylexAttrs<Message>(formStyles.group),
    ],
    [
      h.span([h.Class(clsx(eyebrowClass, input.labelClassName))], [
        input.label,
      ]),
      h.textarea(
        [
          ...(input.attrs ?? []),
          h.Id(input.id),
          h.Name(input.name),
          ...(input.placeholder === undefined
            ? []
            : [h.Placeholder(input.placeholder)]),
          ...(input.rows === undefined ? [] : [h.Rows(input.rows)]),
          ...stylexAttrs<Message>(formStyles.input, formStyles.textarea),
          ...(input.className === undefined ? [] : [h.Class(input.className)]),
        ],
        [input.value ?? ''],
      ),
    ],
  )
}

export const selectMenu = <Message>(input: {
  id: string
  name: string
  label: string
  options: ReadonlyArray<FormOption>
}): Html => {
  const h = html<Message>()

  return h.label(
    [
      kitFamily<Message>('forms/select-menus'),
      h.For(input.id),
      ...stylexAttrs<Message>(formStyles.group),
    ],
    [
      h.span([h.Class(eyebrowClass)], [input.label]),
      h.select(
        [
          h.Id(input.id),
          h.Name(input.name),
          ...stylexAttrs<Message>(formStyles.input),
        ],
        input.options.map(option =>
          h.option(
            [
              h.Value(option.value),
              ...(option.checked === true ? [h.Selected(true)] : []),
              ...(option.disabled === true ? [h.Disabled(true)] : []),
            ],
            [option.label],
          ),
        ),
      ),
    ],
  )
}

export const checkboxList = <Message>(input: {
  name: string
  legend: string
  options: ReadonlyArray<FormOption>
}): Html => {
  const h = html<Message>()

  return h.fieldset(
    [
      kitFamily<Message>('forms/checkboxes'),
      h.Class('m-0 grid gap-3 border-0 p-0'),
    ],
    [
      h.legend([h.Class(eyebrowClass)], [input.legend]),
      ...input.options.map((option, index) => {
        const id = `${input.name}-${index}`

        return h.label(
          [
            h.For(id),
            h.Class('grid grid-cols-[auto_minmax(0,1fr)] gap-3 text-sm'),
          ],
          [
            h.span(
              [h.Class('group inline-grid h-lh items-center text-sm')],
              [
                h.input([
                  h.Id(id),
                  h.Name(input.name),
                  h.Type('checkbox'),
                  h.Value(option.value),
                  ...(option.checked === true ? [h.Checked(true)] : []),
                  ...(option.disabled === true ? [h.Disabled(true)] : []),
                  h.Class(
                    'col-start-1 row-start-1 size-5 appearance-none border border-[#333] bg-[#000] checked:border-[#ffb400] checked:bg-[#ffb400] focus-visible:outline-2 focus-visible:outline-[#ffb400] disabled:border-white/10 disabled:bg-white/5 sm:size-4',
                  ),
                ]),
              ],
            ),
            h.span(
              [h.Class('grid gap-0.5')],
              [
                h.span([h.Class('text-white/70')], [option.label]),
                option.detail === undefined
                  ? null
                  : h.span([h.Class(metaClass)], [option.detail]),
              ],
            ),
          ],
        )
      }),
    ],
  )
}

export const radioGroup = <Message>(input: {
  name: string
  legend: string
  options: ReadonlyArray<FormOption>
}): Html => {
  const h = html<Message>()

  return h.fieldset(
    [
      kitFamily<Message>('forms/radio-groups'),
      h.Class('m-0 grid gap-3 border-0 p-0'),
    ],
    [
      h.legend([h.Class(eyebrowClass)], [input.legend]),
      ...input.options.map((option, index) => {
        const id = `${input.name}-${index}`

        return h.label(
          [
            h.For(id),
            h.Class('grid grid-cols-[auto_minmax(0,1fr)] gap-3 text-sm'),
          ],
          [
            h.span(
              [h.Class('group inline-grid h-lh items-center text-sm')],
              [
                h.input([
                  h.Id(id),
                  h.Name(input.name),
                  h.Type('radio'),
                  h.Value(option.value),
                  ...(option.checked === true ? [h.Checked(true)] : []),
                  ...(option.disabled === true ? [h.Disabled(true)] : []),
                  h.Class(
                    'col-start-1 row-start-1 size-5 appearance-none rounded-full border border-[#333] bg-[#000] checked:border-[#ffb400] checked:bg-[#ffb400] focus-visible:outline-2 focus-visible:outline-[#ffb400] disabled:border-white/10 disabled:bg-white/5 sm:size-4',
                  ),
                ]),
              ],
            ),
            h.span(
              [h.Class('grid gap-0.5')],
              [
                h.span([h.Class('text-white/70')], [option.label]),
                option.detail === undefined
                  ? null
                  : h.span([h.Class(metaClass)], [option.detail]),
              ],
            ),
          ],
        )
      }),
    ],
  )
}

export const toggleRow = <Message>(input: {
  id: string
  name: string
  label: string
  detail?: string
  checked?: boolean
}): Html => {
  const h = html<Message>()

  return h.label(
    [
      kitFamily<Message>('forms/toggles'),
      h.For(input.id),
      h.Class(
        'grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 border border-[#222] bg-[#010102] p-3',
      ),
    ],
    [
      h.span(
        [h.Class('grid gap-1')],
        [
          h.span([h.Class(titleClass)], [input.label]),
          input.detail === undefined
            ? null
            : h.span([h.Class(metaClass)], [input.detail]),
        ],
      ),
      h.span(
        [
          h.Class(
            'group relative inline-flex w-11 shrink-0 border border-[#333] bg-[#080808] p-0.5 has-checked:bg-[#ffb400] sm:w-9',
          ),
        ],
        [
          h.span(
            [
              h.Class(
                'aspect-square w-1/2 bg-white transition-transform group-has-checked:translate-x-full',
              ),
            ],
            [],
          ),
          h.input([
            h.Id(input.id),
            h.Name(input.name),
            h.Type('checkbox'),
            ...(input.checked === true ? [h.Checked(true)] : []),
            h.Class(
              'absolute inset-0 size-full appearance-none focus:outline-none',
            ),
          ]),
        ],
      ),
    ],
  )
}

export const comboboxList = <Message>(input: {
  id: string
  name: string
  label: string
  options: ReadonlyArray<FormOption>
}): Html => {
  const h = html<Message>()

  return h.div(
    [kitFamily<Message>('forms/comboboxes'), h.Class('grid gap-1.5')],
    [
      h.label([h.For(input.id), h.Class(eyebrowClass)], [input.label]),
      h.input([
        h.Id(input.id),
        h.Name(input.name),
        h.Type('text'),
        h.Role('combobox'),
        h.AriaExpanded(false),
        h.Class(inputClass),
      ]),
      h.ul(
        [
          h.Role('listbox'),
          h.Class('m-0 grid list-none border border-[#222] bg-[#010102] p-0'),
        ],
        input.options.map(option =>
          h.li(
            [
              h.Role('option'),
              h.Class(
                'grid gap-0.5 border-b border-[#222] px-3 py-2 last:border-b-0',
              ),
            ],
            [
              h.span([h.Class(titleClass)], [option.label]),
              option.detail === undefined
                ? null
                : h.span([h.Class(metaClass)], [option.detail]),
            ],
          ),
        ),
      ),
    ],
  )
}
