import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './styles.css'

// 在 React mount 之前就给 body 加深色背景，避免 settings 窗口闪白
const isSettings = new URLSearchParams(window.location.search).get('page') === 'settings'
if (isSettings) {
  document.body.classList.add('settings-mode')
} else {
  document.body.classList.add('floating-mode')
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
