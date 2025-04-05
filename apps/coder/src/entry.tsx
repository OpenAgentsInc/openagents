import React, { Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';

// Error boundary component to catch rendering errors
class ErrorBoundary extends React.Component<{ children: React.ReactNode }, { hasError: boolean, error: Error | null }> {
  constructor(props: { children: React.ReactNode }) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Render error caught by ErrorBoundary:', error, errorInfo);
    
    // Log to console with detailed stack trace
    console.error('React error details:', {
      message: error.message,
      name: error.name,
      stack: error.stack,
      componentStack: errorInfo.componentStack
    });
  }

  render() {
    if (this.state.hasError) {
      // Build a simple error display
      return (
        <div style={{ 
          padding: '20px', 
          fontFamily: 'monospace',
          backgroundColor: '#300', 
          color: 'white', 
          border: '1px solid #900',
          borderRadius: '4px',
          margin: '20px',
          display: 'flex',
          flexDirection: 'column',
          gap: '10px'
        }}>
          <h1 style={{ margin: '0 0 10px 0', fontSize: '18px' }}>Application Error</h1>
          <p style={{ margin: 0 }}>The application encountered an error during rendering.</p>
          <div style={{ 
            backgroundColor: '#200', 
            padding: '10px', 
            borderRadius: '4px',
            overflow: 'auto',
            maxHeight: '300px'
          }}>
            <p style={{ color: '#f88', margin: 0 }}>{this.state.error?.name}: {this.state.error?.message}</p>
            <pre style={{ color: '#fcc', fontSize: '12px', margin: '10px 0 0 0' }}>
              {this.state.error?.stack}
            </pre>
          </div>
          <p style={{ margin: '10px 0 0 0' }}>Please check the console for more details.</p>
          <button 
            onClick={() => window.location.reload()} 
            style={{ 
              backgroundColor: '#822',
              color: 'white',
              border: 'none',
              padding: '8px 12px',
              borderRadius: '4px',
              cursor: 'pointer',
              marginTop: '10px',
              alignSelf: 'flex-start'
            }}
          >
            Reload Application
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

// Display a loading state while React is initializing
const LoadingFallback = () => (
  <div style={{ 
    display: 'flex', 
    justifyContent: 'center', 
    alignItems: 'center', 
    height: '100vh',
    fontSize: '16px',
    fontFamily: 'monospace'
  }}>
    <div style={{ textAlign: 'center' }}>
      <p>Loading application...</p>
      <div style={{ 
        width: '50px', 
        height: '50px', 
        border: '5px solid #f3f3f3',
        borderTop: '5px solid #3498db',
        borderRadius: '50%',
        margin: '20px auto',
        animation: 'spin 1s linear infinite'
      }}></div>
      <style>{`
        @keyframes spin {
          0% { transform: rotate(0deg); }
          100% { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  </div>
);

// Apply theme immediately to prevent flash of white
(function initializeTheme() {
  try {
    // Log initialization
    console.log('[Renderer] Initializing theme...');
    
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
    document.body.style.backgroundColor = isDark ? '#1a1a1a' : '#ffffff';
    
    console.log(`[Renderer] Theme initialized: ${isDark ? 'dark' : 'light'}`);
  } catch (error) {
    console.error('[Renderer] Error initializing theme:', error);
    // Fallback to dark mode if there's an error
    document.documentElement.classList.add('dark');
    document.documentElement.style.colorScheme = 'dark';
    document.body.style.backgroundColor = '#1a1a1a';
  }
})();

// Add global uncaught error handlers
window.addEventListener('error', (event) => {
  console.error('[Renderer] Uncaught error:', event.error || event.message);
});

window.addEventListener('unhandledrejection', (event) => {
  console.error('[Renderer] Unhandled promise rejection:', event.reason);
});

// Function to initialize the application
function initializeApp() {
  console.log('[Renderer] Starting application initialization...');
  
  try {
    // Mount the app to the DOM
    const container = document.getElementById('app');
    
    if (!container) {
      throw new Error('Could not find app container element. Check if #app exists in the HTML.');
    }
    
    // Only create root if container exists and hasn't been used before
    if (!container.hasAttribute('data-react-mounted')) {
      console.log('[Renderer] Creating React root...');
      const root = createRoot(container);
      container.setAttribute('data-react-mounted', 'true');
      
      console.log('[Renderer] Rendering application...');
      root.render(
        <ErrorBoundary>
          <Suspense fallback={<LoadingFallback />}>
            <App />
          </Suspense>
        </ErrorBoundary>
      );
      
      // Hide the initial loader now that React is rendering
      if (typeof window.hideInitialLoader === 'function') {
        console.log('[Renderer] Hiding initial loader');
        window.hideInitialLoader();
      } else {
        console.log('[Renderer] hideInitialLoader function not found, trying direct DOM manipulation');
        try {
          const loader = document.getElementById('initial-loader');
          const app = document.getElementById('app');
          
          if (loader) loader.style.display = 'none';
          if (app) app.style.display = 'block';
        } catch (error) {
          console.error('[Renderer] Error hiding initial loader:', error);
        }
      }
      
      console.log('[Renderer] Application rendered successfully.');
    } else {
      console.warn('[Renderer] App container already has React mounted. Skipping render.');
    }
  } catch (error) {
    console.error('[Renderer] Critical error during application initialization:', error);
    
    // Display error on page
    document.body.innerHTML = `
      <div style="padding: 20px; font-family: monospace; background-color: #300; color: white; border: 1px solid #900; border-radius: 4px; margin: 20px;">
        <h1 style="margin: 0 0 10px 0; font-size: 18px;">Critical Initialization Error</h1>
        <p style="margin: 0">The application failed to initialize:</p>
        <div style="background-color: #200; padding: 10px; border-radius: 4px; margin-top: 10px; white-space: pre-wrap; overflow: auto;">
          ${error instanceof Error ? `${error.name}: ${error.message}\n\n${error.stack || ''}` : String(error)}
        </div>
        <p style="margin: 10px 0 0 0">Check the console for more details.</p>
        <button onclick="window.location.reload()" style="background-color: #822; color: white; border: none; padding: 8px 12px; border-radius: 4px; cursor: pointer; margin-top: 10px;">
          Reload Application
        </button>
      </div>
    `;
  }
}

// Start the application
initializeApp();