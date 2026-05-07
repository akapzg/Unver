import React from 'react'
import ReactDOM from 'react-dom/client'
import './i18n'
import './index.css'
import { ThemeProvider } from './store/ThemeContext'
import { ToastProvider } from './components/Toast'
import { ConfirmProvider } from './components/ConfirmDialog'
import App from './App'

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <ThemeProvider>
      <ToastProvider>
        <ConfirmProvider>
          <App />
        </ConfirmProvider>
      </ToastProvider>
    </ThemeProvider>
  </React.StrictMode>
)
