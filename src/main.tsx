import React from 'react'
import ReactDOM from 'react-dom/client'
import './index.css'
import App from './App'
import { ThemeProvider } from './components/theme-provider'
import { DialogProvider } from './contexts/DialogContext'
import { AppSettingsProvider } from './contexts/AppSettingsContext'
import { I18nProvider } from './i18n'
import { TooltipProvider } from '@/components/ui/tooltip'

// 生产环境禁用浏览器快捷键
if (import.meta.env.PROD) {
  document.addEventListener('keydown', (e: KeyboardEvent) => {
    if (e.key === 'F5' || e.key === 'F12') {
      e.preventDefault()
    }
    if (e.ctrlKey) {
      const key = e.key.toLowerCase()
      if (['r', 'u', 'p', 's', 'g', 'f'].includes(key)) {
        e.preventDefault()
      }
      if (e.shiftKey && ['i', 'j'].includes(key)) {
        e.preventDefault()
      }
    }
  })

  document.addEventListener('contextmenu', (e: MouseEvent) => {
    e.preventDefault()
  })
}

const rootElement = document.getElementById('root')
if (!rootElement) throw new Error('Failed to find the root element')

ReactDOM.createRoot(rootElement).render(
  <I18nProvider>
    <AppSettingsProvider>
      <ThemeProvider
        attribute="data-theme"
        defaultTheme="dark"
        // 开启系统跟随：theme='system' 时 next-themes 会按 prefers-color-scheme
        // 把 data-theme 解析成 'light' 或 'dark'（见外观设置里的「跟随系统」选项）。
        enableSystem
        disableTransitionOnChange
        themes={[
          'light', 'dark', 'dark-one', 'tech', 'midnight', 
          'purple', 'green', 'business', 'sunset', 'ocean', 
          'forest', 'rose', 'aurora', 'sakura'
        ]}
      >
        <TooltipProvider>
          <DialogProvider>
            <App />
          </DialogProvider>
        </TooltipProvider>
      </ThemeProvider>
    </AppSettingsProvider>
  </I18nProvider>,
)
