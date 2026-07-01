import { describe, expect, test } from 'bun:test'
import { Array, Effect, Option, Schema as S } from 'effect'

import {
  IconCount,
  IconName,
  type IconName as IconNameType,
  IconService,
  Icons,
  iconSvg,
} from '../src/icon'

const requiredChromeIcons: ReadonlyArray<IconNameType> = [
  'Archive',
  'ArrowLeft',
  'ArrowRotateCcw',
  'ArrowUp',
  'BranchAlt',
  'ChatCompose',
  'ChevronRight',
  'Collapse',
  'Code',
  'Dumbbell',
  'Expand',
  'ExpandSm',
  'Eye',
  'EyeOff',
  'File',
  'FileImage',
  'Folder',
  'NotificationBell',
  'Paperclip',
  'Pencil',
  'Plus',
  'Reload',
  'Robot',
  'Settings',
  'Stop',
  'Text',
  'Trash',
  'Unarchive',
]

describe('icon catalog', () => {
  test('narrows icon names at compile time', () => {
    const acceptedIconName: IconNameType = 'Folder'
    // @ts-expect-error Non-catalog icon names must not typecheck.
    const rejectedIconName: IconNameType = 'NotARealIcon'

    expect(acceptedIconName).toBe('Folder')
    expect(rejectedIconName as string).toBe('NotARealIcon')
  })

  test('contains the full Fireball Apps SDK icon set', () => {
    expect(Icons).toHaveLength(IconCount)
    expect(IconCount).toBe(755)
    expect(new Set(Array.map(Icons, icon => icon.name)).size).toBe(IconCount)
  })

  test('uses Effect schema to allow only catalog icon names', () => {
    const decodeIconName = S.decodeUnknownOption(IconName)

    Array.forEach(Icons, icon => {
      expect(Option.isSome(decodeIconName(icon.name))).toBe(true)
    })
    expect(Option.isNone(decodeIconName('NotARealIcon'))).toBe(true)
  })

  test('includes icons used by the app chrome', () => {
    Array.forEach(requiredChromeIcons, name => {
      expect(iconSvg(name)).toContain('<svg')
    })
  })

  test('exposes the catalog through an Effect service', async () => {
    const svg = await Effect.runPromise(
      Effect.gen(function* () {
        const icons = yield* IconService

        return yield* icons.svg('Folder')
      }).pipe(Effect.provide(IconService.layer)),
    )

    expect(svg).toContain('<svg')
  })
})
