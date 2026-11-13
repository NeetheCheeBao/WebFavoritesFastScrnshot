//! 程序入口：初始化 DPI 感知、创建应用并进入消息循环。

// 生成 GUI 子系统程序，避免启动时弹出控制台窗口
#![windows_subsystem = "windows"]

mod app;
mod config;
mod i18n;
mod model;
mod parser;
mod platform;
mod screenshot;
mod ui;

use std::rc::Rc;

use native_windows_gui as nwg;

fn main() {
    platform::dpi::enable_dpi_awareness();

    // 必须在初始化通用控件之前激活 comctl32 v6，否则控件会退回经典样式
    let _theme = platform::theme::activate();

    if let Err(error) = nwg::init() {
        report_startup_error(&format!("{error:?}"));
        return;
    }

    let app = match app::App::new() {
        Ok(app) => Rc::new(app),
        Err(error) => {
            report_startup_error(&format!("{error:?}"));
            return;
        }
    };

    app.start();
}

/// 界面尚未建立时的兜底错误提示。
fn report_startup_error(error: &str) {
    let i18n = i18n::I18n::load();
    let title = i18n.t("error.init_failed_title").to_string();
    let text = i18n.fmt("error.init_failed", &[("error", error)]);

    platform::shell::message(None, &title, &text, platform::shell::MessageIcon::Error);
}
