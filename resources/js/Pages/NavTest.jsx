import React from 'react'
import {Frame, Navigation} from '@shopify/polaris';
import {
  HomeIcon,
  PlusCircleIcon,
  ProductIcon,
  MinusCircleIcon,
  OrderIcon,
} from '@shopify/polaris-icons';

export default function NavTest({ user }) {
  return (
    <Frame>
      <Navigation location="/">
        <Navigation.Section
          items={[
            {
              url: '#',
              excludePaths: ['#'],
              label: 'Home',
              icon: HomeIcon,
              selected: false,
            },
            {
              url: '#',
              excludePaths: ['#'],
              label: 'Orders',
              icon: OrderIcon,
              badge: '2',
              secondaryActions: [
                {
                  url: '#',
                  accessibilityLabel: 'Add a product',
                  icon: PlusCircleIcon,
                  tooltip: {
                    content: 'Create new order',
                  },
                },
              ],
            },
            {
              url: '#',
              excludePaths: ['#'],
              label: 'Products',
              icon: ProductIcon,
              secondaryActions: [
                {
                  url: '#',
                  accessibilityLabel: 'Add a product',
                  icon: PlusCircleIcon,
                  tooltip: {
                    content: 'Add a product',
                  },
                },
                {
                  accessibilityLabel: 'Remove a product',
                  icon: MinusCircleIcon,
                  onClick: () => {},
                  tooltip: {
                    content: 'Remove a product',
                  },
                },
              ],
            },
          ]}
        />
      </Navigation>
    </Frame>
  )
}
