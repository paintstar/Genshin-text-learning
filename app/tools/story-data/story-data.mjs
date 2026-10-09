#!/usr/bin/env node
import { createHash } from 'node:crypto'
import { createReadStream, createWriteStream } from 'node:fs'
import { mkdir, readFile, writeFile, rename, stat } from 'node:fs/promises'
import { resolve, dirname, join } from 'node:path'
import { Readable } from 'node:stream'
import { pipeline } from 'node:stream/promises'
import { createGzip } from 'node:zlib'

const options = new Map()
const command = process.argv[2]
for (let i = 3; i < process.argv.length; i++) {
  const key = process.argv[i]
  if (!key.startsWith('--')) throw new Error('参数需要使用 --名称 值')
  options.set(key.slice(2), process.argv[i + 1]?.startsWith('--') || !process.argv[i + 1] ? true : process.argv[++i])
}
const option = (name, fallback) => options.get(name) ?? fallback
const outputPath = () => resolve(String(option('output', command === 'fixture' ? 'dist/fixtures/story.gllpack' : 'dist/story.gllpack')))
const isReleased = entry => typeof entry?.chapterTitle === 'string' && entry.chapterTitle.trim() && !entry.chapterTitle.includes('$UNRELEASED')
const now = () => Math.floor(Date.now() / 1000)
const sleep = ms => new Promise(done => setTimeout(done, ms))
const sha = text => createHash('sha256').update(text).digest('hex')
async function json(path) { return JSON.parse(await readFile(path, 'utf8')) }
async function atomic(path, text) {
  await mkdir(dirname(path), { recursive: true })
  await writeFile(`${path}.tmp`, text)
  await rename(`${path}.tmp`, path)
}
function number(name, fallback, min = 0) {
  const value = Number(option(name, fallback))
  if (!Number.isFinite(value) || value < min) throw new Error(`${name} 参数无效`)
  return value
}
function index(raw) {
  if (!raw?.data?.items || typeof raw.data.items !== 'object' || Array.isArray(raw.data.items)) throw new Error('任务目录结构无效')
  const out = {}
  for (const entry of Object.values(raw.data.items)) {
    if (!Number.isSafeInteger(entry?.id) || entry.id <= 0) throw new Error('任务编号无效')
    if (out[entry.id]) throw new Error('任务目录包含重复编号')
    out[entry.id] = Object.fromEntries(['id', 'type', 'chapterNum', 'chapterTitle', 'route', 'chapterCount'].filter(key => entry[key] !== undefined).map(key => [key, entry[key]]))
  }
  return out
}
function detail(raw, questId) {
  if (!raw?.data?.storyList || typeof raw.data.storyList !== 'object' || Array.isArray(raw.data.storyList)) throw new Error('剧情正文结构无效')
  if (raw.data.id !== undefined && Number(raw.data.id) !== questId) throw new Error('正文任务编号不一致')
  const storyList = {}
  const blocks = []
  const allIds = new Set()
  for (const [subId, sub] of Object.entries(raw.data.storyList)) {
    if (!/^\d+$/.test(subId) || !sub?.story || typeof sub.story !== 'object' || Array.isArray(sub.story)) throw new Error('剧情章节结构无效')
    const story = {}
    for (const [stepId, step] of Object.entries(sub.story)) {
      if (!/^\d+$/.test(stepId)) throw new Error('剧情步骤编号无效')
      const taskData = []
      for (const block of Array.isArray(step?.taskData) ? step.taskData : []) {
        if (!block?.items || typeof block.items !== 'object' || Array.isArray(block.items)) throw new Error('对白块结构无效')
        const initDialog = String(block.initDialog ?? '')
        if (!Object.hasOwn(block.items, initDialog)) throw new Error('对白入口不存在')
        const items = {}
        for (const [id, node] of Object.entries(block.items)) {
          if (!node || typeof node !== 'object' || !['SingleDialog', 'MultiDialog'].includes(node.type) || !Array.isArray(node.text)) throw new Error('对白节点结构无效')
          items[id] = { type: node.type, role: node.role, text: node.text.map(row => {
            if (!row || typeof row !== 'object') throw new Error('对白文本结构无效')
            const next = row.next
            if (next !== undefined && next !== null && !(typeof next === 'number' || typeof next === 'string' && (/^\d+$/.test(next) || next === 'finish' || next.endsWith('-player')))) throw new Error('对白跳转格式无效')
            return { text: row.text, ...(next === undefined ? {} : { next }) }
          }) }
          allIds.add(id)
        }
        const parsed = { initDialog: block.initDialog, items }
        taskData.push(parsed)
        blocks.push(parsed)
      }
      story[stepId] = { taskData }
    }
    storyList[subId] = { info: { title: sub.info?.title, description: sub.info?.description }, story }
  }
  for (const block of blocks) for (const node of Object.values(block.items)) for (const row of node.text) {
    if (row.next !== undefined && row.next !== null && row.next !== 'finish' && !Object.hasOwn(block.items, String(row.next)) && allIds.has(String(row.next))) throw new Error('检测到跨对白块跳转，当前应用暂不支持')
  }
  return { data: { id: questId, storyList } }
}
function record(id, jp, chs) {
  const a = detail(jp, id), b = detail(chs, id)
  return { questId: id, jp: a, chs: b, jpSha256: sha(JSON.stringify(a)), chsSha256: sha(JSON.stringify(b)) }
}
let lastRequest = 0
async function fetchJson(source, path, validator) {
  for (let attempt = 0; attempt < 3; attempt++) {
    await sleep(Math.max(0, lastRequest + number('interval-ms', 1000, 1000) - Date.now()))
    lastRequest = Date.now()
    let response
    try {
      response = await fetch(`${source}/${path}`, {
        headers: { 'User-Agent': 'GenshinLangLearning-resource-maintainer/1', ...(validator?.etag ? { 'If-None-Match': validator.etag } : {}), ...(validator?.lastModified ? { 'If-Modified-Since': validator.lastModified } : {}) },
        signal: AbortSignal.timeout(60000),
      })
    } catch {
      if (attempt === 2) throw new Error('请求超时或网络不可用')
      await sleep(1000 * 2 ** attempt)
      continue
    }
    if (response.status === 304) return null
    if (response.status === 429 || response.status >= 500) {
      const retryAfter = response.headers.get('retry-after')
      const wait = retryAfter ? (/^\d+$/.test(retryAfter) ? Number(retryAfter) * 1000 : Date.parse(retryAfter) - Date.now()) : 1000 * 2 ** attempt
      await response.body?.cancel()
      if (attempt === 2) throw new Error(`源站返回 ${response.status}`)
      if (Number.isFinite(wait) && wait > 300000) throw new Error('源站要求暂停较长时间，请稍后续采')
      await sleep(Number.isFinite(wait) ? Math.max(1000, wait) : 1000)
      continue
    }
    if (!response.ok) { await response.body?.cancel(); throw new Error(`源站返回 ${response.status}`) }
    const text = await response.text()
    if (text.length > 64 * 1024 * 1024) throw new Error('源站响应过大')
    let raw
    try { raw = JSON.parse(text) } catch { throw new Error('源站响应不是有效 JSON') }
    return { raw, validator: { etag: response.headers.get('etag'), lastModified: response.headers.get('last-modified') } }
  }
}
async function build(cache, jpIndex, chsIndex, ids, fixture = false) {
  const output = outputPath()
  const dataVersion = String(option('data-version', new Date().toISOString().replace(/[:.]/g, '-')))
  if (!/^[A-Za-z0-9._-]{1,128}$/.test(dataVersion)) throw new Error('资源版本仅支持字母、数字、点、下划线和短横线')
  const items = langIndex => ({ data: { items: Object.fromEntries(ids.map(id => [id, langIndex[id]])) } })
  const header = { formatVersion: 1, dataVersion, createdAt: now(), questCount: ids.length, fixture, index: { jp: items(jpIndex), chs: items(chsIndex) } }
  if (!ids.length) throw new Error('没有可发布的双语任务')
  const fingerprint = createHash('sha256').update(JSON.stringify(header.index))
  for (const id of ids) {
    const pair = await json(join(cache, `${id}.json`))
    const checked = record(id, pair.jp, pair.chs)
    fingerprint.update(`${id}:${checked.jpSha256}:${checked.chsSha256}\n`)
  }
  const contentDigest = fingerprint.digest('hex')
  const previous = await json(join(cache, 'published.json')).catch(() => null)
  if (command === 'collect' && previous?.contentDigest === contentDigest) {
    await atomic(join(dirname(output), 'run-report.json'), JSON.stringify({ changed: false, questCount: ids.length, failures: collectionFailures }, null, 2))
    console.log(`内容未变：保留 ${previous.dataVersion}，${ids.length} 个任务`)
    return
  }
  async function* lines() {
    yield `${JSON.stringify(header)}\n`
    for (const id of ids) {
      const pair = await json(join(cache, `${id}.json`))
      yield `${JSON.stringify(record(id, pair.jp, pair.chs))}\n`
    }
  }
  await mkdir(dirname(output), { recursive: true })
  await pipeline(Readable.from(lines()), createGzip(), createWriteStream(`${output}.tmp`))
  await rename(`${output}.tmp`, output)
  const hash = createHash('sha256')
  for await (const chunk of createReadStream(output)) hash.update(chunk)
  const base = String(option('download-base', '')).replace(/\/$/, '')
  const manifest = { formatVersion: 1, dataVersion, packUrl: base ? `${base}/story.gllpack` : '', sha256: hash.digest('hex'), size: (await stat(output)).size, contentDigest }
  await atomic(join(dirname(output), 'latest.json'), JSON.stringify(manifest, null, 2))
  await atomic(join(dirname(output), 'run-report.json'), JSON.stringify({ changed: true, questCount: ids.length, failures: collectionFailures }, null, 2))
  console.log(`生成资源 ${dataVersion}：${ids.length} 个双语任务，${manifest.size} 字节${fixture ? '（开发样本）' : ''}`)
}
let collectionFailures = []
async function main() {
  const cache = resolve(String(option('cache-dir', command === 'fixture' ? 'work/fixture-cache' : 'work/cache')))
  await mkdir(cache, { recursive: true })
  if (command === 'fixture') {
    const fixtures = resolve(String(option('fixtures', 'app/fixtures')))
    const pair = record(1702, await json(join(fixtures, 'quest-1702-jp.json')), await json(join(fixtures, 'quest-1702-chs.json')))
    await atomic(join(cache, '1702.json'), JSON.stringify(pair))
    const jp = { 1702: { id: 1702, type: 'wq', chapterTitle: '白夜国浮世画天夢', chapterCount: 3 } }
    const chs = { 1702: { id: 1702, type: 'wq', chapterTitle: '白夜似梦初醒', chapterCount: 3 } }
    await build(cache, jp, chs, [1702], true)
    return
  }
  if (command !== 'collect' && command !== 'build') throw new Error('用法：story-data.mjs collect|build|fixture [--cache-dir 路径] [--output 路径] [--data-version 版本]')
  let jp, chs
  if (command === 'collect') {
    const source = String(option('source-url', 'https://gi.yatta.moe/api/v2')).replace(/\/$/, '')
    const parsed = new URL(source)
    if (parsed.protocol !== 'https:' || parsed.username || parsed.password) throw new Error('上游地址必须是无凭据的 HTTPS URL')
    const previousJp = await json(join(cache, 'index-jp.json')).catch(() => ({}))
    const previousChs = await json(join(cache, 'index-chs.json')).catch(() => ({}))
    console.log('开始同步任务目录')
    jp = index((await fetchJson(source, 'jp/quest')).raw)
    chs = index((await fetchJson(source, 'chs/quest')).raw)
    const currentIds = new Set(Object.keys(jp).map(Number).filter(id => isReleased(jp[id]) && isReleased(chs[id])))
    const previousCount = Object.keys(previousJp).filter(id => isReleased(previousJp[id]) && isReleased(previousChs[id])).length
    const maxDrop = number('max-index-drop', 0.1)
    if (maxDrop > 1) throw new Error('max-index-drop 参数需要在 0 到 1 之间')
    if (previousCount && currentIds.size < previousCount * (1 - maxDrop)) throw new Error('任务目录数量异常减少，本轮停止，保留旧资源')
    // 已收录的历史任务不会随上游目录移除而消失。
    const archivedIds = new Set()
    for (const id of Object.keys(previousJp).map(Number)) {
      if (currentIds.has(id) || !isReleased(previousJp[id]) || !isReleased(previousChs[id])) continue
      const old = await json(join(cache, `${id}.json`)).catch(() => null)
      if (old?.sourceUrl !== source) continue
      try { record(id, old.jp, old.chs) } catch { continue }
      jp[id] = previousJp[id]; chs[id] = previousChs[id]; archivedIds.add(id)
    }
    await atomic(join(cache, 'index-jp.json'), JSON.stringify(jp))
    await atomic(join(cache, 'index-chs.json'), JSON.stringify(chs))
    const selected = option('ids', '') ? new Set(String(option('ids')).split(',').map(Number)) : null
    const ids = Object.keys(jp).map(Number).filter(id => chs[id] && isReleased(jp[id]) && isReleased(chs[id]) && (!selected || selected.has(id))).sort((a,b) => a-b)
    if (selected && ids.length !== selected.size) throw new Error('指定任务不在已发布双语目录中')
    const refreshBefore = now() - number('refresh-days', 7) * 86400
    const usable = []
    let consecutiveFailures = 0
    console.log(`目录包含 ${ids.length} 个已发布双语任务`)
    for (let pos = 0; pos < ids.length; pos++) {
      const id = ids[pos], path = join(cache, `${id}.json`)
      let old = await json(path).catch(() => null)
      try { if (old) record(id, old.jp, old.chs) } catch { old = null }
      if (old?.sourceUrl !== source) old = null
      const indexChanged = JSON.stringify(previousJp[id]) !== JSON.stringify(jp[id]) || JSON.stringify(previousChs[id]) !== JSON.stringify(chs[id])
      if (old && (archivedIds.has(id) || old.collectedAt > refreshBefore && !options.has('refresh-all') && !indexChanged)) { usable.push(id); continue }
      try {
        const a = await fetchJson(source, `jp/quest/${id}`, old?.jpValidator)
        const b = await fetchJson(source, `chs/quest/${id}`, old?.chsValidator)
        const pair = record(id, a?.raw ?? old?.jp, b?.raw ?? old?.chs)
        await atomic(path, JSON.stringify({ ...pair, sourceUrl: source, collectedAt: now(), jpValidator: a?.validator ?? old?.jpValidator, chsValidator: b?.validator ?? old?.chsValidator }))
        usable.push(id)
        consecutiveFailures = 0
      } catch (error) {
        consecutiveFailures++
        collectionFailures.push({ questId: id, reason: error.message, retainedPrevious: !!old })
        console.log(`任务 ${id} 获取失败${old ? '，保留旧正文' : ''}：${error.message}`)
        if (old) usable.push(id)
        if (consecutiveFailures >= number('max-consecutive-failures', 5, 1)) {
          console.log('连续获取失败，暂停本轮采集；断点已保留')
          break
        }
      }
      if ((pos + 1) % 25 === 0 || pos === ids.length - 1) console.log(`进度 ${pos + 1}/${ids.length}，失败 ${collectionFailures.length}`)
    }
    if (usable.length !== ids.length) {
      const output = outputPath()
      await atomic(join(dirname(output), 'run-report.json'), JSON.stringify({ changed: false, incomplete: true, questCount: usable.length, expected: ids.length, failures: collectionFailures }, null, 2))
      throw new Error('本轮资源尚未完整，断点已保存；重新执行可补齐失败任务')
    }
    await build(cache, jp, chs, usable)
  } else {
    jp = await json(join(cache, 'index-jp.json'))
    chs = await json(join(cache, 'index-chs.json'))
    const selected = option('ids', '') ? new Set(String(option('ids')).split(',').map(Number)) : null
    const ids = Object.keys(jp).map(Number).filter(id => chs[id] && isReleased(jp[id]) && isReleased(chs[id]) && (!selected || selected.has(id))).sort((a,b) => a-b)
    await build(cache, jp, chs, ids)
  }
}
main().catch(async error => {
  console.error(error.message)
  await atomic(join(dirname(outputPath()), 'run-report.json'), JSON.stringify({ changed: false, incomplete: true, reason: error.message, failures: collectionFailures }, null, 2)).catch(() => {})
  process.exitCode = 1
})
