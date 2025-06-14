/** @type {import('@openagentsinc/storybook').Meta} */
export const parameters = {
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
`
document.head.appendChild(style)