import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';

// Mount the app to the DOM
const container = document.getElementById('app');

// Only create root if container exists and hasn't been used before
if (container && !container.hasAttribute('data-react-mounted')) {
  const root = createRoot(container);
  container.setAttribute('data-react-mounted', 'true');
  
  root.render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  );
}