# Repository Guidelines

## 项目结构与模块边界

本项目使用 Tauri 2、Vue 3、TypeScript、Pinia 和 SQLite。原始需求见 `require.md`，开发说明见 `app/README.md`。

- `app/frontend/src/`：`views/` 存放页面，`components/` 存放组件，`stores/` 管理状态，`modules/` 实现阅读与分词，`gateway/` 封装桌面通信。
- `app/crates/`：`app` 负责桌面入口与服务编排；`kb`、`fetcher` 负责剧情；`dict`、`study` 负责词典与学习数据；`store`、`ai`、`shared` 提供基础能力。
- `app/fixtures/` 保存剧情测试样本；Rust 集成测试位于各 crate 的 `tests/`，前端测试与模块相邻。
- 图标位于 `app/crates/app/icons/`，静态资源位于 `app/frontend/public/`，词典构建工具位于 `app/tools/build-dict/`。

## 开发、构建与检查

准备 Node.js 22.13+、Rust stable 和对应平台的 Tauri 开发环境。从 Release 下载词典，将 `dict.db` 放入 `app/crates/app/resources/`；或在 `app/` 执行 `node tools/build-dict/build-dict.mjs build`。

以下命令在 `app/frontend/` 执行：

| 命令 | 用途 |
| --- | --- |
| `npm ci` | 按锁文件安装依赖并复制注音词典 |
| `npm run desktop` | 同时启动前端与桌面开发程序 |
| `npm run dev` | 启动浏览器预览；添加 `?mock=1` 使用示例数据 |
| `npm run typecheck` | 检查 TypeScript 与 Vue 类型 |
| `npm run test` | 运行 Vitest 测试 |
| `npm run build` | 类型检查并构建前端 |
| `npm run desktop:build` | 构建桌面发布包 |

在 `app/` 执行 `cargo test --workspace` 运行 Rust 测试，使用 `cargo fmt --all` 格式化 Rust。

## 代码风格与命名

Vue 与 TypeScript 沿用两空格缩进、单引号和无分号风格；组件使用 `PascalCase.vue`，函数与变量使用 `camelCase`。Rust 使用四空格缩进、`snake_case` 函数与模块名，以及 `PascalCase` 类型名。仓库未配置独立 lint 命令。

数据访问沿用 gateway 与后端服务边界。修改共享 DTO 后，在 `app/` 执行 `cargo run -p xtask -- bindings`，再以 `--check` 校验生成结果。优先复用现有模块，使用相对路径；必要的硬编码应说明理由。

## 测试要求

Vitest 用例命名为 `*.test.ts`；Rust 使用 `#[test]`、`#[tokio::test]` 和 crate 集成测试。当前没有覆盖率百分比门槛。先完成主线功能，再为行为修复补充针对性回归测试，重点检查双语对齐、下载事件、注音、笔记定位和持久化。界面变更应实际检查桌面交互与窄窗口布局。

## 提交与 Pull Request

目前历史只有中文发布提交，例如 `发布 v0.1.0：提瓦特语旅首个可用版本`，尚无固定前缀规范。提交说明应简短、具体。PR 应描述问题、改动、验证结果与已知限制，关联相关 issue；界面改动附截图。创建新分支前必须取得用户许可并通知用户。

## 安全与协作

优先使用用户已有配置。不要提交或输出密钥、`.env`、个人学习数据库和备份；完整词典与安装包通过 Release 分发。API 密钥使用系统凭据库，开发可用 `GLL_DATA_DIR` 和 `GLL_DICT_DB` 指定独立资源。发布包使用完整词典，`--fixture` 仅用于测试。

沟通使用简体中文。长时间任务应独立后台运行；结束进程前确认属于本次任务。修改本文件或其他重要说明后，复查命令、路径与实际实现是否一致。
