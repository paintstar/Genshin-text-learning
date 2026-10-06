//! `fetcher` — 数据源接入 crate（架构 §2.2/§3.2）。
//!
//! - `TermsGate`（M0 门禁红线）：技术设计 §2.7【修订·完善4】——开发期首次实装
//!   抓取即构成「接入」，`gi.yatta.moe/api/AGENTS.md` 条款的人工（浏览器）核对
//!   为任何抓取发生前的硬门禁。本机网络实测该路径对非浏览器客户端 403（含
//!   伪装浏览器 UA），无法代读——必须由用户在设置页确认「已完成条款核对」
//!   （写 `fetch.terms_accepted_at`）后，TermsGate 才放行任何出站请求。
//! - `FetchScheduler`：全部出站抓取的串行队列——请求间隔下限（默认 ≥1s）、
//!   指数退避重试（最多 3 次）、并发为一。归属本 crate（礼貌抓取是接入层
//!   横切约束）；组合根创建单实例注入 QuestSource 适配器，位于适配器内部。
//! - `YattaClient`：QuestSource 端口的 yatta 适配器（诚实 UA、条件请求、
//!   自定义请求头高级设置）。仅产出原始响应字节 + HTTP 元数据 DTO。

mod scheduler;
mod terms;
mod yatta;

pub use scheduler::FetchScheduler;
pub use terms::TermsGate;
pub use yatta::YattaClient;

pub const USER_AGENT: &str = concat!("GenshinLangLearning/", env!("CARGO_PKG_VERSION"));
pub const DEFAULT_BASE_URL: &str = "https://gi.yatta.moe/api/v2";
