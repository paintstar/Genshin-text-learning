# 剧情资源维护工具

资源仓库位于 https://github.com/paintstar/Genshin-dataset，目前已公开。此目录提供相同的采集工具，便于从应用源码生成和验证资源；维护端使用的工具及工作流在资源仓库管理。

在项目根目录执行以下命令，采集缓存和生成文件保存在已忽略的 `work/` 目录：

```bash
node app/tools/story-data/story-data.mjs collect --cache-dir app/tools/story-data/work/cache --output app/tools/story-data/work/dist/story.gllpack --refresh-days 30
```

需要 Node.js 22.13 或更新版本。支持 `--source-url` 指定上游、`--ids` 指定任务、`--interval-ms` 设置请求间隔（至少 1000 毫秒），以及 `--refresh-all` 强制复查正文。运行中断后执行相同命令即可继续。连续失败会暂停；已取得的双语任务保留，新增任务未取得完整正文时不发布。目录信息变化会触发正文复查，已收录的历史任务会保留；目录异常缩减时停止发布。

在 `app/` 执行资源检查：

```bash
cargo run -p xtask -- story-pack inspect tools/story-data/work/dist/story.gllpack
```

开发样本通过 `fixture --fixtures app/fixtures` 生成，检查时需要添加 `--allow-fixture`。测试资源不能用于正式发布。

生成的正式包可以在桌面应用的设置页选择导入；也可以复制到 `app/crates/app/resources/story.gllpack` 后执行 `npm run desktop:build`，构建脚本会检查并随安装包分发。开发启动使用 `GLL_STORY_PACK` 指向资源包，首次启动在后台导入，升级时只导入更新的随包快照。默认更新地址为 `https://github.com/paintstar/Genshin-dataset/releases/latest/download/latest.json`，由 `app/resource-sources.json` 管理，可在设置中增加备用地址。清单用于版本和完整性检查，当前依赖所配置 HTTPS 源的可信性；它不是独立的数字签名机制。

导入会先检查整个资源包，然后按任务事务更新剧情，原句变化后保留笔记快照并标记需要重新核对。资源导入不替换个人数据库，不包含设置、密钥和学习笔记。取消或磁盘写入中途失败时，已完成任务保留，重新导入会跳过正文未变的任务。联网更新及随包升级不自动降级，手动导入可指定旧资源。
