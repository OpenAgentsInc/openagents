import React, { useCallback, useState } from 'react'
import { Frame, Navigation, TopBar, ActionList, Icon, Text } from '@shopify/polaris';
import {
  HomeIcon,
  PlusCircleIcon,
  ProductIcon,
  MinusCircleIcon,
  OrderIcon,
  ArrowLeftIcon, QuestionCircleIcon, NoteIcon
} from '@shopify/polaris-icons';
import { DropZone, Page, Badge, LegacyCard, Button, ButtonGroup, CalloutCard, FormLayout, TextField, MediaCard, RangeSlider, LegacyStack, Thumbnail } from '@shopify/polaris';
// import { ExamplePage } from './Welcome';

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
    <Frame topBar={topBarMarkup} logo={logo} navigation={<NavigationIs />}>

      <Page
        title="Example Page"
        backAction={{ content: 'Products', url: '#' }}
        titleMetadata={<Badge tone="success">Paid</Badge>}
        subtitle="Perfect for any pet"
        compactTitle
        primaryAction={{ content: 'Save', disabled: true }}
        secondaryActions={[
          {
            content: 'Duplicate',
            accessibilityLabel: 'Secondary action label',
            onAction: () => alert('Duplicate action'),
          },
          {
            content: 'View on your store',
            onAction: () => alert('View on your store action'),
          },
        ]}
        actionGroups={[
          {
            title: 'Promote',
            actions: [
              {
                content: 'Share on Facebook',
                accessibilityLabel: 'Individual action label',
                onAction: () => alert('Share on Facebook action'),
              },
            ],
          },
        ]}
        pagination={{
          hasPrevious: true,
          hasNext: true,
        }}
      >

        <LegacyCard sectioned>
          <Button onClick={() => alert('Button clicked!')}>Example button</Button>
        </LegacyCard>

        <LegacyCard sectioned>
          <ButtonGroup>
            <Button>Cancel</Button>
            <Button variant="primary">Save</Button>
          </ButtonGroup>
        </LegacyCard>

        <CalloutCard
          title="Customize the style of your checkout"
          illustration="https://cdn.shopify.com/s/assets/admin/checkout/settings-customizecart-705f57c725ac05be5a34ec20c05b94298cb8afd10aac7bd9c7ad02030f48cfa0.svg"
          primaryAction={{
            content: 'Customize checkout',
            url: '#',
          }}
        >
          <p>Upload your store’s logo, change colors and fonts, and more.</p>
        </CalloutCard>

        <LegacyCard sectioned>
          <FormLayout>
            <TextField label="Store name" onChange={() => { }} autoComplete="off" />
            <TextField
              type="email"
              label="Account email"
              onChange={() => { }}
              autoComplete="email"
            />
          </FormLayout>
        </LegacyCard>

        <LegacyCard sectioned>
          <DropZoneExample />
        </LegacyCard>

        <RangeSliderExample />

        <MediaCardExample />
      </Page>
    </Frame>
  );
}

function MediaCardExample() {
  return (
    <MediaCard
      title="Getting Started"
      primaryAction={{
        content: 'Learn about getting started',
        onAction: () => { },
      }}
      description="Discover how OpenAgents can power up your entrepreneurial journey."
      popoverActions={[{ content: 'Dismiss', onAction: () => { } }]}
    >
      <img
        alt=""
        width="100%"
        height="100%"
        style={{
          objectFit: 'cover',
          objectPosition: 'center',
        }}
        src="https://burst.shopifycdn.com/photos/business-woman-smiling-in-office.jpg?width=1850"
      />
    </MediaCard>
  );
}

function RangeSliderExample() {
  const [rangeValue, setRangeValue] = useState(32);

  const handleRangeSliderChange = useCallback(
    (value) => setRangeValue(value),
    [],
  );

  return (
    <LegacyCard sectioned title="Background color">
      <RangeSlider
        label="Opacity percentage"
        value={rangeValue}
        onChange={handleRangeSliderChange}
        output
      />
    </LegacyCard>
  );
}

function DropZoneExample() {
  const [files, setFiles] = useState([]);

  const handleDropZoneDrop = useCallback(
    (_dropFiles, acceptedFiles, _rejectedFiles) =>
      setFiles((files) => [...files, ...acceptedFiles]),
    [],
  );

  const validImageTypes = ['image/gif', 'image/jpeg', 'image/png'];

  const fileUpload = !files.length && (
    <DropZone.FileUpload actionHint="Accepts .gif, .jpg, and .png" />
  );

  const uploadedFiles = files.length > 0 && (
    <LegacyStack vertical>
      {files.map((file, index) => (
        <LegacyStack alignment="center" key={index}>
          <Thumbnail
            size="small"
            alt={file.name}
            source={
              validImageTypes.includes(file.type)
                ? window.URL.createObjectURL(file)
                : NoteIcon
            }
          />
          <div>
            {file.name}{' '}
            <Text variant="bodySm" as="p">
              {file.size} bytes
            </Text>
          </div>
        </LegacyStack>
      ))}
    </LegacyStack>
  );

  return (
    <DropZone onDrop={handleDropZoneDrop} variableHeight>
      {uploadedFiles}
      {fileUpload}
    </DropZone>
  );
}

function NavigationIs() {
  return (
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
  )
}
