import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { existsSync, copyFileSync } from 'node:fs'
import { resolve } from 'node:path'

const env = { ...process.env }
const appDir = fileURLToPath(new URL('../../crates/app/', import.meta.url))
const storyPack = resolve(appDir, 'resources/story.gllpack')
const inputPack = env.GLL_STORY_PACK ? resolve(env.GLL_STORY_PACK) : storyPack
if (env.GLL_STORY_PACK && !existsSync(inputPack)) {
  console.error('指定的剧情资源文件不存在')
  process.exit(1)
}
const resourceArgs = []
if (existsSync(inputPack)) {
  const inspection = spawnSync('cargo', ['run', '-p', 'xtask', '--', 'story-pack', 'inspect', inputPack], {
    cwd: fileURLToPath(new URL('../../', import.meta.url)), env, stdio: 'inherit',
  })
  if (inspection.status !== 0) process.exit(inspection.status || 1)
  if (inputPack !== storyPack) copyFileSync(inputPack, storyPack)
  resourceArgs.push('--config', JSON.stringify({ bundle: { resources: ['resources/dict.db', 'resources/story.gllpack'] } }))
}
// 本机构建默认使用 ad-hoc 签名封存整个 .app；已有用户签名配置始终优先。
if (process.platform === 'darwin' && !env.APPLE_SIGNING_IDENTITY) {
  env.APPLE_SIGNING_IDENTITY = '-'
}
const result = spawnSync(process.execPath, [
  fileURLToPath(new URL('../node_modules/@tauri-apps/cli/tauri.js', import.meta.url)),
  'build', ...resourceArgs, ...process.argv.slice(2),
], {
  cwd: appDir,
  env,
  stdio: 'inherit',
})
if (result.error) console.error(result.error.message)
process.exit(result.status ?? 1)
