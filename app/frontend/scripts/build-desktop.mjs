import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const env = { ...process.env }
// 本机构建默认使用 ad-hoc 签名封存整个 .app；已有用户签名配置始终优先。
if (process.platform === 'darwin' && !env.APPLE_SIGNING_IDENTITY) {
  env.APPLE_SIGNING_IDENTITY = '-'
}
const result = spawnSync(process.execPath, [
  fileURLToPath(new URL('../node_modules/@tauri-apps/cli/tauri.js', import.meta.url)),
  'build', ...process.argv.slice(2),
], {
  cwd: fileURLToPath(new URL('../../crates/app/', import.meta.url)),
  env,
  stdio: 'inherit',
})
if (result.error) console.error(result.error.message)
process.exit(result.status ?? 1)
