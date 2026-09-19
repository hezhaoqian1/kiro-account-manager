import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'
import pkg from './package.json'

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
      ...(process.env.TAURI_PLATFORM ? {} : {
        '@tauri-apps/api/core': path.resolve(__dirname, './src/compat/tauriCore.ts'),
        '@tauri-apps/api/event': path.resolve(__dirname, './src/compat/tauriEvent.ts'),
        '@tauri-apps/api/app': path.resolve(__dirname, './src/compat/tauriApp.ts'),
        '@tauri-apps/api/path': path.resolve(__dirname, './src/compat/tauriPath.ts'),
        '@tauri-apps/api/window': path.resolve(__dirname, './src/compat/tauriWindow.ts'),
        '@tauri-apps/plugin-dialog': path.resolve(__dirname, './src/compat/dialog.ts'),
        '@tauri-apps/plugin-fs': path.resolve(__dirname, './src/compat/fs.ts'),
        '@tauri-apps/plugin-opener': path.resolve(__dirname, './src/compat/opener.ts'),
        '@tauri-apps/plugin-process': path.resolve(__dirname, './src/compat/process.ts'),
        '@tauri-apps/plugin-updater': path.resolve(__dirname, './src/compat/updater.ts'),
        '@tauri-apps/plugin-shell': path.resolve(__dirname, './src/compat/shell.ts'),
      }),
    },
  },
  plugins: [
    react(),
    tailwindcss(),
  ],
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // 优化开发服务器性能
    hmr: {
      overlay: false, // 禁用错误覆盖层，减少渲染开销
    },
    // 预热常用文件，加快首次加载
    warmup: {
      clientFiles: ['./src/main.tsx', './src/App.tsx'],
    },
  },
  // 优化依赖预构建
  optimizeDeps: {
    include: [
      'react',
      'react-dom',
      'lucide-react',
      '@tauri-apps/api',
      'i18next',
      'react-i18next',
    ],
    // 强制预构建，避免首次启动慢
    force: false,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: process.env.TAURI_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_DEBUG ? 'terser' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    terserOptions: {
      compress: {
        drop_console: true,      // 移除 console.log
        drop_debugger: true,     // 移除 debugger
        pure_funcs: ['console.info', 'console.debug', 'console.warn'],
      },
      mangle: {
        toplevel: true,          // 混淆顶级变量名
        safari10: true,
      },
      format: {
        comments: false,         // 移除所有注释
      },
    },
    // 代码分割优化
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor': ['react', 'react-dom'],
          'icons': ['lucide-react'],
          'tauri': [
            '@tauri-apps/api',
            '@tauri-apps/plugin-dialog',
            '@tauri-apps/plugin-fs',
            '@tauri-apps/plugin-opener',
            '@tauri-apps/plugin-process',
            '@tauri-apps/plugin-shell',
            '@tauri-apps/plugin-updater'
          ],
          'i18n': ['i18next', 'react-i18next'],
        }
      }
    },
    // 大项目可关闭压缩报告加速构建
    reportCompressedSize: false,
  },
})
