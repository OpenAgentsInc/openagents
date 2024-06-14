import React from 'react'
import { Head } from '@inertiajs/react'
import { Page, Badge, LegacyCard, Button, CalloutCard, FormLayout, TextField } from '@shopify/polaris';

export default function Welcome({ user }) {
  return (
    <>
      <Head title="Welcome" />
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
      </Page>
    </>
  )
}
