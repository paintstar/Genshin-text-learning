# 提瓦特 · 语旅

在《原神》的剧情里学习日语。搜索正在进行的任务，一边听游戏语音，一边阅读日中对照台词，遇到新词就查一查、记下来。

## 下载与使用

前往 [Releases](https://github.com/paintstar/Genshin-text-learning/releases/latest) 下载应用。

当前版本为 **v0.2.0**，提供以下安装包：

- **Windows x64**：运行 `GenshinLangLearning_0.2.0_x64-setup.exe`，安装器包含离线 WebView2 运行组件。
- **macOS Apple Silicon（M 系列芯片）**：解压 `GenshinLangLearning-macos-arm64.zip`，将 `GenshinLangLearning.app` 放入「应用程序」后打开。

macOS 版本尚未经过 Apple 公证。首次打开若提示「Apple 无法验证」，请点击「完成」，前往「系统设置 → 隐私与安全性」，找到该应用的提示并点击「仍要打开」，按提示验证后再点击「打开」。详见 [Apple 官方说明](https://support.apple.com/zh-cn/102445)。

剧情、词典和注音资源已包含在应用中，无需另装 Node.js、Rust 或 AI 工具。首次启动会在后台建立书库，完成后输入中文或日文任务名即可搜索和离线阅读。

## 功能

- **剧情书库**：中文、日文任务搜索，按任务类型筛选，本地缓存。
- **双语阅读**：日中对照或单语显示，章节切换、台词搜索、空 / 荧台词切换。
- **跟随游戏**：逐句推进，选择剧情分支，保存阅读进度。
- **假名注音与词典**：点击或划选日文查看读音、原形和词义，支持「食べた → 食べる」等活用还原。
- **学习笔记**：收藏生词和整句，添加自己的理解，返回对应原句。
- **可选语言助手**：连接兼容 OpenAI 的 API 或本地 CLI，获得词义和句子讲解。
- **本地数据**：学习数据本地保存，支持备份与恢复；API 密钥使用系统凭据库。

基础阅读、查词、注音和笔记无需配置 AI。部分词条仅有英文释义，游戏专有名词的自动注音可能存在偏差。

## 剧情资源

v0.2.0 内置完整双语剧情，也支持导入 `.gllpack` 和独立更新剧情。更新过程显示进度，可以取消后重新操作；笔记和阅读进度保留。资源维护仓库 [Genshin-dataset](https://github.com/paintstar/Genshin-dataset) 已公开，应用提供默认更新地址。

基础阅读、查词、注音和笔记可以离线使用；联网更新和在线 AI 功能需要网络。采集与构建方法见 [资源维护说明](app/tools/story-data/README.md)。

## 从源码运行

需要 Node.js 22.13+、Rust stable 和对应平台的 [Tauri 开发环境](https://v2.tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/paintstar/Genshin-text-learning.git
cd Genshin-text-learning/app/frontend
npm ci
```

在 `app/frontend/` 下载完整剧情和词典资源：

```bash
npm run resources:prepare
```

随后启动或构建桌面应用：

```bash
npm run desktop
npm run desktop:build
```

完整开发说明、测试命令和配置方式见 [app/README.md](app/README.md)。原始需求见 [require.md](require.md)。Windows x64 与 macOS Apple Silicon 安装包由 [桌面构建工作流](.github/workflows/desktop.yml) 生成并检查。Linux 和 Intel Mac 尚未提供已验证的安装包。

## 数据来源与许可

这是非官方学习工具，与《原神》及其发行方无关联。剧情来自 [Project Amber](https://gi.yatta.moe)，相关游戏内容权利归原权利人所有。

离线词典使用中文维基词典、JMdict 和 JMnedict，并按 CC BY-SA 4.0 分发。详见 [第三方说明](THIRD_PARTY_NOTICES.md)。
