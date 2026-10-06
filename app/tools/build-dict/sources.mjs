/** 词典源清单与版本锁定（技术设计 §4.2-6：构建脚本与源数据版本锁定随发布公开）。 */
export const APP_VERSION = '0.1.0'

export const SOURCES = [
  {
    name: 'zhwiktionary',
    title: 'zhwiktionary（kaikki.org Wiktextract 提取版）',
    url: 'https://kaikki.org/dictionary/downloads/zh/zh-extract.jsonl.gz',
    version: 'latest-at-build（构建时写入 meta 表）',
    license: 'CC BY-SA 4.0（与 GFDL 双许可，本项目按 CC BY-SA 4.0 履行）',
    licenseUrl: 'https://creativecommons.org/licenses/by-sa/4.0/',
  },
  {
    name: 'jmdict',
    title: 'JMdict（jmdict-simplified）',
    url: 'https://github.com/scriptin/jmdict-simplified/releases/latest',
    version: 'latest-at-build',
    license: 'CC BY-SA 4.0（EDRDG）',
    licenseUrl: 'https://www.edrdg.org/edrdg/licence.html',
  },
  {
    name: 'jmnedict',
    title: 'JMnedict（jmdict-simplified）',
    url: 'https://github.com/scriptin/jmdict-simplified/releases/latest',
    version: 'latest-at-build',
    license: 'CC BY-SA 4.0（EDRDG）',
    licenseUrl: 'https://www.edrdg.org/edrdg/licence.html',
  },
]
