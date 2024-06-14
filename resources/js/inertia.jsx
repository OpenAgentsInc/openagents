import React from 'react'
import { createInertiaApp } from '@inertiajs/react'
import { createRoot } from 'react-dom/client'
import '@shopify/polaris/build/esm/styles.css';
import enTranslations from '@shopify/polaris/locales/en.json';
import {AppProvider} from '@shopify/polaris';
import Layout from './Layout'

createInertiaApp({
  resolve: name => {
    const pages = import.meta.glob('./Pages/**/*.{jsx,tsx}', { eager: true });
    let page = pages[`./Pages/${name}.jsx`] || pages[`./Pages/${name}.tsx`];
    page.default.layout = page.default.layout || (page => <Layout children={page} />)
    return page
  },
  setup({ el, App, props }) {
    createRoot(el).render(
      <AppProvider i18n={enTranslations} theme="dark-experimental">
        <App {...props} />
      </AppProvider>
    )
  },
})
