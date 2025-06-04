import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@openagentsinc/ui/web/styles/global.css'
import './index.css'
import App from './App.js'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)