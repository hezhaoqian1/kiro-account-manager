import i18n from 'i18next'
import { initReactI18next, I18nextProvider } from 'react-i18next'

// 从 JSON 文件导入翻译
import zhCN from '../locales/zh-CN.json'
import en from '../locales/en.json'
import ru from '../locales/ru.json'

// 初始化 i18n（支持中文、英文、俄文）
i18n
  .use(initReactI18next)
  .init({
    lng: localStorage.getItem('app-language') || 'zh-CN',
    fallbackLng: 'zh-CN',
    supportedLngs: ['zh-CN', 'en', 'ru'],

    resources: {
      'zh-CN': { translation: zhCN },
      'en': { translation: en },
      'ru': { translation: ru }
    },

    interpolation: {
      escapeValue: false
    },

    react: {
      useSuspense: false
    }
  })

// I18nProvider 组件
function I18nProvider({ children }) {
  return <I18nextProvider i18n={i18n}>{children}</I18nextProvider>
}

export { I18nProvider }
export default i18n
