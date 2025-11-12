import type { Preview } from '@storybook/react-vite';
import '../src/App.css';
import { useModelStore } from '../src/lib/model-store';

// Prefer the local (mocked) runtime in Storybook
useModelStore.getState().setSelected('ollama');

// Ensure the entire preview iframe uses dark theme and background
if (typeof document !== 'undefined') {
  document.documentElement.classList.add('dark');
  document.body.classList.add('dark');
}

const preview: Preview = {
  decorators: [
    (Story) => (
      <div
        className="dark"
        style={{
          width: '100vw',
          minHeight: '100vh',
          background: 'var(--background)',
          color: 'var(--foreground)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <Story />
      </div>
    ),
  ],
  parameters: {
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
    a11y: { test: 'todo' },
    layout: 'fullscreen',
  },
};

export default preview;
