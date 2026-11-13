//! 主窗口与所有原生控件的创建、布局。
//!
//! 所有尺寸常量都按 96dpi 的逻辑像素书写，实际使用时经 [`dpi::scaled`] 换算，
//! 保证 125%/150% 缩放下界面不会过小。

use std::mem::zeroed;

use native_windows_gui as nwg;
use winapi::shared::windef::RECT;
use winapi::um::winuser::GetClientRect;

use crate::config::AppMeta;
use crate::i18n::I18n;
use crate::platform::dpi;
use crate::screenshot::gdi::{Canvas, Font};
use crate::ui::bars::strip_height;
use crate::ui::{icons, tree_view};

/// 界面字体族。
const UI_FONT_FAMILY: &str = "Microsoft YaHei UI";

/// 防抖 / 延时用的计时器。
///
/// `nwg::Timer` 基于 `WM_TIMER`，回调在本线程消息循环中派发，
/// 正好符合"不要在别的线程碰控件"的约束。
#[allow(deprecated)]
pub type DebounceTimer = nwg::Timer;

/// 界面字号（96dpi 下的像素高度，约合 9pt）。
const UI_FONT_PX: i32 = 12;
/// 树字号（96dpi 下的像素高度，约合 10pt）。
const TREE_FONT_PX: i32 = 13;

const BUTTON_HEIGHT: i32 = 26;
const INPUT_HEIGHT: i32 = 26;
const LABEL_HEIGHT: i32 = 20;
const PADDING: i32 = 10;
const BUTTON_GAP: i32 = 8;
const ROW_GAP: i32 = 8;
/// 按钮文字左右留白总量。
const BUTTON_TEXT_PADDING: i32 = 34;
/// 树与底部栏之间的间距。
const TREE_BOTTOM_GAP: i32 = 8;

/// 默认窗口高度（96dpi 逻辑像素）。
pub const DEFAULT_HEIGHT: i32 = 640;
/// 最小窗口尺寸（96dpi 逻辑像素）。
pub const MIN_HEIGHT: i32 = 360;
pub const MIN_WIDTH: i32 = 420;

/// 自绘区域的几何信息（客户区坐标）。
#[derive(Clone, Copy)]
pub struct Rects {
    pub path: RECT,
}

impl Default for Rects {
    fn default() -> Rects {
        Rects {
            path: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        }
    }
}

/// 窗口与控件集合。
///
/// `icon` / `font_ui` / `font_tree` / `image_list` / `probe*` 由本结构持有，
/// 保证对应的 GDI 句柄在程序运行期间始终有效（drop 即销毁）。
#[allow(dead_code)]
pub struct Ui {
    pub window: nwg::Window,
    pub icon: Option<nwg::Icon>,
    pub font_ui: nwg::Font,
    pub font_tree: nwg::Font,
    pub image_list: nwg::ImageList,
    pub btn_open: nwg::Button,
    pub btn_close: nwg::Button,
    pub btn_shot_full: nwg::Button,
    pub btn_shot_neutral: nwg::Button,
    pub btn_expand: nwg::Button,
    pub btn_collapse: nwg::Button,
    pub filter_label: nwg::Label,
    pub filter_input: nwg::TextInput,
    pub tree: nwg::TreeView,
    pub filter_timer: DebounceTimer,
    pub open_timer: DebounceTimer,
    /// 文字测量用的离屏画布（不参与显示）
    probe: Canvas,
    /// 界面文字测量字体
    probe_font: Font,
    /// 树文字测量字体
    tree_probe_font: Font,
}

impl Ui {
    pub fn build(i18n: &I18n, meta: &AppMeta) -> Result<Ui, nwg::NwgError> {
        let title = meta.title(i18n);
        let icon = nwg::Icon::from_bin(include_bytes!("../../assets/icon.ico")).ok();

        let min_width = dpi::scaled(MIN_WIDTH);
        let mut window = nwg::Window::default();
        let mut window_builder = nwg::Window::builder()
            .title(&title)
            .size((min_width, dpi::scaled(DEFAULT_HEIGHT)))
            .accept_files(true);
        if let Some(icon) = &icon {
            window_builder = window_builder.icon(Some(icon));
        }
        window_builder.build(&mut window)?;

        // 注意：nwg 的 `size()` 会被直接当作 lfHeight（像素），
        // 要让文字等于 9pt/10pt，必须用 `size_absolute()` 传像素高度
        let mut font_ui = nwg::Font::default();
        nwg::Font::builder()
            .family(UI_FONT_FAMILY)
            .size_absolute(dpi::scaled(UI_FONT_PX) as u32)
            .build(&mut font_ui)?;

        let mut font_tree = nwg::Font::default();
        nwg::Font::builder()
            .family(UI_FONT_FAMILY)
            .size_absolute(dpi::scaled(TREE_FONT_PX) as u32)
            .build(&mut font_tree)?;

        let image_list = icons::build_image_list()?;

        // 文字测量：与控件字体保持一致的像素高度
        let probe = Canvas::new(8, 8)
            .ok_or_else(|| nwg::NwgError::resource_create("无法创建文字测量画布"))?;
        let probe_font = Font::new(dpi::scaled(UI_FONT_PX), UI_FONT_FAMILY);
        let tree_probe_font = Font::new(dpi::scaled(TREE_FONT_PX), UI_FONT_FAMILY);

        let padding = dpi::scaled(PADDING);
        let button_height = dpi::scaled(BUTTON_HEIGHT);
        let button_text_padding = dpi::scaled(BUTTON_TEXT_PADDING);
        let input_height = dpi::scaled(INPUT_HEIGHT);
        let label_height = dpi::scaled(LABEL_HEIGHT);

        // 按钮宽度按文字实际像素宽计算，保证中文不截断
        let labels = [
            i18n.t("toolbar.open"),
            i18n.t("toolbar.close"),
            i18n.t("toolbar.screenshot_full"),
            i18n.t("toolbar.screenshot_neutral"),
            i18n.t("toolbar.expand_all"),
            i18n.t("toolbar.collapse_all"),
        ];
        let widths: Vec<i32> = labels
            .iter()
            .map(|text| (probe.text_width(text, &probe_font) + button_text_padding).max(72))
            .collect();

        let mut buttons = Vec::with_capacity(labels.len());
        for (text, width) in labels.iter().zip(&widths) {
            let mut button = nwg::Button::default();
            nwg::Button::builder()
                .text(text)
                .parent(&window)
                .font(Some(&font_ui))
                .size((*width, button_height))
                .build(&mut button)?;
            buttons.push(button);
        }
        let mut buttons = buttons.into_iter();
        let btn_open = buttons.next().expect("按钮数量不匹配");
        let btn_close = buttons.next().expect("按钮数量不匹配");
        let btn_shot_full = buttons.next().expect("按钮数量不匹配");
        let btn_shot_neutral = buttons.next().expect("按钮数量不匹配");
        let btn_expand = buttons.next().expect("按钮数量不匹配");
        let btn_collapse = buttons.next().expect("按钮数量不匹配");

        let mut filter_label = nwg::Label::default();
        nwg::Label::builder()
            .text(i18n.t("filter.label"))
            .parent(&window)
            .font(Some(&font_ui))
            .background_color(Some([245, 245, 245]))
            .size((
                probe.text_width(i18n.t("filter.label"), &probe_font) + padding / 2,
                label_height,
            ))
            .build(&mut filter_label)?;

        let mut filter_input = nwg::TextInput::default();
        nwg::TextInput::builder()
            .parent(&window)
            .font(Some(&font_ui))
            .size((200, input_height))
            .build(&mut filter_input)?;

        let mut tree = nwg::TreeView::default();
        nwg::TreeView::builder()
            .parent(&window)
            .font(Some(&font_tree))
            .image_list(Some(&image_list))
            .size((min_width, 400))
            .build(&mut tree)?;
        if let Some(hwnd) = tree.handle.hwnd() {
            tree_view::configure(hwnd);
        }

        let filter_timer = create_timer(&window, 200)?;
        let open_timer = create_timer(&window, 120)?;

        let ui = Ui {
            window,
            icon,
            font_ui,
            font_tree,
            image_list,
            btn_open,
            btn_close,
            btn_shot_full,
            btn_shot_neutral,
            btn_expand,
            btn_collapse,
            filter_label,
            filter_input,
            tree,
            filter_timer,
            open_timer,
            probe,
            probe_font,
            tree_probe_font,
        };

        // 宽度 = 工具栏内容宽度 + 边框余量
        let width = ui.toolbar_width() + dpi::scaled(28);
        ui.window.set_size(
            width.max(min_width) as u32,
            dpi::scaled(DEFAULT_HEIGHT) as u32,
        );
        center_on_work_area(&ui.window);

        Ok(ui)
    }

    /// 用界面字体（9pt）测量文字宽度。
    pub fn measure_ui_text(&self, text: &str) -> i32 {
        self.probe.text_width(text, &self.probe_font)
    }

    /// 用树字体测量文字宽度。
    pub fn measure_tree_text(&self, text: &str) -> i32 {
        self.probe.text_width(text, &self.tree_probe_font)
    }

    /// 六个工具栏按钮，顺序与界面一致。
    pub fn buttons(&self) -> [&nwg::Button; 6] {
        [
            &self.btn_open,
            &self.btn_close,
            &self.btn_shot_full,
            &self.btn_shot_neutral,
            &self.btn_expand,
            &self.btn_collapse,
        ]
    }

    /// 工具栏一行按钮占用的总宽度。
    pub fn toolbar_width(&self) -> i32 {
        let buttons = self.buttons();
        let padding = dpi::scaled(PADDING);
        let gap = dpi::scaled(BUTTON_GAP);
        let mut width = padding;

        for (index, button) in buttons.iter().enumerate() {
            width += button.size().0 as i32;
            if index + 1 < buttons.len() {
                width += gap;
            }
        }

        width + padding
    }

    /// 重新排列所有控件，返回自绘区域位置。
    pub fn layout(&self) -> Rects {
        let mut client: RECT = unsafe { zeroed() };
        if let Some(hwnd) = self.window.handle.hwnd() {
            unsafe {
                GetClientRect(hwnd, &mut client);
            }
        }

        let min_width = dpi::scaled(MIN_WIDTH);
        let padding = dpi::scaled(PADDING);
        let gap = dpi::scaled(BUTTON_GAP);
        let row_gap = dpi::scaled(ROW_GAP);
        let button_height = dpi::scaled(BUTTON_HEIGHT);
        let input_height = dpi::scaled(INPUT_HEIGHT);
        let label_height = dpi::scaled(LABEL_HEIGHT);

        let width = (client.right - client.left).max(min_width);
        let height = (client.bottom - client.top).max(dpi::scaled(MIN_HEIGHT));

        // 第一行：工具栏
        let mut y = padding;
        let mut x = padding;
        for button in self.buttons() {
            button.set_position(x, y);
            x += button.size().0 as i32 + gap;
        }
        y += button_height + row_gap;

        // 第二行：筛选
        self.filter_label
            .set_position(padding, y + (input_height - label_height) / 2);
        let filter_x = padding + self.filter_label.size().0 as i32 + gap;
        self.filter_input.set_position(filter_x, y);
        self.filter_input.set_size(
            (width - filter_x - padding).max(80) as u32,
            input_height as u32,
        );
        y += input_height + row_gap;

        // 第三行：路径（自绘）
        let path_rect = RECT {
            left: 0,
            top: y,
            right: width,
            bottom: y + label_height,
        };
        y += label_height + row_gap - 2;

        // 树区域
        let tree_height = (height - strip_height() * 2 - dpi::scaled(TREE_BOTTOM_GAP) - y).max(60);
        self.tree.set_position(padding, y);
        self.tree
            .set_size((width - padding * 2).max(80) as u32, tree_height as u32);

        Rects { path: path_rect }
    }
}

/// 创建一个初始停止的计时器。
#[allow(deprecated)]
fn create_timer(parent: &nwg::Window, interval: u32) -> Result<DebounceTimer, nwg::NwgError> {
    let mut timer = nwg::Timer::default();
    nwg::Timer::builder()
        .parent(parent)
        .interval(interval)
        .stopped(true)
        .build(&mut timer)?;
    Ok(timer)
}

/// 把窗口摆到所在显示器工作区的正中（工作区已排除任务栏）。
fn center_on_work_area(window: &nwg::Window) {
    use std::mem::size_of;
    use winapi::um::winuser::{
        GetMonitorInfoW, GetWindowRect, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let Some(hwnd) = window.handle.hwnd() else {
        return;
    };

    unsafe {
        let mut outer: RECT = zeroed();
        if GetWindowRect(hwnd, &mut outer) == 0 {
            return;
        }
        let width = outer.right - outer.left;
        let height = outer.bottom - outer.top;

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return;
        }

        let area = info.rcWork;
        let area_width = area.right - area.left;
        let area_height = area.bottom - area.top;

        // 窗口比工作区还大时贴左上角，避免算出负坐标跑到屏幕外
        let x = area.left + ((area_width - width) / 2).max(0);
        let y = area.top + ((area_height - height) / 2).max(0);

        window.set_position(x, y);
    }
}
