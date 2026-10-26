//! Language selection for bundled Simplified Chinese and English translations.

pub fn chinese(mode: &str) -> bool {
    match mode {
        "zh_CN" | "zh-CN" => true,
        "en" | "en-US" => false,
        _ => unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() & 0x03ff == 0x04 },
    }
}

pub fn locale(mode: &str) -> &'static str {
    if chinese(mode) { "zh_CN" } else { "en" }
}

pub fn index(mode: &str) -> i32 {
    match mode { "en" | "en-US" => 1, "zh_CN" | "zh-CN" => 2, _ => 0 }
}

/// User-visible strings owned by Rust rather than the Slint interface.
pub fn tr(zh: bool, key: &'static str) -> &'static str {
    if !zh { return key; }
    match key {
        "Show PinIt" => "显示 PinIt",
        "Pin a window…" => "置顶窗口…",
        "Unpin all" => "全部取消置顶",
        "Quit PinIt" => "退出 PinIt",
        "Pin / unpin window" => "置顶 / 取消置顶窗口",
        "Increase opacity" => "提高不透明度",
        "Decrease opacity" => "降低不透明度",
        "Show / hide PinIt" => "显示 / 隐藏 PinIt",
        "Pinned" => "已置顶",
        "Unpinned" => "已取消置顶",
        "Nothing to unpin." => "没有需要取消置顶的窗口。",
        "Clicks reach this window again." => "此窗口已恢复鼠标交互。",
        "Clicks now pass through this window." => "已启用鼠标穿透。",
        "Shortcuts saved." => "快捷键已保存。",
        "Two actions can't share the same keys." => "两个操作不能使用相同的快捷键。",
        "Couldn't change the startup setting." => "无法修改开机启动设置。",
        "PinIt is still running" => "PinIt 仍在运行",
        "Your pins stay active. Find PinIt in the tray to open it again." => "窗口置顶仍然有效，可在系统托盘中重新打开 PinIt。",
        _ => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_fallback_and_user_choice() {
        assert!(chinese("zh_CN"));
        assert!(!chinese("en"));
        assert_eq!(index("system"), 0);
        assert_eq!(tr(true, "Unpin all"), "全部取消置顶");
        assert_eq!(tr(false, "Unpin all"), "Unpin all");
    }
}
