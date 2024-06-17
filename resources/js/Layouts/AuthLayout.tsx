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

export default function AuthLayout({ children, title = "OpenAgents" }) {
  return (
    <>
      <Head title={title} />
      <Frame>
        {children}
      </Frame>
    </>
  )
}
