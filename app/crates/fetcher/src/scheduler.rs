//! FetchScheduler — 限速调度器（技术设计 §2.2 / 架构 §3.2）。
//!
//! 全部出站抓取的串行队列：请求间隔下限、指数退避重试（最多 3 次）、
//! 并发为一。组合根创建单实例注入各 QuestSource 适配器（yatta 与未来
//! lunaris 共用），位于适配器内部——端口调用方经 QuestSource 发起的一切
//! 请求自然经它排队，不可见亦不可绕开。独立成对象的理由：限速必须单点
//! 强制而非各调用处自觉。

use shared::AppError;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;

pub struct FetchScheduler {
    min_interval: Duration,
    /// 串行化令牌：任意时刻至多一个在途请求。
    serial: AsyncMutex<()>,
    last_start: Mutex<Option<Instant>>,
    /// 测试用：不做真实 sleep（由 tokio 时钟测试或注入控制）。
    fast_mode: bool,
}

impl FetchScheduler {
    pub fn new(min_interval_ms: u64) -> Self {
        Self {
            min_interval: Duration::from_millis(min_interval_ms),
            serial: AsyncMutex::new(()),
            last_start: Mutex::new(None),
            fast_mode: false,
        }
    }

    pub fn for_tests() -> Self {
        Self {
            min_interval: Duration::from_millis(0),
            serial: AsyncMutex::new(()),
            last_start: Mutex::new(None),
            fast_mode: true,
        }
    }

    /// 在串行锁与间隔下限约束内执行一次请求闭包（含重试）。
    pub async fn run<T, E, F, Fut>(&self, mut f: F) -> Result<T, AppError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: Into<AppError> + Clone,
    {
        let _guard = self.serial.lock().await;
        let mut attempt = 0u32;
        loop {
            // 间隔下限：距上次请求开始至少 min_interval。
            let wait = {
                let mut last = self.last_start.lock().unwrap();
                let wait = last
                    .map(|t| {
                        let elapsed = t.elapsed();
                        if elapsed < self.min_interval && !self.fast_mode {
                            Some(self.min_interval - elapsed)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(None);
                *last = Some(Instant::now());
                wait
            };
            if let Some(w) = wait {
                tokio::time::sleep(w).await;
            }
            match f().await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    let app_err: AppError = e.into();
                    // 仅对网络/5xx 类错误重试；数据完整性/取消等不重试。
                    let retryable = matches!(
                        app_err.kind,
                        shared::AppErrorKind::Network | shared::AppErrorKind::DataSource
                    ) && !app_err.message.contains("M0");
                    attempt += 1;
                    if retryable && attempt < 3 {
                        let backoff = Duration::from_millis(1000u64.pow(attempt));
                        if !self.fast_mode {
                            tokio::time::sleep(backoff).await;
                        }
                        continue;
                    }
                    return Err(app_err);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn retries_then_fails() {
        let s = FetchScheduler::for_tests();
        let count = AtomicUsize::new(0);
        let r: Result<(), AppError> = s
            .run(|| async {
                count.fetch_add(1, Ordering::SeqCst);
                Err::<(), shared::AppError>(AppError::network("boom"))
            })
            .await;
        assert!(r.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 3, "最多重试 3 次");
    }

    #[tokio::test]
    async fn serializes_requests() {
        let s = std::sync::Arc::new(FetchScheduler::for_tests());
        let mut handles = Vec::new();
        let active = std::sync::Arc::new(AtomicUsize::new(0));
        let max_active = std::sync::Arc::new(AtomicUsize::new(0));
        for _ in 0..8 {
            let s = s.clone();
            let active = active.clone();
            let max_active = max_active.clone();
            handles.push(tokio::spawn(async move {
                s.run(|| {
                    let active = active.clone();
                    let max_active = max_active.clone();
                    async move {
                        let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
                        max_active.fetch_max(cur, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(5)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok::<(), shared::AppError>(())
                    }
                })
                .await
                .unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        assert_eq!(max_active.load(Ordering::SeqCst), 1, "并发为一");
    }
}
