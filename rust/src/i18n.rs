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
