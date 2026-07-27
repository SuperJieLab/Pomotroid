//! macOS 勿扰模式（DND）联动。
//! 通过 `shortcuts run <name>` 调用用户在「快捷指令」App 里建好的指令。
//! 非 macOS 或功能关闭时直接 no-op。

/// 抽象 CLI 调用，便于单测注入 mock。
pub trait Runner {
    fn run(&self, name: &str) -> std::io::Result<()>;
}

/// 生产用 runner：真正执行 `shortcuts run <name>`。
pub struct OsRunner;
impl Runner for OsRunner {
    fn run(&self, name: &str) -> std::io::Result<()> {
        std::process::Command::new("shortcuts")
            .arg("run")
            .arg(name)
            .status()
            .and_then(|s| {
                if s.success() {
                    Ok(())
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("shortcut '{name}' exited non-zero"),
                    ))
                }
            })
    }
}

/// 根据当前是否 Work 轮，切换 DND。
/// - `is_mac=false` → 直接 Ok，不碰 CLI（其他平台无 shortcuts CLI）。
/// - `is_work=true`  → 运行 `on_name`
/// - `is_work=false` → 运行 `off_name`
///
/// 注意「功能是否开启」的闸门不在此处，而在调用方（timer 的 Started 分支），
/// 这样 `apply_dnd` 保持纯函数、易于单测。
pub fn apply_dnd(
    is_work: bool,
    on_name: &str,
    off_name: &str,
    runner: &dyn Runner,
    is_mac: bool,
) -> Result<(), std::io::Error> {
    if !is_mac {
        return Ok(());
    }
    let name = if is_work { on_name } else { off_name };
    runner.run(name)
}

/// 从 round_type 判定是否应开启 DND（仅 Work 轮开启）。
pub fn should_enable(round: crate::timer::sequence::RoundType) -> bool {
    matches!(round, crate::timer::sequence::RoundType::Work)
}

#[cfg(test)]
mod tests {
    /// Mock runner：记录调用，返回预设结果。
    struct MockRunner {
        calls: std::cell::RefCell<Vec<String>>,
        fail: bool,
    }
    impl super::Runner for MockRunner {
        fn run(&self, name: &str) -> std::io::Result<()> {
            self.calls.borrow_mut().push(name.to_string());
            if self.fail {
                Err(std::io::Error::new(std::io::ErrorKind::NotFound, "boom"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn non_mac_is_noop() {
        let r = MockRunner { calls: Default::default(), fail: false };
        let res = super::apply_dnd(true, "On", "Off", &r, false); // is_mac=false
        assert!(res.is_ok());
        assert!(r.calls.borrow().is_empty(), "non-mac must not call CLI");
    }

    #[test]
    fn work_enables_on_shortcut() {
        let r = MockRunner { calls: Default::default(), fail: false };
        super::apply_dnd(true, "On", "Off", &r, true).unwrap();
        assert_eq!(r.calls.borrow().as_slice(), &["On".to_string()]);
    }

    #[test]
    fn break_disables_via_off_shortcut() {
        let r = MockRunner { calls: Default::default(), fail: false };
        super::apply_dnd(false, "On", "Off", &r, true).unwrap();
        assert_eq!(r.calls.borrow().as_slice(), &["Off".to_string()]);
    }

    #[test]
    fn cli_failure_propagates_error() {
        let r = MockRunner { calls: Default::default(), fail: true };
        let res = super::apply_dnd(true, "On", "Off", &r, true);
        assert!(res.is_err());
    }

    #[test]
    fn only_work_enables() {
        use crate::timer::sequence::RoundType;
        assert!(super::should_enable(RoundType::Work));
        assert!(!super::should_enable(RoundType::ShortBreak));
        assert!(!super::should_enable(RoundType::LongBreak));
    }
}
