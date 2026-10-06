# 第三方说明

## 剧情文本

剧情文本由 [Project Amber](https://gi.yatta.moe) 提供，游戏及其文本、角色等内容权利归原权利人所有。本项目是非官方语言学习工具。

## 离线词典

本项目把以下词典提取、整理为可离线查询的 SQLite 数据库。修改包括筛选日语词条、整理读音和释义、建立检索索引及合并查询结果。

- **中文维基词典**：通过 [Kaikki / Wiktextract](https://kaikki.org/zhwiktionary/) 获取，由维基词典贡献者编写。改编数据按 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) 分发。
- **JMdict / JMnedict**：由 Electronic Dictionary Research and Development Group 及其贡献者维护，通过 [jmdict-simplified](https://github.com/scriptin/jmdict-simplified) 获取。本版使用 `3.6.2+20261005200550`，遵循 [EDRDG 许可说明](https://www.edrdg.org/edrdg/licence.html)和 CC BY-SA 4.0。

本项目的改编词典 `dict.db` 按 CC BY-SA 4.0 分发。词典元数据保留来源和许可信息，构建脚本位于 `app/tools/build-dict/`。此许可针对词典数据，不改变其他独立代码或内容的许可。

## 分词与注音

分词使用 [kuromoji.js](https://github.com/takuyaa/kuromoji.js) 的 `@wwzzyying/kuromoji` 分支，遵循 Apache License 2.0。该库使用的 IPADIC 字典保留其原始版权及许可条款。相关声明见 [Kuromoji NOTICE](app/licenses/kuromoji-NOTICE.md) 和 [Apache License 2.0](app/licenses/Apache-2.0.txt)。

## 软件依赖

应用基于 Tauri、Vue、Naive UI、Pinia、Vue Router、SQLite 等项目构建。各依赖保持各自许可；准确版本记录于 `app/Cargo.lock` 和 `app/frontend/package-lock.json`。发布包随附依赖的许可文本。

`selectors 0.38.0` 按 MPL 2.0 使用，未经修改的对应源代码可从 [crates.io](https://crates.io/api/v1/crates/selectors/0.38.0/download) 获取，许可全文随附于 `app/licenses/MPL-2.0.txt`。Objective-C 绑定相关的 objc2、block2、dispatch2 组件保留其 MIT / Zlib / Apache 许可文本；alloc-stdlib 保留 Dropbox 的 BSD 3-Clause 许可文本。
