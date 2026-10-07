//! 窗口尺寸存于应用自己的设置库，使用逻辑像素适配不同屏幕缩放。
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use store::{SettingsKvStore, Store};
use tauri::{LogicalSize, Manager, WebviewWindow, WindowEvent};

const SETTING_KEY: &str = "ui.window_state";

#[derive(Clone, Copy, Deserialize, Serialize)]
struct WindowSize {
    width: f64,
    height: f64,
    #[serde(default)]
    maximized: bool,
}

impl WindowSize {
    fn bounded(self, minimum: LogicalSize<f64>, available: LogicalSize<f64>) -> Self {
        let width = if self.width.is_finite() && self.width > 0.0 {
            self.width
        } else {
            minimum.width
        };
        let height = if self.height.is_finite() && self.height > 0.0 {
            self.height
        } else {
            minimum.height
        };
        Self {
            width: width.clamp(minimum.width.min(available.width), available.width),
            height: height.clamp(minimum.height.min(available.height), available.height),
            maximized: self.maximized,
        }
    }
}

#[derive(Clone)]
pub struct WindowPreferences {
    size: Arc<Mutex<WindowSize>>,
    revision: Arc<AtomicU64>,
    store: Arc<Store>,
}

impl WindowPreferences {
    pub fn save(&self) {
        let size = *self.size.lock().unwrap();
        if let Ok(json) = serde_json::to_string(&size) {
            let _ = self
                .store
                .with_write(|c| SettingsKvStore::set(c, SETTING_KEY, &json));
        }
    }

    pub fn install(window: WebviewWindow, store: Arc<Store>) -> tauri::Result<Self> {
        let config = window
            .app_handle()
            .config()
            .app
            .windows
            .iter()
            .find(|config| config.label == window.label());
        let current = window
            .inner_size()?
            .to_logical::<f64>(window.scale_factor()?);
        let minimum = LogicalSize::new(
            config.and_then(|c| c.min_width).unwrap_or(1.0),
            config.and_then(|c| c.min_height).unwrap_or(1.0),
        );
        let saved = store
            .with_read(|c| SettingsKvStore::get(c, SETTING_KEY))
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str::<WindowSize>(&json).ok())
            .unwrap_or(WindowSize {
                width: current.width,
                height: current.height,
                maximized: false,
            });
        let mut restored = saved;
        if let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) {
            let area = monitor
                .work_area()
                .size
                .to_logical::<f64>(window.scale_factor()?);
            let outer = window
                .outer_size()?
                .to_logical::<f64>(window.scale_factor()?);
            let available = LogicalSize::new(
                (area.width - (outer.width - current.width).max(0.0)).max(1.0),
                (area.height - (outer.height - current.height).max(0.0)).max(1.0),
            );
            restored = saved.bounded(minimum, available);
            // 小屏幕上配置的最小尺寸也可能大于可用区域。
            window.set_min_size(Some(LogicalSize::new(
                minimum.width.min(available.width),
                minimum.height.min(available.height),
            )))?;
        }
        window.set_size(LogicalSize::new(restored.width, restored.height))?;
        window.center()?;
        if restored.maximized {
            window.maximize()?;
        }
        let preferences = Self {
            size: Arc::new(Mutex::new(restored)),
            revision: Arc::new(AtomicU64::new(0)),
            store,
        };
        let tracked = window.clone();
        let prefs = preferences.clone();
        window.on_window_event(move |event| match event {
            WindowEvent::Resized(size) => {
                if size.width == 0
                    || size.height == 0
                    || tracked.is_minimized().unwrap_or(true)
                    || tracked.is_fullscreen().unwrap_or(true)
                {
                    return;
                }
                let maximized = tracked.is_maximized().unwrap_or(false);
                {
                    let mut current = prefs.size.lock().unwrap();
                    current.maximized = maximized;
                    if !maximized {
                        let logical = size.to_logical::<f64>(tracked.scale_factor().unwrap_or(1.0));
                        current.width = logical.width;
                        current.height = logical.height;
                    }
                }
                // 拖动时只保留最后一次尺寸，停稳后写入，避免每一帧都写数据库。
                let revision = prefs.revision.fetch_add(1, Ordering::SeqCst) + 1;
                let pending = prefs.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    if pending.revision.load(Ordering::SeqCst) == revision {
                        pending.save();
                    }
                });
            }
            WindowEvent::CloseRequested { .. } => prefs.save(),
            _ => {}
        });
        Ok(preferences)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_saved_size_to_smaller_display() {
        let saved = WindowSize {
            width: 1800.0,
            height: 1200.0,
            maximized: false,
        };
        let size = saved.bounded(
            LogicalSize::new(960.0, 640.0),
            LogicalSize::new(800.0, 560.0),
        );
        assert_eq!((size.width, size.height), (800.0, 560.0));
    }

    #[test]
    fn valid_size_is_preserved_and_invalid_size_is_bounded() {
        let minimum = LogicalSize::new(960.0, 640.0);
        let available = LogicalSize::new(1920.0, 1040.0);
        let size = WindowSize {
            width: 1200.0,
            height: 800.0,
            maximized: true,
        }
        .bounded(minimum, available);
        assert_eq!(
            (size.width, size.height, size.maximized),
            (1200.0, 800.0, true)
        );
        let invalid = WindowSize {
            width: f64::NAN,
            height: -10.0,
            maximized: false,
        }
        .bounded(minimum, available);
        assert_eq!((invalid.width, invalid.height), (960.0, 640.0));
    }
}
