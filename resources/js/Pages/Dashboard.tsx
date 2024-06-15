import React from 'react'
import { InlineGrid, Page } from '@shopify/polaris';

export default function Dashboard() {
  return (
    <Page title="Dashboard">
      <InlineGrid columns={2}>
        <Placeholder height="320px" />
        <Placeholder height="320px" showBorder />
      </InlineGrid>
    </Page>
  )
}

const Placeholder = ({ height = 'auto', width = 'auto', showBorder = false }) => {
  return (
    <div
      style={{
        display: 'inherit',
        background: '#1a1a1a',
        height: height ?? undefined,
        width: width ?? undefined,
        borderInlineStart: showBorder
          ? '1px dashed #fff'
          : 'none',
      }}
    />
  );
};
