import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';

// Apply theme immediately to prevent flash of white
(function initializeTheme() {
  // Check localStorage for theme preference
  const savedTheme = localStorage.getItem('theme');
  
  // If we have a saved theme, apply it immediately
  if (savedTheme === 'dark') {
    document.documentElement.classList.add('dark');
    document.documentElement.style.colorScheme = 'dark';
  } else if (savedTheme === 'light') {
    document.documentElement.classList.remove('dark');
    document.documentElement.style.colorScheme = 'light';
  } else {
    // If no saved theme or system preference, check OS preference
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    if (prefersDark) {
      document.documentElement.classList.add('dark');
      document.documentElement.style.colorScheme = 'dark';
    }
  }

  // Set background color based on theme to prevent white flash
  const isDark = document.documentElement.classList.contains('dark');
  document.body.style.backgroundColor = isDark ? '#000000' : '#ffffff';
})();

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