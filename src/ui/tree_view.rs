//! 树控件的辅助操作：样式修正、命中测试、展开状态读取。

use native_windows_gui as nwg;
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::commctrl::{TVHITTESTINFO, TVM_HITTEST, TVM_SETINDENT, TVS_EDITLABELS};
use winapi::um::winuser::{
    GetCursorPos, GetWindowLongW, ScreenToClient, SendMessageW, SetWindowLongW, SetWindowPos,
    GWL_EXSTYLE, GWL_STYLE, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WM_SETREDRAW,
    WS_BORDER, WS_EX_CLIENTEDGE, WS_HSCROLL, WS_VSCROLL,
};

/// 树中当前可见的一行（只包含展开路径上的节点）。
#[derive(Debug, Clone)]
pub struct VisibleRow {
    /// 节点在 arena 中的下标
    pub node: usize,
    /// 层级，根文件夹为 0
    pub depth: i32,
    pub text: String,
    pub is_folder: bool,
    /// 是否是首行（显示完整文件名、星标图标）
    pub is_top: bool,
    pub is_open: bool,
    pub has_children: bool,
}

/// 修正原生树控件样式：补纵向滚动条、去掉 `TVS_EDITLABELS`、缩进 20px。
pub fn configure(hwnd: HWND) {
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let updated = (style | WS_VSCROLL) & !TVS_EDITLABELS & !WS_BORDER;

        if updated != style {
            SetWindowLongW(hwnd, GWL_STYLE, updated as i32);
        }

        // 凹陷的客户区边框
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_CLIENTEDGE == 0 {
            SetWindowLongW(hwnd, GWL_EXSTYLE, (ex_style | WS_EX_CLIENTEDGE) as i32);
        }

        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );

        SendMessageW(
            hwnd,
            TVM_SETINDENT,
            crate::platform::dpi::scaled(20) as usize,
            0,
        );
    }
}

/// 滚动条宽度（随 DPI 变化）。
pub fn scrollbar_size() -> i32 {
    unsafe {
        let width = winapi::um::winuser::GetSystemMetrics(winapi::um::winuser::SM_CXVSCROLL);
        if width > 0 {
            width
        } else {
            17
        }
    }
}

/// 取树中一行的像素高度（用根节点的矩形推算）。没有节点时返回 0。
pub fn row_height(tree: &nwg::TreeView) -> i32 {
    use winapi::um::commctrl::{TVGN_ROOT, TVM_GETITEMRECT, TVM_GETNEXTITEM};

    let Some(hwnd) = tree.handle.hwnd() else {
        return 0;
    };

    let root = unsafe { SendMessageW(hwnd, TVM_GETNEXTITEM, TVGN_ROOT as usize, 0) };
    if root == 0 {
        return 0;
    }

    let mut rect: RECT = unsafe { std::mem::zeroed() };
    // TVM_GETITEMRECT 的约定：wParam 为 TRUE 时，RECT 起始处要放 HTREEITEM
    unsafe {
        *(&mut rect as *mut RECT as *mut isize) = root;
    }
    let ok = unsafe { SendMessageW(hwnd, TVM_GETITEMRECT, 1, &mut rect as *mut RECT as isize) };
    if ok == 0 {
        return 0;
    }

    rect.bottom - rect.top
}

/// 批量修改树状态前挂起重绘，避免每次都整块刷新。
pub fn set_redraw(tree: &nwg::TreeView, enabled: bool) {
    if let Some(hwnd) = tree.handle.hwnd() {
        unsafe {
            SendMessageW(hwnd, WM_SETREDRAW, enabled as usize, 0);
        }
    }
}

pub fn refresh(tree: &nwg::TreeView) {
    if let Some(hwnd) = tree.handle.hwnd() {
        unsafe {
            winapi::um::winuser::InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }
}

pub fn set_scrollbars(hwnd: HWND, vertical: bool, horizontal: bool) {
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let mut updated = style & !(WS_HSCROLL | WS_VSCROLL);
        if vertical {
            updated |= WS_VSCROLL;
        }
        if horizontal {
            updated |= WS_HSCROLL;
        }

        if updated != style {
            SetWindowLongW(hwnd, GWL_STYLE, updated as i32);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }
}

/// 复制一个 TreeItem 句柄（该类型没有实现 Clone）。
pub fn copy_item(item: &nwg::TreeItem) -> nwg::TreeItem {
    nwg::TreeItem {
        handle: item.handle,
    }
}

pub fn is_expanded(tree: &nwg::TreeView, item: &nwg::TreeItem) -> bool {
    tree.item_state(item)
        .map(|state| state.contains(nwg::TreeItemState::EXPANDED))
        .unwrap_or(false)
}

/// 命中测试：返回坐标处的树项。
pub fn hit_test(tree: &nwg::TreeView, x: i32, y: i32) -> Option<nwg::TreeItem> {
    let hwnd = tree.handle.hwnd()?;

    let mut info: TVHITTESTINFO = unsafe { std::mem::zeroed() };
    info.pt = POINT { x, y };

    let result = unsafe { SendMessageW(hwnd, TVM_HITTEST, 0, &mut info as *mut _ as isize) };

    if result == 0 || info.hItem.is_null() {
        None
    } else {
        Some(nwg::TreeItem { handle: info.hItem })
    }
}

/// 取鼠标位置（相对窗口客户区）。
pub fn cursor_client_pos(hwnd: HWND) -> (i32, i32) {
    unsafe {
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        ScreenToClient(hwnd, &mut point);
        (point.x, point.y)
    }
}
