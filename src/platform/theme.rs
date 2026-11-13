//! 启动时激活 comctl32 v6 视觉样式（挂激活上下文，无需外部清单）。

use std::mem::{size_of, zeroed};
use std::path::PathBuf;

use winapi::shared::basetsd::ULONG_PTR;
use winapi::um::winbase::{
    ActivateActCtx, CreateActCtxW, DeactivateActCtx, ReleaseActCtx, ACTCTXW,
};

/// 只声明 comctl32 v6 依赖。
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
