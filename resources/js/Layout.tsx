import React, { useCallback, useState } from 'react';
import { Frame, Navigation, TopBar } from "@shopify/polaris";
import {
  HomeIcon,
  PlusCircleIcon,
  ProductIcon,
  ExitIcon,
  AppExtensionIcon,
  AffiliateIcon,
  MinusCircleIcon,
  OrderIcon,
} from '@shopify/polaris-icons';
import { Head } from '@inertiajs/react';

export default function Layout({ children }) {
  const [isUserMenuOpen, setIsUserMenuOpen] = useState(false);

  const toggleIsUserMenuOpen = useCallback(
    () => setIsUserMenuOpen((isUserMenuOpen) => !isUserMenuOpen),
    [],
  );

  const userMenuMarkup = (
    <TopBar.UserMenu
      actions={[
        {
          items: [{
            content: 'Log out', icon: ExitIcon, onAction: () => {
              console.log("log out placeholder")
            }
          }],
        },
      ]}
      name="TestUser"
      initials="TU"
      avatar="https://i.pravatar.cc/150?img=14"
      open={isUserMenuOpen}
      onToggle={toggleIsUserMenuOpen}
    />
  );

  const topBar = (
    <TopBar
      showNavigationToggle
      userMenu={userMenuMarkup}
    />
  )

  const logo = {
    topBarSource:
      '/images/oarect.png',
    width: 146,
    url: '/',
    accessibilityLabel: 'OpenAgents',
  };

  return (
    <>
      <Head title="OpenAgents" />
      <Frame logo={logo} topBar={topBar} navigation={<NavigationIs />}>
        {children}
      </Frame>
    </>
  )
}

function NavigationIs() {
  return (
    <Navigation location="/dashboard">
      <Navigation.Section
        items={[
          {
            url: '/dashboard',
            label: 'Dashboard',
            icon: HomeIcon,
            selected: true,
          },
          {
            url: '/agents',
            label: 'Agents',
            icon: AffiliateIcon,
            badge: '2',
          },
          {
            url: '/plugins',
            label: 'Plugins',
            icon: AppExtensionIcon,
            badge: '4',
          },
          // {
          //   url: '#',
          //   excludePaths: ['#'],
          //   label: 'Products',
          //   icon: ProductIcon,
          //   secondaryActions: [
          //     {
          //       url: '#',
          //       accessibilityLabel: 'Add a product',
          //       icon: PlusCircleIcon,
          //       tooltip: {
          //         content: 'Add a product',
          //       },
          //     },
          //     {
          //       accessibilityLabel: 'Remove a product',
          //       icon: MinusCircleIcon,
          //       onClick: () => { },
          //       tooltip: {
          //         content: 'Remove a product',
          //       },
          //     },
          //   ],
          // },
        ]}
      />
    </Navigation>
  )
}
