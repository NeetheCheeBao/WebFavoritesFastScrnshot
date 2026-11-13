//! 启用 comctl32 v6 视觉样式。
//!
//! 没有应用程序清单时，进程会加载 comctl32 v5，按钮/输入框/树/滚动条都会退化成
//! Win95 经典外观。这里在启动阶段挂一个激活上下文（activation context），
//! 让后续加载的 comctl32 解析到 v6 版本，无需构建脚本或 rc.exe。

use std::mem::{size_of, zeroed};
use std::path::PathBuf;

use winapi::shared::basetsd::ULONG_PTR;
use winapi::um::winbase::{
    ActivateActCtx, CreateActCtxW, DeactivateActCtx, ReleaseActCtx, ACTCTXW,
};

/// 只声明 comctl32 v6 依赖，不在此处声明 DPI（运行时再设已来不及）。
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="WebFavoritesFastScrnshot" version="1.0.0.0" processorArchitecture="*"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#;

/// 激活上下文句柄。必须在创建任何窗口之前激活，并保持存活到进程结束。
pub struct ThemeContext {
    handle: *mut winapi::ctypes::c_void,
    cookie: ULONG_PTR,
    manifest_path: PathBuf,
}

/// 尝试激活视觉样式；失败时返回 `None`（界面退回经典样式，但不影响功能）。
pub fn activate() -> Option<ThemeContext> {
    let manifest_path = std::env::temp_dir().join(format!(
        "webfavorites-fast-scrnshot-{}.manifest",
        std::process::id()
    ));
    std::fs::write(&manifest_path, MANIFEST.as_bytes()).ok()?;

    let wide: Vec<u16> = manifest_path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut context: ACTCTXW = unsafe { zeroed() };
    context.cbSize = size_of::<ACTCTXW>() as u32;
    context.lpSource = wide.as_ptr();

    let handle = unsafe { CreateActCtxW(&context) };
    if handle as isize == -1 {
        let _ = std::fs::remove_file(&manifest_path);
        return None;
    }

    let mut cookie: ULONG_PTR = 0;
    if unsafe { ActivateActCtx(handle, &mut cookie) } == 0 {
        unsafe {
            ReleaseActCtx(handle);
        }
        let _ = std::fs::remove_file(&manifest_path);
        return None;
    }

    Some(ThemeContext {
        handle,
        cookie,
        manifest_path,
    })
}

impl Drop for ThemeContext {
    fn drop(&mut self) {
        unsafe {
            DeactivateActCtx(0, self.cookie);
            ReleaseActCtx(self.handle);
        }
        let _ = std::fs::remove_file(&self.manifest_path);
    }
}
