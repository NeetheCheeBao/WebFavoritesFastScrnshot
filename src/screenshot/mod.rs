//! 把当前展开状态渲染成一张长图。
//!
//! 宽度只由标题长度与层级决定，与窗口大小无关；按 2 倍像素密度绘制，
//! 保证文字与图标清晰。两种模式：
//! * `Full`：所有标题完整绘制，宽度取最长一行；
//! * `Neutral`：超长标题截断为省略号，宽度按整体分布计算。

pub mod gdi;
pub mod png;

use crate::ui::tree_view::VisibleRow;

use gdi::{rgb, Canvas, Font};

/// 绘制倍数。
const SCALE: i32 = 2;
/// 截图使用的字体族。
const FONT_FAMILY: &str = "Microsoft YaHei";
/// 省略号。
const ELLIPSIS: &str = "…";

const WHITE: u32 = rgb(255, 255, 255);
const BORDER: u32 = rgb(224, 224, 224);
const TEXT: u32 = rgb(26, 26, 26);
const INDICATOR: u32 = rgb(102, 102, 102);

/// 截图模式。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShotMode {
    Full,
    Neutral,
}

/// 渲染长图，返回 PNG 字节流。
pub fn render(rows: &[VisibleRow], mode: ShotMode) -> Result<Vec<u8>, String> {
    if rows.is_empty() {
        return Err("没有可绘制的内容".to_string());
    }

    let s = SCALE;
    let row_h = 28 * s;
    let pad_x = 12 * s;
    let pad_y = 10 * s;
    let indent_w = 20 * s;
    let icon_size = 20 * s;
    let indicator_w = 16 * s;
    let gap = 8 * s;
    let font_px = 16 * s;

    let probe = Canvas::new(8, 8).ok_or_else(|| "无法创建测量画布".to_string())?;
    let font = Font::new(font_px, FONT_FAMILY);
    if !font.is_valid() {
        return Err("无法创建绘图字体".to_string());
    }

    // 中性模式先算一个标题宽度上限，根节点与一级项必须完整显示
    let cap = if mode == ShotMode::Neutral {
        Some(neutral_cap(&probe, rows, &font, s))
    } else {
        None
    };

    let mut texts = Vec::with_capacity(rows.len());
    let mut widths = Vec::with_capacity(rows.len());

    for row in rows {
        let mut text = row.text.clone();
        if let Some(cap) = cap {
            if row.depth > 0 {
                text = fit_text(&text, cap, |candidate| probe.text_width(candidate, &font));
            }
        }

        let chrome = pad_x + row.depth * indent_w + indicator_w + icon_size + gap;
        widths.push(chrome + probe.text_width(&text, &font) + pad_x);
        texts.push(text);
    }

    let width = widths.iter().copied().max().unwrap_or(0).max(320 * s) + 8 * s;
    let height = pad_y * 2 + rows.len() as i32 * row_h;

    let canvas = Canvas::new(width, height).ok_or_else(|| "无法创建绘图画布".to_string())?;
    canvas.fill(0, 0, width, height, WHITE);
    canvas.stroke_rect(0, 0, width - 1, height - 1, BORDER, s);

    let text_h = canvas.text_height(&font);

    for (index, row) in rows.iter().enumerate() {
        let top = pad_y + index as i32 * row_h;
        let mut left = pad_x + row.depth * indent_w;

        if row.has_children {
            let center_x = left + indicator_w / 2;
            let center_y = top + row_h / 2;
            let points = if row.is_open {
                vec![
                    (center_x - 5 * s, center_y - 3 * s),
                    (center_x + 5 * s, center_y - 3 * s),
                    (center_x, center_y + 4 * s),
                ]
            } else {
                vec![
                    (center_x - 3 * s, center_y - 5 * s),
                    (center_x - 3 * s, center_y + 5 * s),
                    (center_x + 4 * s, center_y),
                ]
            };
            canvas.polygon(&points, INDICATOR, INDICATOR);
        }
        left += indicator_w;

        let icon = if row.is_top {
            IconKind::Star
        } else if row.is_folder {
            IconKind::Folder
        } else {
            IconKind::Link
        };
        draw_icon(
            &canvas,
            icon,
            left,
            top + (row_h - icon_size) / 2,
            icon_size,
        );
        left += icon_size + gap;

        let text_top = top + (row_h - text_h) / 2;
        canvas.text(left, text_top, &texts[index], &font, TEXT);
    }

    let pixels = canvas.to_rgba();
    Ok(png::encode_rgba(width as u32, height as u32, &pixels))
}

/// 把 PNG 字节写入磁盘。
pub fn save(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

#[derive(Clone, Copy)]
enum IconKind {
    Folder,
    Star,
    Link,
}

fn draw_icon(canvas: &Canvas, kind: IconKind, x: i32, y: i32, size: i32) {
    let scale = |value: f64| (value * size as f64 / 16.0).round() as i32;

    match kind {
        IconKind::Folder => {
            canvas.rect(
                x + scale(2.0),
                y + scale(6.0),
                x + scale(13.0),
                y + scale(13.0),
                rgb(255, 204, 51),
                rgb(201, 160, 0),
            );
            canvas.rect(
                x + scale(2.0),
                y + scale(3.0),
                x + scale(7.0),
                y + scale(6.0),
                rgb(230, 184, 0),
                rgb(201, 160, 0),
            );
        }
        IconKind::Star => {
            let center_x = x as f64 + size as f64 / 2.0;
            let center_y = y as f64 + size as f64 / 2.0;
            let radius = size as f64 * 0.40;
            let mut points = Vec::with_capacity(10);

            for index in 0..10 {
                let angle = (-90.0 + index as f64 * 36.0).to_radians();
                let length = if index % 2 == 0 {
                    radius
                } else {
                    radius * 0.45
                };
                points.push((
                    (center_x + length * angle.cos()).round() as i32,
                    (center_y + length * angle.sin()).round() as i32,
                ));
            }

            canvas.polygon(&points, rgb(245, 197, 24), rgb(212, 160, 23));
        }
        IconKind::Link => {
            canvas.rect(
                x + scale(4.0),
                y + scale(2.0),
                x + scale(11.0),
                y + scale(13.0),
                rgb(240, 240, 240),
                rgb(107, 107, 107),
            );
            let thickness = (size / 16).max(1);
            for line in [6.0, 8.0, 10.0] {
                canvas.hline(
                    x + scale(6.0),
                    x + scale(9.0),
                    y + scale(line),
                    rgb(136, 136, 136),
                    thickness,
                );
            }
        }
    }
}

/// 中性模式的标题宽度上限：90 分位宽度、一级标题宽度、下限三者取最大。
fn neutral_cap(probe: &Canvas, rows: &[VisibleRow], font: &Font, scale: i32) -> i32 {
    let mut widths: Vec<i32> = rows
        .iter()
        .map(|row| probe.text_width(&row.text, font))
        .filter(|width| *width > 0)
        .collect();

    if widths.is_empty() {
        return 280 * scale;
    }

    let must_fit = rows
        .iter()
        .filter(|row| row.depth <= 1)
        .map(|row| probe.text_width(&row.text, font))
        .max()
        .unwrap_or(0);

    widths.sort_unstable();
    let index = (((widths.len() - 1) as f64) * 0.90).round() as usize;
    let bulk = widths[index.min(widths.len() - 1)];

    bulk.max(must_fit).max(280 * scale)
}

/// 截断文字并补省略号，使宽度不超过 `max_width`。
fn fit_text(text: &str, max_width: i32, measure: impl Fn(&str) -> i32) -> String {
    if max_width <= 0 {
        return String::new();
    }
    if measure(text) <= max_width {
        return text.to_string();
    }
    if measure(ELLIPSIS) > max_width {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut low = 0usize;
    let mut high = chars.len();
    let mut best = ELLIPSIS.to_string();

    while low <= high {
        let middle = (low + high) / 2;
        let mut candidate: String = chars[..middle].iter().collect();
        candidate.push_str(ELLIPSIS);

        if measure(&candidate) <= max_width {
            best = candidate;
            low = middle + 1;
        } else if middle == 0 {
            break;
        } else {
            high = middle - 1;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_with_ellipsis() {
        let measure = |text: &str| text.chars().count() as i32 * 10;
        assert_eq!(fit_text("abcdef", 200, measure), "abcdef");
        assert_eq!(fit_text("abcdef", 45, measure), "abc…");
        assert_eq!(fit_text("abcdef", 5, measure), "");
    }

    /// 生成样张供人工检查排版（默认忽略）。
    /// 运行：`cargo test --offline -- --ignored --nocapture`
    #[test]
    #[ignore = "生成排版样张，供人工检查"]
    fn renders_sample_image() {
        let rows = vec![
            VisibleRow {
                node: 0,
                depth: 0,
                text: "favorites_2026_7_27.html".to_string(),
                is_folder: true,
                is_top: true,
                is_open: true,
                has_children: true,
            },
            VisibleRow {
                node: 1,
                depth: 1,
                text: "开发工具".to_string(),
                is_folder: true,
                is_top: false,
                is_open: true,
                has_children: true,
            },
            VisibleRow {
                node: 2,
                depth: 2,
                text: "WebFavoritesFastScrnshot 仓库".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 3,
                depth: 2,
                text: "Rust 程序设计语言（中文版）".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 4,
                depth: 1,
                text: "资料".to_string(),
                is_folder: true,
                is_top: false,
                is_open: false,
                has_children: true,
            },
            VisibleRow {
                node: 5,
                depth: 2,
                text: "一条特别特别长的书签标题，用来观察中性截图模式下的省略号截断效果是否正常"
                    .to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 6,
                depth: 1,
                text: "示例站点 & 实体".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 7,
                depth: 1,
                text: "GitHub".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 8,
                depth: 1,
                text: "Rust 官网".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 9,
                depth: 1,
                text: "文档".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 10,
                depth: 1,
                text: "博客".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 11,
                depth: 1,
                text: "工具".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 12,
                depth: 1,
                text: "论坛".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 13,
                depth: 1,
                text: "镜像".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 14,
                depth: 1,
                text: "规范".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
            VisibleRow {
                node: 15,
                depth: 1,
                text: "教程".to_string(),
                is_folder: false,
                is_top: false,
                is_open: false,
                has_children: false,
            },
        ];

        std::fs::create_dir_all(".temp").ok();
        for (mode, name) in [
            (ShotMode::Full, ".temp/render_full.png"),
            (ShotMode::Neutral, ".temp/render_neutral.png"),
        ] {
            let png = render(&rows, mode).expect("渲染失败");
            std::fs::write(name, png).expect("写入样张失败");
        }
    }
}
