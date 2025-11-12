import type { Preview } from '@storybook/react-vite';
import '../src/App.css';
import { useModelStore } from '../src/lib/model-store';

// Prefer the local (mocked) runtime in Storybook
useModelStore.getState().setSelected('ollama');

const preview: Preview = {
  decorators: [
    (Story) => (
      <div
        className="dark"
        style={{ minHeight: '100vh', background: 'var(--background)', color: 'var(--foreground)' }}
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
    layout: 'padded',
  },
};

export default preview;
