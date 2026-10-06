//! AiCache — 请求缓存装饰器（技术设计 §7.4【修订·驳回6】/ 架构决策 10）。
//!
//! 实现 AiClient 端口的**装饰器**，由组合根在装配时包裹真实通道实现——
//! 所有调用一律先经缓存（未命中才转发），终态成功后写缓存；命中时直接产出
//! 带「缓存命中」标注的终态事件。缓存纪律单点存在于装饰器内部：
//! - 键 = SHA256(配置指纹 | 功能 | 提示词模板版本 | system | user)
//!   ——键内直接包含完整请求内容与模板版本；
//! - 仅缓存成功完成的结果（流式正常结束且文本非空；失败/超时/取消/解析异常
//!   一律不落缓存）；
//! - 配置变化自动失效（新指纹组新键，旧键自然不命中）+ 惰性清理旧指纹行；
//! - 无 TTL（语言讲解不过时）；多轮对话每轮 payload 含历史、键天然不同。

use crate::client::{AiClient, AiEvent, AiRequest};
use crate::profile::AiProfileRow;
use async_trait::async_trait;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use shared::AppError;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 计算缓存键（红线：键含配置指纹——同 payload 不同指纹不得命中同一缓存）。
pub fn cache_key(config_fingerprint: &str, feature: &str, tpl_version: &str, system: &str, user: &str) -> String {
    let mut h = Sha256::new();
    h.update(config_fingerprint.as_bytes());
    h.update(b"\x1f");
    h.update(feature.as_bytes());
    h.update(b"\x1f");
    h.update(tpl_version.as_bytes());
    h.update(b"\x1f");
    h.update(system.as_bytes());
    h.update(b"\x1f");
    h.update(user.as_bytes());
    format!("{:x}", h.finalize())
}

pub struct AiCache {
    pub inner: Arc<dyn AiClient>,
    /// 缓存表访问（app.db 的 ai_cache 表）。
    pub conn: Arc<std::sync::Mutex<Connection>>,
}

#[async_trait]
impl AiClient for AiCache {
    async fn ask(
        &self,
        profile: &AiProfileRow,
        secret: Option<String>,
        req: AiRequest,
    ) -> Result<mpsc::Receiver<AiEvent>, AppError> {
        let fp = profile.config_fingerprint.clone().unwrap_or_default();
        let key = cache_key(&fp, &req.feature, &req.prompt_tpl_version, &req.system, &req.user);
        // 查缓存。
        let cached: Option<String> = {
            let conn = self.conn.lock().map_err(|_| AppError::internal("缓存连接锁中毒"))?;
            conn.query_row(
                "SELECT response_text FROM ai_cache WHERE cache_key = ?1",
                [&key],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(crate::q(other)),
            })?
        };
        if let Some(text) = cached {
            let (tx, rx) = mpsc::channel(8);
            tokio::spawn(async move {
                let _ = tx.send(AiEvent::Done { text, cached: true }).await;
            });
            return Ok(rx);
        }
        // 未命中 → 转发真实通道；终态成功后写缓存。
        let conn = self.conn.clone();
        let key = key.clone();
        let feature = req.feature.clone();
        let model = profile.model.clone();
        let mut inner_rx = self.inner.ask(profile, secret, req).await?;
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            while let Some(ev) = inner_rx.recv().await {
                if let AiEvent::Done { text, cached: false } = &ev {
                    // 仅成功完成且非空文本落缓存（失败/超时/取消一律不落）。
                    if !text.trim().is_empty() {
                        if let Ok(conn) = conn.lock() {
                            let _ = conn.execute(
                                "INSERT OR REPLACE INTO ai_cache (cache_key, config_fingerprint, feature, response_text, model, created_at)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                rusqlite::params![key, fp, feature, text, model, crate::now_secs()],
                            );
                        }
                    }
                }
                if tx.send(ev).await.is_err() {
                    break;
                }
            }
        });
        Ok(rx)
    }
}

impl AiCache {
    /// 配置指纹变化时的惰性清理（profile 保存后异步调用）。
    pub fn lazy_cleanup(conn: &Connection, old_fingerprint: &str) -> Result<usize, AppError> {
        let n = conn
            .execute("DELETE FROM ai_cache WHERE config_fingerprint = ?1", [old_fingerprint])
            .map_err(crate::q)?;
        Ok(n)
    }

    /// 全量清空（设置页「清空 AI 缓存」）。
    pub fn clear_all(conn: &Connection) -> Result<(), AppError> {
        conn.execute("DELETE FROM ai_cache", []).map_err(crate::q)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE ai_cache(cache_key TEXT PRIMARY KEY, config_fingerprint TEXT NOT NULL, feature TEXT NOT NULL, response_text TEXT NOT NULL, model TEXT, created_at INTEGER NOT NULL);",
        )
        .unwrap();
        c
    }

    #[test]
    fn cache_key_includes_fingerprint_and_payload() {
        // 红线：配置指纹入键——同 payload 不同指纹 → 不同键（无陈旧命中）。
        let a = cache_key("fp1", "ctx_parse", "v1", "sys", "user");
        let b = cache_key("fp2", "ctx_parse", "v1", "sys", "user");
        assert_ne!(a, b);
        // 模板版本入键。
        let c = cache_key("fp1", "ctx_parse", "v2", "sys", "user");
        assert_ne!(a, c);
        // 完整请求内容入键。
        let d = cache_key("fp1", "ctx_parse", "v1", "sys", "user2");
        assert_ne!(a, d);
        // 同参确定。
        assert_eq!(a, cache_key("fp1", "ctx_parse", "v1", "sys", "user"));
    }

    #[tokio::test]
    async fn decorator_hits_and_writes_cache() {
        use crate::client::StubClient;
        let c = Arc::new(std::sync::Mutex::new(conn()));
        let cache = AiCache {
            inner: Arc::new(StubClient::of(vec![
                AiEvent::Delta("答".into()),
                AiEvent::Done { text: "答案".into(), cached: false },
            ])),
            conn: c.clone(),
        };
        let profile = AiProfileRow {
            id: 1,
            name: "t".into(),
            channel: "http".into(),
            cli_kind: None,
            command_path: None,
            base_url: Some("http://x/v1".into()),
            api_key_ref: None,
            model: "m".into(),
            extra_json: None,
            cli_version: None,
            config_fingerprint: Some("fp1".into()),
            is_active: true,
        };
        let req = AiRequest {
            system: "s".into(),
            user: "u".into(),
            feature: "f".into(),
            prompt_tpl_version: "v1".into(),
            timeout_secs: 5,
            max_tokens: None,
        };
        // 第一次：未命中 → 转发 + 落缓存。
        let mut rx = cache.ask(&profile, None, req.clone()).await.unwrap();
        while let Some(ev) = rx.recv().await {
            if matches!(ev, AiEvent::Done { .. }) {
                break;
            }
        }
        let n: i64 = c.lock().unwrap()
            .query_row("SELECT COUNT(*) FROM ai_cache", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        // 第二次：命中 → cached=true 终态、不调用内层。
        let mut rx = cache.ask(&profile, None, req).await.unwrap();
        let mut done = None;
        while let Some(ev) = rx.recv().await {
            if let AiEvent::Done { text, cached } = ev {
                done = Some((text, cached));
                break;
            }
        }
        let (text, cached) = done.unwrap();
        assert_eq!(text, "答案");
        assert!(cached, "命中应带缓存标注");
    }

    #[test]
    fn lazy_cleanup_removes_old_fingerprint_rows() {
        let c = conn();
        c.execute(
            "INSERT INTO ai_cache VALUES ('k1','fp-old','f','t','m',0), ('k2','fp-new','f','t','m',0)",
            [],
        )
        .unwrap();
        assert_eq!(AiCache::lazy_cleanup(&c, "fp-old").unwrap(), 1);
        let n: i64 = c.query_row("SELECT COUNT(*) FROM ai_cache", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }
}
