use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
pub const PROTOCOL: u64 = 1;

/// CodeBuddy CN must be matched before CodeBuddy: `codebuddy` is a prefix of `codebuddycn`.
pub fn codebuddy_edition_from_text(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("codebuddycn") || text.contains("CodeBuddy CN") {
        Some("domestic")
    } else if lower.contains("com.tencent.codebuddy")
        || text.contains("/CodeBuddy.app")
        || text.contains("\\CodeBuddy.app")
        || text.contains("Application Support/CodeBuddy/")
        || text.contains("Application Support\\CodeBuddy\\")
    {
        Some("international")
    } else {
        None
    }
}

pub fn codebuddy_edition_from_host() -> Option<&'static str> {
    let mut blob = String::new();
    for key in [
        "__CFBundleIdentifier",
        "XPC_SERVICE_NAME",
        "VSCODE_IPC_HOOK",
        "VSCODE_CODE_CACHE_PATH",
        "VSCODE_NLS_CONFIG",
    ] {
        if let Ok(value) = std::env::var(key) {
            blob.push(' ');
            blob.push_str(&value);
        }
    }
    if let Some(found) = codebuddy_edition_from_text(&blob) {
        return Some(found);
    }
    #[cfg(unix)]
    {
        let mut pid = std::process::id();
        for _ in 0..8 {
            let output = std::process::Command::new("ps")
                .args(["-p", &pid.to_string(), "-o", "ppid=,command="])
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stdout);
            blob.push(' ');
            blob.push_str(&text);
            if let Some(found) = codebuddy_edition_from_text(&blob) {
                return Some(found);
            }
            let ppid = text
                .split_whitespace()
                .next()
                .and_then(|value| value.parse().ok())?;
            if ppid <= 1 || ppid == pid {
                break;
            }
            pid = ppid;
        }
    }
    None
}
pub fn arg_value(name: &str) -> Option<String> {
    let args: Vec<_> = std::env::args().collect();
    args.iter()
        .position(|s| s == name)
        .and_then(|i| args.get(i + 1).cloned())
}
/// 解析运行时主目录（`.agent-studio`、hooks 与各客户端配置都相对它定位）。
///
/// 优先级从高到低：
/// 1. `--home <path>`：显式参数（hooks 命令与「设置路径」场景写入）；
/// 2. `AGENT_STUDIO_HOME`：应用级覆盖（便携安装、测试、多实例）；
/// 3. 平台默认：
///    - Windows：`USERPROFILE` → `HOMEDRIVE`+`HOMEPATH` → `HOME`（仅接受绝对路径）；
///    - 其它：`HOME` → `USERPROFILE`。
///
/// Windows 上 `HOME` 只是 Unix 约定：Git Bash / MSYS2 常把它改成仓库目录
/// （如 `D:\GitHome`），先读它会让运行时把端点与 hooks 写到非用户目录，
/// 于是悬浮窗设置报「未找到应用配置目录」。所以 Windows 以系统认定的
/// `USERPROFILE` 为准，`HOME` 只在没有系统主目录时兜底，且 MSYS 风格的
/// `/d/GitHome` 不是 Windows 绝对路径，会被忽略。
///
/// 所有候选都要求非空；全部缺失时返回空路径（等价于当前工作目录），
/// 与旧行为一致。
pub fn home() -> PathBuf {
    let args: Vec<_> = std::env::args_os().collect();
    let explicit = args
        .iter()
        .position(|s| s == "--home")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    resolve_home(
        explicit,
        std::env::var_os("AGENT_STUDIO_HOME").map(PathBuf::from),
        std::env::var_os("USERPROFILE").map(PathBuf::from),
        home_drive_path(),
        std::env::var_os("HOME").map(PathBuf::from),
        cfg!(windows),
    )
}

/// `HOMEDRIVE` + `HOMEPATH`：服务账号 / 精简环境常常只有这两个变量。
#[cfg(windows)]
fn home_drive_path() -> Option<PathBuf> {
    let mut path = std::env::var_os("HOMEDRIVE")?;
    path.push(std::env::var_os("HOMEPATH")?);
    Some(PathBuf::from(path))
}
#[cfg(not(windows))]
fn home_drive_path() -> Option<PathBuf> {
    None
}

fn non_empty(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| !path.as_os_str().is_empty())
}

/// `home()` 的纯函数内核：参数即候选值，便于在任意平台跑全部分支的测试。
fn resolve_home(
    explicit: Option<PathBuf>,
    app_override: Option<PathBuf>,
    user_profile: Option<PathBuf>,
    home_drive: Option<PathBuf>,
    home_var: Option<PathBuf>,
    windows: bool,
) -> PathBuf {
    if let Some(path) = non_empty(explicit) {
        return path;
    }
    if let Some(path) = non_empty(app_override) {
        return path;
    }
    if windows {
        non_empty(user_profile)
            .or_else(|| non_empty(home_drive).filter(|path| path.is_absolute()))
            .or_else(|| non_empty(home_var).filter(|path| path.is_absolute()))
            .unwrap_or_default()
    } else {
        non_empty(home_var)
            .or_else(|| non_empty(user_profile))
            .unwrap_or_default()
    }
}
pub fn endpoint(home: &Path) -> PathBuf {
    home.join(".agent-studio/runtime-v1.json")
}
pub fn call(home: &Path, command: &str, payload: Value) -> Result<Value, String> {
    let bytes = std::fs::read(endpoint(home)).map_err(|_| "监听服务未启动")?;
    let info: Value = serde_json::from_slice(&bytes).map_err(|_| "监听服务地址无效")?;
    if info["protocol"] != PROTOCOL {
        return Err("监听协议版本不兼容".into());
    }
    let port = info["port"]
        .as_u64()
        .filter(|p| *p > 0 && *p < 65536)
        .ok_or("监听服务端口无效")?;
    let token = info["token"].as_str().ok_or("监听认证信息缺失")?;
    let response: Value = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(if command.starts_with("settings_")
            || command.starts_with("integrations_")
            || command.starts_with("custom_")
        {
            12
        } else {
            3
        }))
        .build()
        .post(&format!("http://127.0.0.1:{port}/rpc"))
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(json!({"command":command,"payload":payload}))
        .map_err(|_| "监听服务连接失败")?
        .into_json()
        .map_err(|_| "监听服务响应无效")?;
    if let Some(e) = response["error"].as_str() {
        Err(e.into())
    } else {
        Ok(response["value"].clone())
    }
}
pub struct Client {
    pub home: PathBuf,
    pub id: String,
}
impl Client {
    pub fn connect(home: PathBuf, binary: &Path) -> Result<Self, String> {
        let client = Self {
            home,
            id: uuid::Uuid::new_v4().to_string(),
        };
        if call(&client.home, "hello", json!({"client":client.id})).is_err() {
            let mut c = std::process::Command::new(binary);
            c.arg("serve")
                .env("AGENT_STUDIO_HOME", &client.home)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                c.creation_flags(0x08000000);
            }
            let mut child = c.spawn().map_err(|_| "无法启动原生采集服务")?;
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            for _ in 0..60 {
                if call(&client.home, "hello", json!({"client":client.id})).is_ok() {
                    return Ok(client);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            return Err("原生采集服务启动超时".into());
        }
        Ok(client)
    }
    pub fn poll(&self) -> Result<Value, String> {
        call(&self.home, "poll", json!({"client":self.id}))
    }
    pub fn request(&self, command: &str, payload: Value) -> Result<Value, String> {
        call(&self.home, command, payload)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = call(&self.home, "leave", json!({"client":self.id}));
    }
}

#[cfg(test)]
mod tests {
    use super::codebuddy_edition_from_text;

    #[test]
    fn integration_rpc_allows_remote_status_latency() {
        let home = std::env::temp_dir().join(format!("integration-rpc-{}", uuid::Uuid::new_v4()));
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        agent_studio_core::atomic_json(&super::endpoint(&home), &serde_json::json!({"protocol":super::PROTOCOL,"port":port,"token":"test"})).unwrap();
        let worker = std::thread::spawn(move || {
            let request = server.recv().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(3200));
            request.respond(tiny_http::Response::from_string(r#"{"value":{"sources":[]}}"#)).unwrap();
        });
        let result = super::call(&home, "integrations_get", serde_json::json!({}));
        worker.join().unwrap();
        std::fs::remove_dir_all(home).unwrap();
        assert_eq!(result.unwrap(), serde_json::json!({"sources":[]}));
    }

    #[test]
    fn codebuddy_host_text_prefers_cn_before_the_international_prefix() {
        assert_eq!(
            codebuddy_edition_from_text("__CFBundleIdentifier=com.tencent.codebuddycn"),
            Some("domestic")
        );
        assert_eq!(
            codebuddy_edition_from_text("/Applications/CodeBuddy CN.app/Contents/MacOS/Electron"),
            Some("domestic")
        );
        assert_eq!(
            codebuddy_edition_from_text("Application Support/CodeBuddy CN/1.10-main.sock"),
            Some("domestic")
        );
        assert_eq!(
            codebuddy_edition_from_text("__CFBundleIdentifier=com.tencent.codebuddy"),
            Some("international")
        );
        assert_eq!(
            codebuddy_edition_from_text("/Applications/CodeBuddy.app/Contents/MacOS/Electron"),
            Some("international")
        );
        assert_eq!(
            codebuddy_edition_from_text("Application Support/CodeBuddy/1.10-main.sock"),
            Some("international")
        );
        assert_eq!(codebuddy_edition_from_text("unrelated"), None);
    }
}

#[cfg(test)]
mod home_tests {
    use super::resolve_home;
    use std::path::PathBuf;

    fn path(value: &str) -> Option<PathBuf> {
        Some(PathBuf::from(value))
    }

    fn windows(
        user_profile: Option<PathBuf>,
        home_drive: Option<PathBuf>,
        home_var: Option<PathBuf>,
    ) -> PathBuf {
        resolve_home(None, None, user_profile, home_drive, home_var, true)
    }

    #[test]
    fn explicit_home_wins_over_every_environment() {
        let home = resolve_home(
            path(r"D:/Custom"),
            path(r"C:/Override"),
            path(r"C:/Users/me"),
            None,
            path(r"D:/GitHome"),
            true,
        );
        assert_eq!(home, PathBuf::from(r"D:/Custom"));
    }

    #[test]
    fn app_override_wins_over_platform_variables() {
        let home = resolve_home(
            None,
            path(r"D:/Portable"),
            path(r"C:/Users/me"),
            None,
            path(r"D:/GitHome"),
            true,
        );
        assert_eq!(home, PathBuf::from(r"D:/Portable"));
    }

    /// 回归：Windows 上必须优先 USERPROFILE，不能被 Git Bash 的 HOME 带跑。
    #[test]
    fn windows_prefers_user_profile_over_home() {
        let home = windows(path(r"C:/Users/me"), None, path(r"D:/GitHome"));
        assert_eq!(home, PathBuf::from(r"C:/Users/me"));
    }

    #[test]
    fn windows_falls_back_to_home_drive_then_home() {
        assert_eq!(
            windows(None, path(r"C:/Users/service"), path(r"D:/GitHome")),
            PathBuf::from(r"C:/Users/service")
        );
        assert_eq!(
            windows(None, None, path(r"D:/GitHome")),
            PathBuf::from(r"D:/GitHome")
        );
    }

    /// MSYS 风格的 `/d/GitHome` 不是 Windows 绝对路径，不能当主目录。
    #[test]
    fn windows_ignores_msys_style_home() {
        assert_eq!(windows(None, None, path("/d/GitHome")), PathBuf::new());
        assert_eq!(windows(None, None, path("/")), PathBuf::new());
    }

    #[test]
    fn unix_prefers_home_and_falls_back_to_user_profile() {
        let home = resolve_home(None, None, path("/home/fallback"), None, path("/home/me"), false);
        assert_eq!(home, PathBuf::from("/home/me"));
        let fallback = resolve_home(None, None, path("/home/fallback"), None, None, false);
        assert_eq!(fallback, PathBuf::from("/home/fallback"));
    }

    #[test]
    fn empty_values_fall_through_and_missing_everything_stays_empty() {
        assert_eq!(
            windows(path(""), path(""), path(r"D:/GitHome")),
            PathBuf::from(r"D:/GitHome")
        );
        assert_eq!(windows(None, None, None), PathBuf::new());
        assert_eq!(
            resolve_home(Some(PathBuf::new()), None, None, None, None, false),
            PathBuf::new()
        );
    }
}
