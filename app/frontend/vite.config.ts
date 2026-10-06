import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'
import { createReadStream, existsSync } from 'node:fs'
import type { ViteDevServer, PreviewServer } from 'vite'

const host = process.env.TAURI_DEV_HOST
// 分词器自行解压 gzip；静态服务自动添加 Content-Encoding 会导致浏览器先解压一次。
function serveDictionary(server: ViteDevServer | PreviewServer) {
  server.middlewares.use((req, res, next) => {
    const match = req.url?.split('?')[0].match(/^\/dict\/([a-z_]+\.dat\.gz)$/)
    if (!match) return next()
    const file = fileURLToPath(
      new URL(`./public/dict/${match[1]}`, import.meta.url),
    )
    if (!existsSync(file)) return next()
    res.setHeader('Content-Type', 'application/octet-stream')
    createReadStream(file).pipe(res)
  })
}

export default defineConfig(async () => ({
  plugins: [
    vue(),
    {
      name: 'dictionary-binary',
      configureServer: serveDictionary,
      configurePreviewServer: serveDictionary,
    },
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 5174 } : undefined,
    watch: {
      usePolling: process.env.GLL_WATCH_POLL === '1',
      ignored: ['**/src-tauri/**', '**/crates/**'],
    },
  },
  build: {
    target: 'es2022',
    minify: 'esbuild',
    sourcemap: false,
  },
  worker: {
    format: 'es',
  },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
}))
