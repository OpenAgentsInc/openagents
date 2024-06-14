import React from 'react'
import { Head } from '@inertiajs/react'
import {Page, LegacyCard, Button} from '@shopify/polaris';

export default function Welcome({ user }) {
  return (
    <>
      <Head title="Welcome" />
      <Page title="Example app">
      <LegacyCard sectioned>
        <Button onClick={() => alert('Button clicked!')}>Example button</Button>
      </LegacyCard>
    </Page>
    </>
  )
}
