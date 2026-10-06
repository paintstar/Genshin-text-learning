// 复制 kuromoji IPADIC 词典到 public/dict（随应用打包为静态资源）。
import { cpSync, existsSync, mkdirSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const src = resolve(here, '../node_modules/@wwzzyying/kuromoji/dict')
const dest = resolve(here, '../public/dict')

if (!existsSync(src)) {
  console.error(`kuromoji 词典不存在：${src}（先 npm install）`)
  process.exit(1)
}
mkdirSync(dest, { recursive: true })
cpSync(src, dest, { recursive: true })
console.log(`已复制 IPADIC 词典 → ${dest}`)
