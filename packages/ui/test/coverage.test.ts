import { describe, expect, it } from 'bun:test'
import { existsSync, readdirSync, statSync } from 'node:fs'
import { homedir } from 'node:os'
import { join, relative } from 'node:path'

import {
  applicationUiV4Families,
  ecommerceUiV4Families,
  marketingUiV4Families,
} from '../src/index'

const applicationFamiliesFromDownloads = [
  'application-shells/multi-column',
  'application-shells/sidebar',
  'application-shells/stacked',
  'data-display/calendars',
  'data-display/description-lists',
  'data-display/stats',
  'elements/avatars',
  'elements/badges',
  'elements/button-groups',
  'elements/buttons',
  'elements/dropdowns',
  'feedback/alerts',
  'feedback/empty-states',
  'forms/action-panels',
  'forms/checkboxes',
  'forms/comboboxes',
  'forms/form-layouts',
  'forms/input-groups',
  'forms/radio-groups',
  'forms/select-menus',
  'forms/sign-in-forms',
  'forms/textareas',
  'forms/toggles',
  'headings/card-headings',
  'headings/page-headings',
  'headings/section-headings',
  'layout/cards',
  'layout/containers',
  'layout/dividers',
  'layout/list-containers',
  'layout/media-objects',
  'lists/feeds',
  'lists/grid-lists',
  'lists/stacked-lists',
  'lists/tables',
  'navigation/breadcrumbs',
  'navigation/command-palettes',
  'navigation/navbars',
  'navigation/pagination',
  'navigation/progress-bars',
  'navigation/sidebar-navigation',
  'navigation/tabs',
  'navigation/vertical-navigation',
  'overlays/drawers',
  'overlays/modal-dialogs',
  'overlays/notifications',
  'page-examples/detail-screens',
  'page-examples/home-screens',
  'page-examples/settings-screens',
] as const

const ecommerceFamiliesFromDownloads = [
  'components/category-filters',
  'components/category-previews',
  'components/checkout-forms',
  'components/incentives',
  'components/order-history',
  'components/order-summaries',
  'components/product-features',
  'components/product-lists',
  'components/product-overviews',
  'components/product-quickviews',
  'components/promo-sections',
  'components/reviews',
  'components/shopping-carts',
  'components/store-navigation',
  'page-examples/category-pages',
  'page-examples/checkout-pages',
  'page-examples/order-detail-pages',
  'page-examples/order-history-pages',
  'page-examples/product-pages',
  'page-examples/shopping-cart-pages',
  'page-examples/storefront-pages',
] as const

const marketingFamiliesFromDownloads = [
  'elements/banners',
  'elements/flyout-menus',
  'elements/headers',
  'feedback/404-pages',
  'page-examples/about-pages',
  'page-examples/landing-pages',
  'page-examples/pricing-pages',
  'sections/bento-grids',
  'sections/blog-sections',
  'sections/contact-sections',
  'sections/content-sections',
  'sections/cta-sections',
  'sections/faq-sections',
  'sections/feature-sections',
  'sections/footers',
  'sections/header',
  'sections/heroes',
  'sections/logo-clouds',
  'sections/newsletter-sections',
  'sections/pricing',
  'sections/stats-sections',
  'sections/team-sections',
  'sections/testimonials',
] as const

const sorted = (values: ReadonlyArray<string>) => [...values].sort()

const htmlFiles = (root: string): ReadonlyArray<string> => {
  if (!existsSync(root)) {
    return []
  }

  const visit = (dir: string): ReadonlyArray<string> =>
    readdirSync(dir).flatMap(entry => {
      const path = join(dir, entry)
      const stat = statSync(path)

      if (stat.isDirectory()) {
        return visit(path)
      }

      return path.endsWith('.html') ? [path] : []
    })

  return visit(root)
}

const variantFamily = (root: string, file: string): string => {
  const parts = relative(root, file).split('/')

  return `${parts[0]}/${parts[1]}`
}

const downloadsRoot = join(homedir(), 'Downloads')

const localKits = [
  {
    expectedCount: 364,
    families: applicationUiV4Families,
    name: 'application-ui-v4',
    root: join(downloadsRoot, 'application-ui-v4/html'),
  },
  {
    expectedCount: 114,
    families: ecommerceUiV4Families,
    name: 'ecommerce-v4',
    root: join(downloadsRoot, 'ecommerce-v4/html'),
  },
  {
    expectedCount: 179,
    families: marketingUiV4Families,
    name: 'marketing-v4',
    root: join(downloadsRoot, 'marketing-v4/html'),
  },
] as const

describe('Tailwind UI v4 family coverage', () => {
  it('tracks every Application UI family from ~/Downloads/application-ui-v4/html', () => {
    expect(sorted(applicationUiV4Families)).toEqual(
      sorted(applicationFamiliesFromDownloads),
    )
  })

  it('tracks every Ecommerce UI family from ~/Downloads/ecommerce-v4/html', () => {
    expect(sorted(ecommerceUiV4Families)).toEqual(
      sorted(ecommerceFamiliesFromDownloads),
    )
  })

  it('tracks every Marketing UI family from ~/Downloads/marketing-v4/html', () => {
    expect(sorted(marketingUiV4Families)).toEqual(
      sorted(marketingFamiliesFromDownloads),
    )
  })

  it('maps every local Tailwind UI HTML variant file to a registered family when downloads are present', () => {
    const presentKits = localKits.filter(kit => existsSync(kit.root))

    // The per-file coverage check reads the proprietary Tailwind UI v4 HTML
    // exports from ~/Downloads, which are never committed to the repo. On a
    // clean checkout (CI, fresh clone, any machine without the local kits)
    // none are present and there is nothing to verify, so skip the body
    // rather than fail. When ANY kit is present we still require that the
    // full set is present and run the complete per-file coverage assertions —
    // a partial download is a real coverage gap and stays caught.
    if (presentKits.length === 0) {
      return
    }

    expect(presentKits.map(kit => kit.name)).toEqual([
      'application-ui-v4',
      'ecommerce-v4',
      'marketing-v4',
    ])

    for (const kit of presentKits) {
      const files = htmlFiles(kit.root)
      const families = new Set<string>(kit.families)
      const unregisteredFamilies = files
        .map(file => variantFamily(kit.root, file))
        .filter(family => !families.has(family))

      expect(files).toHaveLength(kit.expectedCount)
      expect(unregisteredFamilies).toEqual([])
    }
  })
})
