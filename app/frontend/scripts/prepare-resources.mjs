// 下载到独立临时目录，校验成功后替换；curl 沿用用户已有代理和证书配置。
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, statSync } from 'node:fs'
import { dirname, resolve, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const appDir = fileURLToPath(new URL('../../', import.meta.url))
const config = JSON.parse(readFileSync(resolve(appDir, 'resource-sources.json'), 'utf8'))
const resourceDir = resolve(appDir, 'crates/app/resources')
mkdirSync(resourceDir, { recursive: true })
const work = mkdtempSync(join(resourceDir, '.prepare-'))
const force = process.argv.includes('--force')

function download(url, path) {
  const parsed = new URL(url)
  if (parsed.protocol !== 'https:' || parsed.username || parsed.password) throw new Error('资源地址必须使用 HTTPS，且不能包含凭据')
  const result = spawnSync('curl', ['--fail', '--location', '--silent', '--show-error', '--retry', '2', '--connect-timeout', '20', '--max-time', '900', '--proto', '=https', '--proto-redir', '=https', '--output', path, url], { stdio: 'inherit' })
  if (result.status !== 0) throw new Error('资源下载失败，请检查网络、代理，或使用已有完整资源')
}
async function sha256(path) {
  const hash = createHash('sha256')
  for await (const chunk of createReadStream(path)) hash.update(chunk)
  return hash.digest('hex')
}
function extract(archive, target) {
  mkdirSync(target, { recursive: true })
  const command = process.platform === 'win32' ? 'powershell.exe' : 'unzip'
  const args = process.platform === 'win32'
    ? ['-NoProfile', '-NonInteractive', '-Command', 'Expand-Archive -LiteralPath $env:GLL_RESOURCE_ARCHIVE -DestinationPath $env:GLL_RESOURCE_EXTRACT -Force']
    : ['-q', archive, '-d', target]
  const result = spawnSync(command, args, { stdio: 'inherit', env: { ...process.env, GLL_RESOURCE_ARCHIVE: archive, GLL_RESOURCE_EXTRACT: target } })
  if (result.status !== 0) throw new Error('完整词典解压失败')
}
try {
  // 显式提供的包优先；已有完整剧情包允许离线构建，强制刷新由维护者发起。
  const story = process.env.GLL_STORY_PACK ? resolve(process.env.GLL_STORY_PACK) : join(resourceDir, 'story.gllpack')
  if (!existsSync(story) || (force && !process.env.GLL_STORY_PACK)) {
    if (process.env.GLL_STORY_PACK) throw new Error('指定的剧情包不存在')
    let lastError
    for (const url of config.storyManifestUrls) {
      try {
        const manifestPath = join(work, 'latest.json')
        download(url, manifestPath)
        if (statSync(manifestPath).size > 65536) throw new Error('资源清单过大')
        const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
        if (manifest.formatVersion !== 1 || !/^[a-f0-9]{64}$/i.test(manifest.sha256) || !Number.isSafeInteger(manifest.size) || manifest.size <= 0 || manifest.size > 1024 ** 3) throw new Error('资源清单不兼容')
        const pack = join(work, 'story.gllpack')
        download(manifest.packUrl, pack)
        if (statSync(pack).size !== manifest.size || await sha256(pack) !== manifest.sha256.toLowerCase()) throw new Error('剧情包下载不完整或校验失败')
        renameSync(pack, story)
        console.log(`已准备剧情资源 ${manifest.dataVersion}`)
        lastError = null
        break
      } catch (error) { lastError = error }
    }
    if (lastError || !existsSync(story)) throw lastError || new Error('没有可用的剧情更新源')
  }
  const dictionary = process.env.GLL_DICT_DB ? resolve(process.env.GLL_DICT_DB) : join(resourceDir, 'dict.db')
  if (!existsSync(dictionary) || (force && !process.env.GLL_DICT_DB)) {
    if (process.env.GLL_DICT_DB) throw new Error('指定的词典不存在')
    const archive = join(work, 'dictionary.zip')
    download(config.dictionary.url, archive)
    if (await sha256(archive) !== config.dictionary.sha256) throw new Error('完整词典下载校验失败')
    const extracted = join(work, 'dictionary')
    extract(archive, extracted)
    const source = join(extracted, 'dict.db')
    if (!existsSync(source)) throw new Error('下载文件没有包含完整词典')
    renameSync(source, dictionary)
    console.log('已准备完整词典')
  }
  console.log('资源已准备；正式构建还会检查剧情、词典和注音资源')
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
} finally {
  rmSync(work, { recursive: true, force: true })
}
