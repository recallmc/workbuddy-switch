use crate::{AutoLaunch, Result};
use winreg::enums::RegType::REG_BINARY;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::{RegKey, RegValue};

static AL_REGKEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
static TASK_MANAGER_OVERRIDE_REGKEY: &str =
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
static TASK_MANAGER_OVERRIDE_ENABLED_VALUE: [u8; 12] = [
    0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Windows implement
impl AutoLaunch {
    /// Create a new AutoLaunch instance
    /// - `app_name`: application name
    /// - `app_path`: application path
    /// - `args`: startup args passed to the binary
    ///
    /// ## Notes
    ///
    /// The parameters of `AutoLaunch::new` are different on each platform.
    pub fn new(app_name: &str, app_path: &str, args: &[impl AsRef<str>]) -> AutoLaunch {
        AutoLaunch {
            app_name: app_name.into(),
            app_path: app_path.into(),
            args: args.iter().map(|s| s.as_ref().to_string()).collect(),
        }
    }

    /// 命令行：路径含空格时必须加引号，否则登录时 Windows 会把
    /// `C://Program Files\...` 截成 `C://Program` 去执行，自启动静默失败。
    fn startup_command(&self) -> String {
        let mut command = format!("\"{}\"", self.app_path);
        if !self.args.is_empty() {
            command.push(' ');
            command.push_str(&self.args.join(" "));
        }
        command
    }

    /// Enable the AutoLaunch setting
    ///
    /// ## Errors
    ///
    /// - failed to open the registry key
    /// - failed to set value
    pub fn enable(&self) -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey_with_flags(AL_REGKEY, KEY_SET_VALUE)?
            .set_value::<_, _>(&self.app_name, &self.startup_command())?;

        // this key maybe not found
        if let Ok(reg) = hkcu.open_subkey_with_flags(TASK_MANAGER_OVERRIDE_REGKEY, KEY_SET_VALUE) {
            reg.set_raw_value(
                &self.app_name,
                &RegValue {
                    vtype: REG_BINARY,
                    bytes: TASK_MANAGER_OVERRIDE_ENABLED_VALUE.to_vec(),
                },
            )?;
        }

        Ok(())
    }

    /// Disable the AutoLaunch setting
    ///
    /// ## Errors
    ///
    /// - failed to open the registry key
    /// - failed to delete value
    pub fn disable(&self) -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey_with_flags(AL_REGKEY, KEY_SET_VALUE)?
            .delete_value(&self.app_name)?;
        Ok(())
    }

    /// Check whether the AutoLaunch setting is enabled
    pub fn is_enabled(&self) -> Result<bool> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        let al_enabled = hkcu
            .open_subkey_with_flags(AL_REGKEY, KEY_READ)?
            .get_value::<String, _>(&self.app_name)
            .is_ok();
        let task_manager_enabled = self.task_manager_enabled(hkcu);

        Ok(al_enabled && task_manager_enabled.unwrap_or(true))
    }

    fn task_manager_enabled(&self, hkcu: RegKey) -> Option<bool> {
        let task_manager_override_raw_value = hkcu
            .open_subkey_with_flags(TASK_MANAGER_OVERRIDE_REGKEY, KEY_READ)
            .ok()?
            .get_raw_value(&self.app_name)
            .ok()?;
        Some(last_eight_bytes_all_zeros(
            &task_manager_override_raw_value.bytes,
        )?)
    }
}

fn last_eight_bytes_all_zeros(bytes: &[u8]) -> Option<bool> {
    if bytes.len() < 8 {
        return None;
    }
    Some(bytes.iter().rev().take(8).all(|v| *v == 0u8))
}

#[cfg(test)]
mod windows_tests {
    use super::AutoLaunch;

    #[test]
    fn startup_command_quotes_paths_with_spaces() {
        let launcher = AutoLaunch::new(
            "workbuddy-switch",
            r"D:/Program Files/workbuddy-switch/wb-switch-rust.exe",
            &["--hidden"],
        );
        assert_eq!(
            launcher.startup_command(),
            r#""D:/Program Files/workbuddy-switch/wb-switch-rust.exe" --hidden"#
        );
    }

    #[test]
    fn startup_command_without_args_has_no_trailing_space() {
        let launcher = AutoLaunch::new("demo", r"C:/Tools/demo.exe", &[] as &[&str]);
        assert_eq!(launcher.startup_command(), r#""C:/Tools/demo.exe""#);
    }
}
