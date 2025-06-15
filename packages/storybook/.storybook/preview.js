/** @type {import('@storybook/html').Preview} */
export default {
  parameters: {
    actions: { argTypesRegex: "^on[A-Z].*" },
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/
      }
    },
    backgrounds: {
      default: 'dark',
      values: [
        {
          name: 'dark',
          value: '#000000'
        },
        {
          name: 'light',
          value: '#ffffff'
        }
      ]
    }
  }
}

// Add global styles for OpenAgents aesthetic
const style = document.createElement('style')
style.innerHTML = `
  @import url('https://fonts.googleapis.com/css2?family=Berkeley+Mono:ital,wght@0,400;0,700;1,400;1,700&display=swap');
  
  body {
    font-family: 'Berkeley Mono', monospace !important;
    background-color: #000000 !important;
    color: #ffffff !important;
  }
  
  .sb-show-main {
    background-color: #000000 !important;
    color: #ffffff !important;
  }
  
  * {
    font-family: 'Berkeley Mono', monospace !important;
  }
  
  /* Button styles */
  .btn {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    white-space: nowrap;
    font-size: 14px;
    font-weight: 500;
    font-family: 'Berkeley Mono', monospace;
    transition: all 0.2s;
    border-radius: 0;
    outline: none;
    border: 1px solid;
  }
  
  .btn-primary {
    background-color: #ffffff;
    color: #000000;
    border-color: #ffffff;
  }
  
  .btn-secondary {
    background-color: transparent;
    color: #ffffff;
    border-color: #ffffff;
  }
  
  .btn-danger {
    background-color: #dc2626;
    color: #ffffff;
    border-color: #dc2626;
  }
  
  .btn-sm {
    height: 32px;
    padding: 0 12px;
    font-size: 12px;
  }
  
  .btn-default {
    height: 36px;
    padding: 0 16px;
    font-size: 14px;
  }
  
  .btn-lg {
    height: 40px;
    padding: 0 32px;
    font-size: 16px;
  }
  
  .btn-disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  
  /* Card styles */
  .card {
    font-family: 'Berkeley Mono', monospace;
    background-color: #000000;
    color: #ffffff;
    border: 1px solid #ffffff;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    box-shadow: 0 1px 3px 0 rgba(255, 255, 255, 0.1);
    width: 400px;
  }
  
  .card-header {
    display: grid;
    grid-template-rows: auto auto;
    align-items: start;
    gap: 6px;
  }
  
  .card-title {
    font-family: 'Berkeley Mono', monospace;
    font-weight: 600;
    line-height: 1;
    font-size: 16px;
  }
  
  .card-description {
    font-family: 'Berkeley Mono', monospace;
    color: #a1a1aa;
    font-size: 14px;
  }
  
  .card-content {
    font-family: 'Berkeley Mono', monospace;
    line-height: 1.5;
  }
  
  .card-footer {
    font-family: 'Berkeley Mono', monospace;
    display: flex;
    align-items: center;
    margin-top: 24px;
  }
`
document.head.appendChild(style)