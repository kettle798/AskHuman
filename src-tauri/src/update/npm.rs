//! NpmUpdater：npm 全局安装的更新器。
//!
//! 检查：npm registry 的 `latest` 元数据（HTTP，不依赖本地 npm，保证「检查」始终可用）。
//! 应用：跑 `npm i -g askhuman@latest`；npm 不可用 / 执行失败 → 返回带手动命令的错误，
//! 由前端回显命令让用户手动执行。日志（notes）仍按 tag 从 GitHub 取（best-effort）。
//! **不 restart**：npm 替换 node_modules 内二进制后，daemon drain 在答完后换新。

use super::{http_client, NPM_PACKAGE};
use super::{ProgressCb, RemoteLatest, Updater};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;

pub struct NpmUpdater;

impl NpmUpdater {
    pub fn new() -> Self {
        Self
    }

    /// 手动更新命令提示（npm 不可用时回显给用户）。
    pub fn manual_command() -> String {
        format!("npm i -g {}@latest", NPM_PACKAGE)
    }
}

#[async_trait::async_trait]
impl Updater for NpmUpdater {
    async fn check_latest(&self) -> Result<RemoteLatest> {
        let version = npm_latest_version().await?;
        // 日志按 tag 从 GitHub 取（best-effort，取不到则空，前端显示占位）。
        let notes = super::notes::notes_for_tag(&version)
            .await
            .unwrap_or_default();
        Ok(RemoteLatest {
            version,
            notes,
            source_url: Self::manual_command(),
        })
    }

    async fn apply(&self, _progress: Option<ProgressCb>) -> Result<()> {
        run_npm_install().await
    }
}

/// 从 npm registry 取主包最新版本（`https://registry.npmjs.org/<pkg>/latest`）。
async fn npm_latest_version() -> Result<String> {
    let url = format!("https://registry.npmjs.org/{}/latest", NPM_PACKAGE);
    let resp = http_client()
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .context("npm registry 请求失败")?;
    if !resp.status().is_success() {
        return Err(anyhow!("npm registry 返回 {}", resp.status()));
    }
    let meta = resp.json::<Value>().await.context("解析 npm 元数据失败")?;
    let v = super::normalize_version(meta["version"].as_str().unwrap_or(""));
    if v.is_empty() {
        return Err(anyhow!("无法解析 npm 最新版本号"));
    }
    Ok(v)
}

/// 执行 `npm i -g <pkg>@latest`。npm 缺失 / 失败 → Err（含手动命令提示）。
async fn run_npm_install() -> Result<()> {
    let cmd = NpmUpdater::manual_command();
    // 在阻塞线程跑外部命令，避免占用 async 执行器。
    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new("npm")
            .args(["i", "-g", &format!("{}@latest", NPM_PACKAGE)])
            .output()
    })
    .await
    .context("等待 npm 进程失败")?;

    match result {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(anyhow!(
            "npm 更新失败，请手动执行：{cmd}\n{}",
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(anyhow!("未找到 npm，请手动执行：{cmd}"))
        }
        Err(e) => Err(anyhow!("无法启动 npm（{e}），请手动执行：{cmd}")),
    }
}
