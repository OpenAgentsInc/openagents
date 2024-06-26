import React, { useCallback, useState } from 'react';
import { Frame, Navigation, TopBar } from "@shopify/polaris";
import {
  HomeIcon,
  ExitIcon,
  AppExtensionIcon,
  AffiliateIcon,
  GaugeIcon,
  PlusCircleIcon,
  ChatIcon
} from '@shopify/polaris-icons';
import { Head, usePage } from '@inertiajs/react';

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
  const { url } = usePage()
  return (
    <Navigation location={url}>
      <Navigation.Section
        items={[
          {
            url: '/',
            label: 'Home',
            icon: HomeIcon,
            selected: url === '/',
          },
          {
            url: '/dashboard',
            label: 'Dashboard',
            icon: GaugeIcon,
            selected: url === '/dashboard',
          },
          // {
          //   url: '/agents',
          //   label: 'Agents',
          //   icon: AffiliateIcon,
          //   badge: '2',
          // },
          {
            url: '/plugin-map',
            label: 'Plugin Map',
            icon: AppExtensionIcon,
            selected: url === '/plugin-map'
            // badge: '4',
          },
          {
            url: '/scratchpad',
            label: 'Scratchpad',
            icon: AppExtensionIcon,
            selected: url === '/scratchpad'
          }
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
      {/* <Navigation.Section
        title="Chats"
        items={[
          {
            url: '#',
            excludePaths: ['#'],
            label: 'How does this work',
            icon: ChatIcon,
          },
        ]}
        action={{
          accessibilityLabel: 'Add sales channel',
          icon: PlusCircleIcon,
          onClick: () => { },
        }}
      /> */}
    </Navigation>
  )
}
