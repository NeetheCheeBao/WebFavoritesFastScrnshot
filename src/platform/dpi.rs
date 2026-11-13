//! 进程 DPI 感知设置与像素换算。
//!
//! 与原版一致，采用"系统 DPI 感知"：窗口与控件按物理像素布局，
//! 由系统在跨屏时统一缩放。字体与间距按系统 DPI 换算，
//! 保证 125%/150% 缩放下界面不会变得过小。

use std::sync::atomic::{AtomicU32, Ordering};

use winapi::um::wingdi::{GetDeviceCaps, LOGPIXELSX};
use winapi::um::winuser::{GetDC, ReleaseDC, SetProcessDPIAware};

/// 系统 DPI 的千分比（96dpi 记为 1000）。
static DPI_PERMILLE: AtomicU32 = AtomicU32::new(1000);

/// 尽早调用，需在创建任何窗口之前执行。
pub fn enable_dpi_awareness() {
    unsafe {
        SetProcessDPIAware();

        let dc = GetDC(std::ptr::null_mut());
        if !dc.is_null() {
            let dpi = GetDeviceCaps(dc, LOGPIXELSX);
            if dpi > 0 {
                DPI_PERMILLE.store((dpi as u32) * 1000 / 96, Ordering::Relaxed);
            }
            ReleaseDC(std::ptr::null_mut(), dc);
        }
    }
}

/// 当前缩放系数（96dpi 为 1.0）。
pub fn scale() -> f32 {
    DPI_PERMILLE.load(Ordering::Relaxed) as f32 / 1000.0
}

/// 把 96dpi 下的逻辑像素换算成当前 DPI 的物理像素。
pub fn scaled(value: i32) -> i32 {
    (value as f32 * scale()).round() as i32
}
