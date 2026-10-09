import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { existsSync, copyFileSync, readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const env = { ...process.env }
const appDir = fileURLToPath(new URL('../../crates/app/', import.meta.url))
const storyPack = resolve(appDir, 'resources/story.gllpack')
const inputPack = env.GLL_STORY_PACK ? resolve(env.GLL_STORY_PACK) : storyPack
if (!existsSync(inputPack)) {
  console.error('正式构建需要完整剧情包，请先执行 npm run resources:prepare，或设置 GLL_STORY_PACK')
  process.exit(1)
}
const dict = resolve(appDir, 'resources/dict.db')
const inputDict = env.GLL_DICT_DB ? resolve(env.GLL_DICT_DB) : dict
const furigana = fileURLToPath(new URL('../public/dict/', import.meta.url))
const expectedDict = fileURLToPath(new URL('../node_modules/@wwzzyying/kuromoji/dict/', import.meta.url))
const annotationFiles = existsSync(expectedDict) ? readdirSync(expectedDict).filter(name => name.endsWith('.dat.gz')) : []
if (!existsSync(inputDict) || !annotationFiles.length || !annotationFiles.every(name => existsSync(resolve(furigana, name)) && readFileSync(resolve(furigana, name)).equals(readFileSync(resolve(expectedDict, name))))) {
  console.error('完整词典或注音资源缺失，请执行 npm ci 和 npm run resources:prepare')
  process.exit(1)
}
for (const args of [['story-pack', 'inspect', inputPack], ['dict', 'inspect', inputDict]]) {
  const check = spawnSync('cargo', ['run', '--locked', '-p', 'xtask', '--', ...args], { cwd: fileURLToPath(new URL('../../', import.meta.url)), env, stdio: 'inherit' })
  if (check.status !== 0) process.exit(check.status || 1)
}
if (inputPack !== storyPack) copyFileSync(inputPack, storyPack)
if (inputDict !== dict) copyFileSync(inputDict, dict)
// 本机构建默认使用 ad-hoc 签名封存整个 .app；已有用户签名配置始终优先。
if (process.platform === 'darwin' && !env.APPLE_SIGNING_IDENTITY) {
  env.APPLE_SIGNING_IDENTITY = '-'
}
const result = spawnSync(process.execPath, [
  fileURLToPath(new URL('../node_modules/@tauri-apps/cli/tauri.js', import.meta.url)),
  'build', ...process.argv.slice(2),
], {
  cwd: appDir,
  env,
  stdio: 'inherit',
})
if (result.error) console.error(result.error.message)
process.exit(result.status ?? 1)
