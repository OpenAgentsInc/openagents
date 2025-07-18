export const theme = {
  dark: true,
  
  colors: {
    primary: {
      main: '#00ffff'
    },
    secondary: {
      main: '#ff00ff'
    },
    success: {
      main: '#00ff00'
    },
    error: {
      main: '#ff0000'
    },
    neutral: {
      main: '#021114'
    }
  },
  
  fontFamilies: {
    title: 'var(--font-titillium), "Titillium Web", sans-serif',
    body: 'var(--font-berkeley-mono), "Berkeley Mono", monospace',
    code: 'var(--font-berkeley-mono), "Berkeley Mono", monospace'
  },
  
  fontWeights: {
    light: 300,
    normal: 400,
    bold: 600
  },
  
  space: 4,
  
  shape: {
    rectangle: { borderRadius: 0 },
    square: { borderRadius: 0 },
    round: { borderRadius: '50%' }
  }
};