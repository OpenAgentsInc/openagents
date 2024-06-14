import React from 'react'
import { createInertiaApp } from '@inertiajs/react'
import { createRoot } from 'react-dom/client'
import '@shopify/polaris/build/esm/styles.css';
import enTranslations from '@shopify/polaris/locales/en.json';
import {AppProvider} from '@shopify/polaris';

createInertiaApp({
  resolve: name => {
    const pages = import.meta.glob('./Pages/**/*.{jsx,tsx}', { eager: true });
    return pages[`./Pages/${name}.jsx`] || pages[`./Pages/${name}.tsx`];
  },
  setup({ el, App, props }) {
    createRoot(el).render(
      <AppProvider i18n={enTranslations} theme="dark-experimental">
        <App {...props} />
      </AppProvider>
    )
  },
})
