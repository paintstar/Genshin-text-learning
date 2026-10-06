#!/usr/bin/env node
/**
 * tools/build-dict — 词典构建管线（技术设计 §4.2；构建期产物，不进用户机器由
 * 本脚本外发）。源数据按「版本锁定」下载（URL 清单见 sources.mjs）。
 *
 * 子命令：
 *   build           下载三源 → 抽取 → 合并生成 dict.db（ShareAlike：元数据表 +
 *                   attribution 信息写入；journal_mode 置 DELETE 收尾）
 *   build --fixture 用 fixtures/ 种子数据构建小型 dict.db（开发/测试用）
 *   audit           构建期质量评估（≥3 类任务 ≥2000 句语料 → 常用词中文命中率
 *                   ≥90% / 活用还原 ≥85% 门槛报告；报告随产物归档）
 *   audit --fixture 以 fixture 语料跑 audit 流程（自测）
 */

import { DatabaseSync } from 'node:sqlite'
import { readFileSync, writeFileSync, existsSync, mkdirSync, createWriteStream, createReadStream } from 'node:fs'
import { unlink, rename } from 'node:fs/promises'
import { pipeline } from 'node:stream/promises'
import { Readable } from 'node:stream'
import { createInterface } from 'node:readline'
import { gunzipSync, createGunzip } from 'node:zlib'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { SOURCES, APP_VERSION } from './sources.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const outDir = path.resolve(here, '../../crates/app/resources')

function log(...a) {
  console.log('[build-dict]', ...a)
}

async function download(url, dest) {
  log('下载', url)
  const res = await fetch(url, { headers: { 'User-Agent': `GenshinLangLearning/${APP_VERSION} dict-builder` } })
  if (!res.ok) throw new Error(`下载失败 ${res.status}: ${url}`)
  const part = dest + '.part'
  await pipeline(Readable.fromWeb(res.body), createWriteStream(part))
  await rename(part, dest)
  log('→', dest)
}

/** 解析 zhwiktionary（kaikki Wiktextract）JSONL 行 → 词条抽取。 */
function extractZhwiktionary(line) {
  let v
  try {
    v = JSON.parse(line)
  } catch {
    return null
  }
  if (v.lang_code !== 'ja') return null
  const headword = v.word
  if (!headword) return null
  const glosses = []
  for (const sense of v.senses ?? []) {
    for (const g of sense.glosses ?? []) {
      if (typeof g === 'string' && g.trim()) glosses.push(g.trim())
    }
  }
  if (glosses.length === 0) return null
  const readings = []
  for (const s of (v.sounds ?? [])) {
    if (s.hira) readings.push(s.hira)
    if (s.kana) readings.push(s.kana)
    if (s.other && /^[\u3040-\u30ffー]+$/.test(s.other)) readings.push(s.other)
  }
  const pos = (v.pos ?? []) .length ? [v.pos] : []
  return { headword, readings: [...new Set(readings)], pos, glosses: { zh: glosses }, source: 'zhwiktionary' }
}

/** 解析 jmdict-simplified JSON 词条。 */
function extractJmdictEntry(e, sourceName) {
  const kanji = (e.kanji ?? []).map((k) => k.text)
  const kana = (e.kana ?? []).map((k) => k.text)
  const pos = (e.sense ?? e.senses ?? []).flatMap((s) => s.partOfSpeech ?? [])
  const common = [...(e.kanji ?? []), ...(e.kana ?? [])].some(k => k.common) || Boolean(e.common)
  const glossEn = sourceName === 'jmnedict'
    ? (e.translation ?? []).flatMap(s => (s.translation ?? []).filter(g => !g.lang || g.lang === 'eng').map(g => g.text))
    : (e.sense ?? e.senses ?? []).flatMap(s => (s.gloss ?? []).map(g => typeof g === 'string' ? g : g.text)).filter(Boolean)
  const out = []
  for (const hw of [kanji[0] || kana[0]].filter(Boolean)) {
    out.push({
      headword: hw,
      readings: kana,
      alternativeForms: kanji,
      pos: [...new Set(pos)],
      glosses: glossEn.length ? { en: glossEn } : {},
      source: sourceName,
      common,
      formKind: kanji.includes(hw) ? 'kanji' : 'kana',
    })
  }
  return out
}

function extractJmdict(text, sourceName) {
  const data = JSON.parse(text)
  const list = sourceName === 'jmnedict' ? data.words ?? [] : data.words ?? []
  const out = []
  for (const e of list) out.push(...extractJmdictEntry(e, sourceName))
  return out
}

function createSchema(db) {
  db.exec(`
PRAGMA journal_mode = DELETE;
CREATE TABLE IF NOT EXISTS dict_entry (
  entry_id INTEGER PRIMARY KEY,
  source TEXT NOT NULL,
  headword TEXT NOT NULL,
  reading_kana TEXT,
  pos_json TEXT NOT NULL DEFAULT '[]',
  common INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS dict_index (
  form TEXT NOT NULL,
  form_kind TEXT NOT NULL,
  entry_id INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS dict_gloss (
  entry_id INTEGER NOT NULL,
  lang TEXT NOT NULL,
  gloss_json TEXT NOT NULL,
  PRIMARY KEY (entry_id, lang)
);
CREATE INDEX IF NOT EXISTS idx_dict_index_form ON dict_index(form);
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
`)
}

const statements = new WeakMap()
function writeEntry(db, e) {
  const forms = new Map()
  if (e.formKind) forms.set(e.headword, e.formKind)
  else forms.set(e.headword, /[\u4e00-\u9faf]/.test(e.headword) ? 'kanji' : 'kana')
  for (const r of e.readings ?? []) if (r && r !== '*') forms.set(r, 'kana')
  for (const form of e.alternativeForms ?? []) forms.set(form, 'kanji')
  const glossLangs = Object.keys(e.glosses ?? {})
  if (glossLangs.length === 0) return
  if (!statements.has(db)) statements.set(db, {
    idStmt: db.prepare('INSERT INTO dict_entry (source, headword, reading_kana, pos_json, common) VALUES (?, ?, ?, ?, ?)'),
    idxStmt: db.prepare('INSERT INTO dict_index (form, form_kind, entry_id) VALUES (?, ?, ?)'),
    glossStmt: db.prepare('INSERT OR REPLACE INTO dict_gloss (entry_id, lang, gloss_json) VALUES (?, ?, ?)'),
  })
  const { idStmt, idxStmt, glossStmt } = statements.get(db)
  const readingKana = (e.readings ?? [])[0] ?? null
  const r = idStmt.run(e.source, e.headword, readingKana, JSON.stringify(e.pos ?? []), e.common ? 1 : 0)
  const entryId = Number(r.lastInsertRowid)
  for (const [form, kind] of forms) idxStmt.run(form, kind, entryId)
  for (const lang of glossLangs) glossStmt.run(entryId, lang, JSON.stringify(e.glosses[lang].slice(0, 12)))
}

function writeMetadata(db, entriesBySource) {
  const meta = db.prepare('INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)')
  meta.run('app_version', APP_VERSION)
  for (const s of SOURCES) {
    meta.run(`${s.name}.name`, s.title)
    meta.run(`${s.name}.url`, s.url)
    meta.run(`${s.name}.version`, s.version)
    meta.run(`${s.name}.license`, s.license)
    meta.run(`${s.name}.license_url`, s.licenseUrl)
    meta.run(`${s.name}.entries`, String(entriesBySource.get(s.name) ?? 0))
  }
  meta.run('sharealike_statement', 'dict.db 为 zhwiktionary(CC BY-SA 4.0/GFDL 双许可) 与 JMdict/JMnedict(CC BY-SA 4.0) 的改编合并数据集，按 CC BY-SA 4.0 再分发；构建脚本与源版本锁定随应用公开。')
}

async function build(fixture) {
  mkdirSync(outDir, { recursive: true })
  const dbPath = path.join(outDir, 'dict.db')
  const tmpPath = dbPath + '.tmp'
  if (existsSync(tmpPath)) await unlink(tmpPath)
  const db = new DatabaseSync(tmpPath)
  createSchema(db)
  db.exec('BEGIN')
  const counts = new Map()

  if (fixture) {
    const dir = path.join(here, 'fixtures')
    // zhwiktionary 形状的种子数据。
    const zhLines = readFileSync(path.join(dir, 'zhwiktionary-seed.jsonl'), 'utf8').trim().split('\n')
    let n = 0
    for (const line of zhLines) {
      const e = extractZhwiktionary(line)
      if (e) {
        writeEntry(db, e)
        n++
      }
    }
    counts.set('zhwiktionary', n)
    // JMdict 形状的种子数据。
    const jm = JSON.parse(readFileSync(path.join(dir, 'jmdict-seed.json'), 'utf8'))
    let m = 0
    for (const e of extractJmdict(JSON.stringify(jm), 'jmdict')) {
      writeEntry(db, e)
      m++
    }
    counts.set('jmdict', m)
  } else {
    const dl = path.join(here, 'downloads')
    mkdirSync(dl, { recursive: true })
    // zhwiktionary
    const zhGz = path.join(dl, 'zhwiktionary.jsonl.gz')
    if (!existsSync(zhGz)) await download(SOURCES[0].url, zhGz)
    let n = 0
    const lines = createInterface({ input: createReadStream(zhGz).pipe(createGunzip()), crlfDelay: Infinity })
    for await (const line of lines) {
      const e = extractZhwiktionary(line)
      if (e) { writeEntry(db, e); n++ }
    }
    counts.set('zhwiktionary', n)
    log('中文释义词条', n)
    const release = await fetch('https://github.com/scriptin/jmdict-simplified/releases/latest')
    if (!release.ok) throw new Error('无法获取最新词典版本')
    const version = decodeURIComponent(release.url.split('/').pop())
    await release.body?.cancel()
    if (!/^\d+\.\d+\.\d+\+\d+$/.test(version)) throw new Error('未识别词典发布版本')
    for (const s of SOURCES.slice(1)) {
      s.version = version
      const filename = `${s.name}-${s.name === 'jmnedict' ? 'all' : 'eng'}-${version}.json.tgz`
      s.url = `https://github.com/scriptin/jmdict-simplified/releases/download/${encodeURIComponent(version)}/${filename}`
      const f = path.join(dl, filename)
      if (!existsSync(f)) await download(s.url, f)
      // 官方产物是 tar.gz，读取其中的 JSON（不依赖系统 tar 或绝对路径）。
      const tar = gunzipSync(readFileSync(f))
      let found = false; let count = 0
      for (let offset = 0; offset + 512 <= tar.length;) {
        const name = tar.toString('utf8', offset, offset + 100).replace(/\0.*$/, '')
        if (!name) break
        const size = parseInt(tar.toString('ascii', offset + 124, offset + 136).replace(/\0.*$/, '').trim(), 8) || 0
        if (name.endsWith('.json')) {
          const data = JSON.parse(tar.toString('utf8', offset + 512, offset + 512 + size))
          for (const word of data.words ?? []) {
            const entries = extractJmdictEntry(word, s.name)
            for (const entry of entries) { if (Object.keys(entry.glosses).length) { writeEntry(db, entry); count++ } }
          }
          found = true; break
        }
        offset += 512 + Math.ceil(size / 512) * 512
      }
      if (!found || !count) throw new Error(`${s.name} 未提取到有效词条，保留旧词典`)
      counts.set(s.name, count)
      log(s.name, count)
    }
  }

  writeMetadata(db, counts)
  db.prepare('INSERT OR REPLACE INTO meta (key,value) VALUES (?,?)').run('build_mode', fixture ? 'fixture' : 'full')
  db.exec('COMMIT')
  db.close()
  await rename(tmpPath, dbPath)
  log('dict.db 完成 →', dbPath, Object.fromEntries(counts))
  return counts
}

/** audit：语料分词 → 生产查询链 → 中文命中率/活用还原命中率。 */
async function audit(fixture) {
  const dictPath = path.join(outDir, 'dict.db')
  const db = new DatabaseSync(dictPath, { readOnly: true })
  const corpusDir = fixture ? path.join(here, 'fixtures') : path.join(here, 'corpus')
  const sentences = []
  for (const f of existsSync(corpusDir) ? readdirSafe(corpusDir) : []) {
    if (!f.endsWith('.txt')) continue
    for (const line of readFileSync(path.join(corpusDir, f), 'utf8').split('\n')) {
      if (line.trim()) sentences.push(line.trim())
    }
  }
  // kuromoji 分词（与前端同一分支库；经 frontend/node_modules 绝对路径加载）。
  const kmPath = path.resolve(here, '../../frontend/node_modules/@wwzzyying/kuromoji/src/kuromoji.js')
  const { pathToFileURL } = await import('node:url')
  const kuromojiMod = await import(pathToFileURL(kmPath).href)
  const kuromoji = kuromojiMod.default ?? kuromojiMod
  const dictDir = path.resolve(here, '../../frontend/node_modules/@wwzzyying/kuromoji/dict')
  const tokenizer = await new Promise((resolve, reject) =>
    kuromoji.builder({ dicPath: dictDir }).build((err, tok) => (err ? reject(err) : resolve(tok))),
  )
  const freq = new Map()
  let nonBaseSamples = []
  for (const s of sentences) {
    for (const t of tokenizer.tokenize(s)) {
      const base = t.basic_form && t.basic_form !== '*' ? t.basic_form : t.surface_form
      freq.set(base, (freq.get(base) ?? 0) + 1)
      if (t.surface_form !== base && nonBaseSamples.length < 300) {
        nonBaseSamples.push({ surface: t.surface_form, base, reading: t.reading })
      }
    }
  }
  const top = [...freq.entries()].sort((a, b) => b[1] - a[1]).slice(0, 2000)
  const queryZh = db.prepare(
    `SELECT EXISTS(SELECT 1 FROM dict_index i JOIN dict_gloss g ON g.entry_id = i.entry_id AND g.lang='zh' WHERE i.form = ?) AS hit`,
  )
  let zhHit = 0
  const zhMiss = []
  for (const [w] of top) {
    if (Number(queryZh.get(w)?.hit ?? 0) > 0) zhHit++
    else zhMiss.push(w)
  }
  let baseHit = 0
  for (const s of nonBaseSamples) {
    if (Number(queryZh.get(s.surface)?.hit ?? 0) > 0 || Number(queryZh.get(s.base)?.hit ?? 0) > 0) baseHit++
  }
  const report = {
    generatedAt: new Date().toISOString(),
    corpus: { sentences: sentences.length, modes: fixture ? ['fixture'] : ['corpus/'] },
    topZhCoverage: { hit: zhHit, total: top.length, rate: top.length ? +(zhHit / top.length).toFixed(4) : null, threshold: 0.9 },
    conjugationCoverage: { hit: baseHit, total: nonBaseSamples.length, rate: nonBaseSamples.length ? +(baseHit / nonBaseSamples.length).toFixed(4) : null, threshold: 0.85 },
    missSample: zhMiss.slice(0, 50),
  }
  const reportPath = path.join(outDir, 'dict-audit-report.json')
  writeFileSync(reportPath, JSON.stringify(report, null, 2))
  log('audit 报告 →', reportPath, JSON.stringify(report.topZhCoverage), JSON.stringify(report.conjugationCoverage))
  return report
}

function readdirSafe(dir) {
  try {
    return readdirSyncShim(dir)
  } catch {
    return []
  }
}
import { readdirSync as readdirSyncShim } from 'node:fs'

const cmd = process.argv[2]
const fixture = process.argv.includes('--fixture')
switch (cmd) {
  case 'build':
    await build(fixture)
    break
  case 'audit':
    await audit(fixture)
    break
  default:
    console.log('用法: node build-dict.mjs <build|audit> [--fixture]')
}
