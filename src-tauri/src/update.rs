use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
}

#[derive(Debug, serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    prerelease: bool,
    draft: bool,
    html_url: String,
}

/// 检测 GitHub 是否有新版本
pub async fn check_for_update(
    current_version: &str,
    include_prerelease: bool,
) -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("PeriphMonitor")
        .build()
        .map_err(|e| e.to_string())?;

    let releases: Vec<GitHubRelease> = client
        .get("https://api.github.com/repos/oneday5799/PeriphMonitor/releases")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "timeout".to_string()
            } else if let Some(status) = e.status() {
                if status.as_u16() == 403 {
                    "rate_limited".to_string()
                } else {
                    format!("network error: {}", e)
                }
            } else {
                format!("network error: {}", e)
            }
        })?
        .json()
        .await
        .map_err(|e| format!("parse error: {}", e))?;

    // 过滤：跳过 draft 和不符合条件的 pre-release
    let latest = releases.iter().find(|r| {
        if r.draft {
            return false;
        }
        if r.prerelease && !include_prerelease {
            return false;
        }
        true
    });

    match latest {
        Some(release) => {
            let latest_ver = release.tag_name.trim_start_matches('v');
            let has_update = compare_versions(current_version, latest_ver);
            Ok(UpdateInfo {
                has_update,
                current_version: current_version.to_string(),
                latest_version: latest_ver.to_string(),
                release_url: release.html_url.clone(),
            })
        }
        None => Ok(UpdateInfo {
            has_update: false,
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            release_url: String::new(),
        }),
    }
}

/// 比较版本号：返回 latest > current
fn compare_versions(current: &str, latest: &str) -> bool {
    let cur = semver::Version::parse(current);
    let lat = semver::Version::parse(latest);
    match (cur, lat) {
        (Ok(c), Ok(l)) => l > c,
        _ => false,
    }
}
