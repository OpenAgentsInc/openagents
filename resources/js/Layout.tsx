import React, { useCallback, useState } from 'react';
import { Frame, TopBar } from "@shopify/polaris";
import { ArrowLeftIcon } from '@shopify/polaris-icons';

export default function Layout({ children }) {
  const [isUserMenuOpen, setIsUserMenuOpen] = useState(false);

  const toggleIsUserMenuOpen = useCallback(
    () => setIsUserMenuOpen((isUserMenuOpen) => !isUserMenuOpen),
    [],
  );

  const userMenuMarkup = (
    <TopBar.UserMenu
      actions={[
        // {
        //   items: [{ content: 'Back to OpenAgents', icon: ArrowLeftIcon }],
        // },
        // {
        //   items: [{ content: 'Community forums' }],
        // },
      ]}
      name="TestUser"
      // detail="Agent Builder"
      initials="T"
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
    <Frame logo={logo} topBar={topBar}>
      {children}
    </Frame>
  )
}
