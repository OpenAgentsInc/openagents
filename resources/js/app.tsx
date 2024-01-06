import './bootstrap';
import '../css/regards.css';
import '../css/app.css';

import { createRoot } from 'react-dom/client';
import { createInertiaApp } from '@inertiajs/react';
import { resolvePageComponent } from 'laravel-vite-plugin/inertia-helpers';

createInertiaApp({
  title: (title) => `OpenAgents`,
  // title: (title) => `${title} - ${appName}`,
  resolve: (name) => resolvePageComponent(`./Pages/${name}.tsx`, import.meta.glob('./Pages/**/*.tsx')),
  setup({ el, App, props }) {
    const root = createRoot(el);

    root.render(<App {...props} />);
  },
  progress: {
    delay: 250,
    color: '#6365f1',
    includeCSS: true,
    showSpinner: true
  }
});
