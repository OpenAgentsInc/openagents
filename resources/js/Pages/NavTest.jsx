import React, { useCallback, useState } from 'react'
import { Frame, Navigation, TopBar, ActionList, Icon, Text } from '@shopify/polaris';
import {
  HomeIcon,
  PlusCircleIcon,
  ProductIcon,
  MinusCircleIcon,
  OrderIcon,
  ArrowLeftIcon, QuestionCircleIcon
} from '@shopify/polaris-icons';

export default function NavTest({ user }) {
  return <TopBarExample />
}

function TopBarExample() {
  const [isUserMenuOpen, setIsUserMenuOpen] = useState(false);
  const [isSecondaryMenuOpen, setIsSecondaryMenuOpen] = useState(false);
  const [isSearchActive, setIsSearchActive] = useState(false);
  const [searchValue, setSearchValue] = useState('');

  const toggleIsUserMenuOpen = useCallback(
    () => setIsUserMenuOpen((isUserMenuOpen) => !isUserMenuOpen),
    [],
  );

  const toggleIsSecondaryMenuOpen = useCallback(
    () => setIsSecondaryMenuOpen((isSecondaryMenuOpen) => !isSecondaryMenuOpen),
    [],
  );

  const handleSearchResultsDismiss = useCallback(() => {
    setIsSearchActive(false);
    setSearchValue('');
  }, []);

  const handleSearchChange = useCallback((value) => {
    setSearchValue(value);
    setIsSearchActive(value.length > 0);
  }, []);

  const handleNavigationToggle = useCallback(() => {
    console.log('toggle navigation visibility');
  }, []);
  const logo = {
    topBarSource:
      '/images/oarect.png',
    width: 146,
    url: '/',
    accessibilityLabel: 'OpenAgents',
  };

  const userMenuMarkup = (
    <TopBar.UserMenu
      actions={[
        {
          items: [{ content: 'Back to OpenAgents', icon: ArrowLeftIcon }],
        },
        {
          items: [{ content: 'Community forums' }],
        },
      ]}
      name="TestUser"
      detail="Agent Builder"
      initials="T"
      open={isUserMenuOpen}
      onToggle={toggleIsUserMenuOpen}
    />
  );

  const searchResultsMarkup = (
    <ActionList
      items={[{ content: 'OpenAgents help center' }, { content: 'Community forums' }]}
    />
  );

  const searchFieldMarkup = (
    <TopBar.SearchField
      onChange={handleSearchChange}
      value={searchValue}
      placeholder="Search"
      showFocusBorder
    />
  );

  const secondaryMenuMarkup = (
    <TopBar.Menu
      activatorContent={
        <span>
          <Icon source={QuestionCircleIcon} />
          <Text as="span" visuallyHidden>
            Secondary menu
          </Text>
        </span>
      }
      open={isSecondaryMenuOpen}
      onOpen={toggleIsSecondaryMenuOpen}
      onClose={toggleIsSecondaryMenuOpen}
      actions={[
        {
          items: [{ content: 'Community forums' }],
        },
      ]}
    />
  );

  const topBarMarkup = (
    <TopBar
      showNavigationToggle
      userMenu={userMenuMarkup}
      secondaryMenu={secondaryMenuMarkup}
      searchResultsVisible={isSearchActive}
      searchField={searchFieldMarkup}
      searchResults={searchResultsMarkup}
      onSearchResultsDismiss={handleSearchResultsDismiss}
      onNavigationToggle={handleNavigationToggle}
    />
  );

  return (
    <Frame topBar={topBarMarkup} logo={logo}>
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
                  onClick: () => { },
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
  );
}
