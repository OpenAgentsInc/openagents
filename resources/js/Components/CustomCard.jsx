import React from 'react';
import { AppProvider, BlockStack, Button, Card, InlineGrid, Text } from '@shopify/polaris';
import enTranslations from '@shopify/polaris/locales/en.json';
import { PlusIcon } from '@shopify/polaris-icons';

export default function CustomCard({ title, description}) {
  return (
    <AppProvider i18n={enTranslations} theme="dark-experimental">
      <Card roundedAbove="sm">
        <BlockStack gap="200">
          <InlineGrid columns="1fr auto">
            <Text as="h2" variant="headingSm">
             {title}
            </Text>
            <Button
              onClick={() => { }}
              // accessibilityLabel="More actions"
              icon={PlusIcon}
            >
              More actions
            </Button>
          </InlineGrid>
          <Text as="p" variant="bodyMd">
            {description}
          </Text>
        </BlockStack>
      </Card>
    </AppProvider>
  );
}
