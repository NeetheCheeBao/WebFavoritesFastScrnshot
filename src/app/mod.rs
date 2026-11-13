//! 应用主体：状态、事件分发、自绘栏刷新。

mod commands;
mod menu;
mod tree_build;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use native_windows_gui as nwg;
use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{
    GetKeyState, InvalidateRect, LoadCursorW, SetCursor, TrackMouseEvent, IDC_HAND, TME_LEAVE,
    TRACKMOUSEEVENT, VK_CONTROL, WM_ERASEBKGND, WM_LBUTTONUP, WM_MOUSELEAVE, WM_MOUSEMOVE,
    WM_PAINT, WM_SETCURSOR,
};

use crate::config::AppMeta;
use crate::i18n::I18n;
use crate::model::Arena;
use crate::platform::dpi;
use crate::platform::shell::{self, MessageIcon};
use crate::ui::bars::{self, BarContent, BarFonts};
use crate::ui::layout::{Rects, Ui, MIN_HEIGHT, MIN_WIDTH};
use crate::ui::tree_view;

/// 已加载的收藏夹文档。
pub struct Document {
    pub arena: Arena,
    pub path: PathBuf,
    pub file_name: String,
    pub total_folders: usize,
    pub total_links: usize,
}

/// 状态栏内容。
pub enum Status {
    /// 空闲：显示可点击的版权信息
    Idle,
    /// 业务消息
    Message(String),
}

/// 可变的界面状态（控件本身不在此结构中）。
pub struct State {
    pub doc: Option<Document>,
    /// 节点下标 -> 树项句柄
    pub items: Vec<Option<nwg::TreeItem>>,
    pub status: Status,
    pub hover_credit: bool,
    /// 鼠标悬停在路径行上（可点击定位文件）
    pub hover_path: bool,
    /// 路径文本的像素宽度，用于判断点击是否落在文字上
    pub path_text_width: i32,
    pub info: String,
    pub path_text: String,
    pub path_dim: bool,
    pub ctx_node: Option<usize>,
    pub pending_urls: VecDeque<String>,
    pub urls_total: usize,
    pub urls_done: usize,
}

impl State {
    fn new(i18n: &I18n) -> State {
        State {
            doc: None,
            items: Vec::new(),
            status: Status::Idle,
            hover_credit: false,
            hover_path: false,
            path_text_width: 0,
            info: i18n.t("info.ready").to_string(),
            path_text: i18n.t("path.hint").to_string(),
            path_dim: true,
            ctx_node: None,
            pending_urls: VecDeque::new(),
            urls_total: 0,
            urls_done: 0,
        }
    }
}

/// 应用实例。控件不可变，可变数据放在 `RefCell` 中，
/// 以便事件回调以 `Rc<App>` 形式共享。
pub struct App {
    pub ui: Ui,
    pub i18n: I18n,
    pub state: RefCell<State>,
    pub bars: RefCell<BarFonts>,
    pub rects: RefCell<Rects>,
    pub event_handler: RefCell<Option<nwg::EventHandler>>,
    pub raw_handlers: RefCell<Vec<nwg::RawEventHandler>>,
    pub filter_guard: Cell<bool>,
    /// 批量展开/折叠期间抑制信息栏重算，避免 O(n²)
    pub suppress_info: Cell<bool>,
}

impl App {
    pub fn new() -> Result<App, nwg::NwgError> {
        let i18n = I18n::load();
        let meta = AppMeta::load();
        let ui = Ui::build(&i18n, &meta)?;
        let state = State::new(&i18n);

        let app = App {
            ui,
            i18n,
            state: RefCell::new(state),
            bars: RefCell::new(BarFonts::new(dpi::scaled(12), "Microsoft YaHei UI")),
            rects: RefCell::new(Rects::default()),
            event_handler: RefCell::new(None),
            raw_handlers: RefCell::new(Vec::new()),
            filter_guard: Cell::new(false),
            suppress_info: Cell::new(false),
        };

        let rects = app.ui.layout();
        *app.rects.borrow_mut() = rects;
        app.update_scrollbars();

        Ok(app)
    }

    /// 绑定事件并进入消息循环。
    pub fn start(self: &Rc<Self>) {
        let event_target = Rc::clone(self);
        let handler =
            nwg::full_bind_event_handler(&self.ui.window.handle, move |event, data, handle| {
                event_target.on_event(event, data, handle);
            });
        *self.event_handler.borrow_mut() = Some(handler);

        let raw_target = Rc::clone(self);
        if let Ok(raw) = nwg::bind_raw_event_handler(
            &self.ui.window.handle,
            0x10001,
            move |hwnd, msg, wparam, lparam| raw_target.on_raw_message(hwnd, msg, wparam, lparam),
        ) {
            self.raw_handlers.borrow_mut().push(raw);
        }

        if let Some(argument) = std::env::args().nth(1) {
            if crate::config::is_html_path(std::path::Path::new(&argument)) {
                self.open_file(std::path::Path::new(&argument));
            }
        }

        self.refresh_bars();
        nwg::dispatch_thread_events();
    }

    #[allow(deprecated)]
    fn on_event(&self, event: nwg::Event, data: nwg::EventData, handle: nwg::ControlHandle) {
        match event {
            nwg::Event::OnButtonClick => {
                if handle == self.ui.btn_open {
                    self.browse_open();
                } else if handle == self.ui.btn_close {
                    self.close_file();
                } else if handle == self.ui.btn_shot_full {
                    self.take_screenshot(crate::screenshot::ShotMode::Full);
                } else if handle == self.ui.btn_shot_neutral {
                    self.take_screenshot(crate::screenshot::ShotMode::Neutral);
                } else if handle == self.ui.btn_expand {
                    self.expand_all();
                } else if handle == self.ui.btn_collapse {
                    self.collapse_all();
                }
            }
            nwg::Event::OnTextInput => {
                if handle == self.ui.filter_input && !self.filter_guard.get() {
                    self.schedule_filter();
                }
            }
            nwg::Event::OnTimerTick => {
                if handle == self.ui.filter_timer.handle {
                    self.ui.filter_timer.stop();
                    self.apply_filter();
                } else if handle == self.ui.open_timer.handle {
                    self.open_next_url();
                }
            }
            nwg::Event::OnTreeItemExpanded => {
                if !self.suppress_info.get() {
                    self.update_info_bar();
                    self.update_scrollbars();
                }
            }
            nwg::Event::OnTreeViewClick => {
                self.on_tree_click();
            }
            nwg::Event::OnTreeViewDoubleClick => {
                self.on_tree_double_click();
            }
            nwg::Event::OnTreeViewRightClick => {
                self.on_tree_right_click();
            }
            nwg::Event::OnKeyPress => {
                self.on_key_press(data.on_key(), &handle);
            }
            nwg::Event::OnFileDrop => {
                let files = data.on_file_drop().files();
                self.on_files_dropped(files);
            }
            nwg::Event::OnMousePress(nwg::MousePressEvent::MousePressLeftDown) => {
                if self.ui.tree.handle != handle {
                    self.clear_tree_selection();
                }
            }
            nwg::Event::OnResize | nwg::Event::OnWindowMaximize => {
                // 子控件也会冒泡出 OnResize，只有主窗口才需要重新布局；
                // 最大化时系统只发 SIZE_MAXIMIZED，不会补发 OnResize
                if handle == self.ui.window.handle {
                    self.relayout();
                }
            }
            nwg::Event::OnMinMaxInfo => {
                data.on_min_max().set_min_size(
                    self.ui.toolbar_width().max(dpi::scaled(MIN_WIDTH)),
                    dpi::scaled(MIN_HEIGHT),
                );
            }
            nwg::Event::OnWindowClose => {
                nwg::stop_thread_dispatch();
            }
            _ => {}
        }
    }

    /// 处理主窗口的原始消息：背景擦除、两条栏绘制、链接悬停与点击。
    fn on_raw_message(
        &self,
        hwnd: HWND,
        message: u32,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Option<LRESULT> {
        match message {
            WM_ERASEBKGND => {
                self.draw_bars(hwnd, true);
                Some(1)
            }
            WM_PAINT => {
                self.draw_bars(hwnd, false);
                Some(0)
            }
            WM_MOUSEMOVE => {
                self.update_credit_hover(hwnd);
                None
            }
            WM_MOUSELEAVE => {
                self.set_hover(false, false);
                None
            }
            WM_SETCURSOR => {
                let (hover_path, hover_credit) = {
                    let state = self.state.borrow();
                    (state.hover_path, state.hover_credit)
                };
                if hover_path || hover_credit {
                    unsafe {
                        SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_HAND));
                    }
                    Some(1)
                } else {
                    None
                }
            }
            WM_LBUTTONUP => {
                self.handle_bar_click(hwnd);
                None
            }
            _ => None,
        }
    }

    /// `erase = true` 时走背景擦除通道，否则走 `WM_PAINT` 通道。
    fn draw_bars(&self, hwnd: HWND, erase: bool) {
        let state = self.state.borrow();
        let (status_text, is_credit) = match &state.status {
            Status::Idle => (crate::config::COPYRIGHT.to_string(), true),
            Status::Message(text) => (text.clone(), false),
        };

        let content = BarContent {
            info: &state.info,
            status: &status_text,
            status_is_credit: is_credit,
            hover_credit: state.hover_credit,
            path: &state.path_text,
            path_dim: state.path_dim,
            path_hover: state.hover_path,
            path_rect: self.rects.borrow().path,
        };

        let fonts = self.bars.borrow();
        if erase {
            bars::erase_background(hwnd, &content, &fonts);
        } else {
            bars::paint(hwnd, &content, &fonts);
        }
    }

    /// 请求重绘自绘区域。
    pub fn refresh_bars(&self) {
        if let Some(hwnd) = self.ui.window.handle.hwnd() {
            unsafe {
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
        }
    }

    /// 重新计算控件位置并刷新自绘区域。
    fn relayout(&self) {
        let rects = self.ui.layout();
        *self.rects.borrow_mut() = rects;
        self.update_scrollbars();
        self.refresh_bars();
    }

    /// 滚动条按需显示：空闲时都没有；内容放得下时也不出现。
    ///
    /// 做法是先两条都打开让控件算出真实滚动范围，再用 `GetScrollInfo` 判断是否需要，
    /// 最后按判断结果关闭多余的一条。测量与切换期间挂起重绘，避免闪烁。
    pub fn update_scrollbars(&self) {
        let Some(hwnd) = self.ui.tree.handle.hwnd() else {
            return;
        };

        if self.state.borrow().doc.is_none() {
            tree_view::set_scrollbars(hwnd, false, false);
            return;
        }

        tree_view::set_redraw(&self.ui.tree, false);

        // 先两条都关掉，量出「完全没有滚动条」时的可视区域
        tree_view::set_scrollbars(hwnd, false, false);
        let mut client: winapi::shared::windef::RECT = unsafe { std::mem::zeroed() };
        unsafe {
            winapi::um::winuser::GetClientRect(hwnd, &mut client);
        }
        let available_width = client.right;
        let available_height = client.bottom;

        let visible = self.collect_visible_rows();
        let rows = visible.len() as i32;
        let row_height = tree_view::row_height(&self.ui.tree).max(1);
        let scrollbar = tree_view::scrollbar_size();

        // 只按当前可见行判断，折叠起来的内容不算
        let content_width = visible
            .iter()
            .map(|row| self.row_width(&row.text, row.depth))
            .max()
            .unwrap_or(0);

        let mut need_vertical = rows * row_height > available_height;
        let width_limit = available_width - if need_vertical { scrollbar } else { 0 };
        let need_horizontal = content_width > width_limit;

        // 横向滚动条会再吃掉一条高度，需要回头复核纵向
        if need_horizontal && !need_vertical {
            need_vertical = rows * row_height > available_height - scrollbar;
        }

        tree_view::set_scrollbars(hwnd, need_vertical, need_horizontal);

        tree_view::set_redraw(&self.ui.tree, true);
        tree_view::refresh(&self.ui.tree);
    }

    /// 更新状态栏为一条业务消息。
    pub fn set_status(&self, text: String) {
        self.state.borrow_mut().status = Status::Message(text);
        self.refresh_bars();
    }

    /// 状态栏回到空闲（显示版权信息）。
    pub fn set_status_idle(&self) {
        self.state.borrow_mut().status = Status::Idle;
        self.refresh_bars();
    }

    /// 弹出错误提示。
    pub fn error(&self, title_key: &str, text_key: &str, args: &[(&str, &str)]) {
        let title = self.i18n.t(title_key).to_string();
        let text = self.i18n.fmt(text_key, args);
        shell::message(
            self.ui.window.handle.hwnd(),
            &title,
            &text,
            MessageIcon::Error,
        );
    }

    /// 弹出信息提示。
    pub fn info_box(&self, title_key: &str, text: &str) {
        let title = self.i18n.t(title_key).to_string();
        shell::message(
            self.ui.window.handle.hwnd(),
            &title,
            text,
            MessageIcon::Info,
        );
    }

    fn status_is_credit(&self) -> bool {
        matches!(self.state.borrow().status, Status::Idle)
    }

    fn update_credit_hover(&self, hwnd: HWND) {
        unsafe {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
        }

        let (x, y) = tree_view::cursor_client_pos(hwnd);
        let path_hover = self.is_over_path_text(x, y);
        let credit_hover = self.status_is_credit() && bars::is_in_status_bar(hwnd, y);
        self.set_hover(path_hover, credit_hover);
    }

    /// 坐标是否落在路径行的文字上（只有打开文件后才可点）。
    fn is_over_path_text(&self, x: i32, y: i32) -> bool {
        let state = self.state.borrow();
        if state.doc.is_none() || state.path_text_width <= 0 {
            return false;
        }
        let rect = self.rects.borrow().path;
        y >= rect.top && y < rect.bottom && x >= 8 && x <= 8 + state.path_text_width + 4
    }

    fn set_hover(&self, path_hover: bool, credit_hover: bool) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.hover_path == path_hover && state.hover_credit == credit_hover {
                false
            } else {
                state.hover_path = path_hover;
                state.hover_credit = credit_hover;
                true
            }
        };

        if changed {
            self.refresh_bars();
        }
    }

    fn handle_bar_click(&self, hwnd: HWND) {
        let (x, y) = tree_view::cursor_client_pos(hwnd);

        // 路径行：在资源管理器中定位当前文件
        if self.is_over_path_text(x, y) {
            let path = self.state.borrow().doc.as_ref().map(|doc| doc.path.clone());
            if let Some(path) = path {
                if let Err(error) = shell::reveal_in_explorer(&path) {
                    self.error(
                        "error.open_failed_title",
                        "error.open_failed",
                        &[("error", &error)],
                    );
                }
            }
            return;
        }

        // 状态栏：打开仓库
        if self.status_is_credit() && bars::is_in_status_bar(hwnd, y) {
            let _ = shell::open_url(crate::config::REPOSITORY_URL);
        }
    }

    fn on_key_press(&self, key: u32, handle: &nwg::ControlHandle) {
        let ctrl_pressed = unsafe { GetKeyState(VK_CONTROL) < 0 };

        match key {
            0x4F if ctrl_pressed => self.browse_open(),
            0x46 if ctrl_pressed => self.focus_filter(),
            0x74 => self.reload(),
            0x0D if *handle == self.ui.tree.handle => self.open_selected(),
            _ => {}
        }
    }

    fn focus_filter(&self) {
        self.ui.filter_input.set_focus();
        let length = self.ui.filter_input.len();
        self.ui.filter_input.set_selection(0..length);
    }

    fn clear_tree_selection(&self) {
        if let Some(item) = self.ui.tree.selected_item() {
            self.ui.tree.unselect_item(&item);
        }
    }
}
