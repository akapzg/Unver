import React from 'react'
import ReactDOM from 'react-dom/client'
import './i18n'
import './index.css'
import { ThemeProvider } from './store/ThemeContext'
import { ToastProvider } from './components/Toast'
import App from './App'

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <ThemeProvider>
      <ToastProvider>
        <App />
      </ToastProvider>
    </ThemeProvider>
  </React.StrictMode>
)
