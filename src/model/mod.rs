//! 收藏夹树模型：索引数组（arena），子节点存下标，节点下标即树控件的 lParam。

/// 一个文件夹，或一条书签链接。
#[derive(Debug, Clone)]
pub struct BookmarkNode {
    pub title: String,
    pub is_folder: bool,
    pub url: Option<String>,
    /// 收藏夹导出的 favicon（base64 data URI），仅解析不使用。
    #[allow(dead_code)]
    pub icon: Option<String>,
    pub add_date: Option<String>,
    /// 子节点在 arena 中的下标（仅文件夹使用）
    pub children: Vec<usize>,
}

/// 节点集合。下标 0 是解析器创建的虚拟根节点。
#[derive(Debug, Default)]
pub struct Arena {
    pub nodes: Vec<BookmarkNode>,
}

impl Arena {
    pub fn new() -> Arena {
        Arena { nodes: Vec::new() }
    }

    /// 创建虚拟根节点并返回其下标（恒为 0）。
    pub fn with_root(title: String) -> Arena {
        let mut arena = Arena::new();
        arena.add_folder(title);
        arena
    }

    pub fn root(&self) -> usize {
        0
    }

    pub fn add_folder(&mut self, title: String) -> usize {
        self.nodes.push(BookmarkNode {
            title,
            is_folder: true,
            url: None,
            icon: None,
            add_date: None,
            children: Vec::new(),
        });
        self.nodes.len() - 1
    }

    pub fn add_link(
        &mut self,
        title: String,
        url: Option<String>,
        icon: Option<String>,
        add_date: Option<String>,
    ) -> usize {
        self.nodes.push(BookmarkNode {
            title,
            is_folder: false,
            url,
            icon,
            add_date,
            children: Vec::new(),
        });
        self.nodes.len() - 1
    }

    /// 根节点的顶层子项（通常只有一个「收藏夹栏」）。
    pub fn top_level(&self, root: usize) -> Vec<usize> {
        self.nodes[root].children.clone()
    }

    pub fn count_links(&self, index: usize) -> usize {
        let mut total = 0;
        let mut stack = vec![index];

        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            if node.is_folder {
                stack.extend(node.children.iter().copied());
            } else {
                total += 1;
            }
        }

        total
    }

    /// 统计某节点下的文件夹总数（含节点自身）。
    pub fn count_folders(&self, index: usize) -> usize {
        let mut total = 0;
        let mut stack = vec![index];

        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            if node.is_folder {
                total += 1;
                stack.extend(node.children.iter().copied());
            }
        }

        total
    }

    /// 业务口径的文件夹总数：不含虚拟根，也不含顶层根文件夹本身。
    pub fn count_folders_excluding_roots(&self, root: usize) -> usize {
        self.nodes[root]
            .children
            .iter()
            .filter(|&&c| self.nodes[c].is_folder)
            .map(|&c| self.count_folders(c).saturating_sub(1))
            .sum()
    }

    /// 深度优先收集某节点下的全部链接，保持原始顺序。
    pub fn collect_urls(&self, index: usize, out: &mut Vec<String>) {
        let mut stack = vec![index];

        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            if node.is_folder {
                for &child in node.children.iter().rev() {
                    stack.push(child);
                }
            } else if let Some(url) = &node.url {
                out.push(url.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Arena {
        let mut arena = Arena::with_root("收藏夹".to_string());
        let bar = arena.add_folder("收藏夹栏".to_string());
        let sub = arena.add_folder("子文件夹".to_string());
        let link_a = arena.add_link("A".to_string(), Some("https://a".to_string()), None, None);
        let link_b = arena.add_link("B".to_string(), Some("https://b".to_string()), None, None);
        arena.nodes[sub].children = vec![link_b];
        arena.nodes[bar].children = vec![sub, link_a];
        arena.nodes[0].children = vec![bar];
        arena
    }

    #[test]
    fn counts_match_python_semantics() {
        let arena = sample();
        assert_eq!(arena.count_links(0), 2);
        // 含根文件夹自身
        assert_eq!(arena.count_folders(1), 2);
        // 业务口径：不含根文件夹
        assert_eq!(arena.count_folders_excluding_roots(0), 1);
    }

    #[test]
    fn collects_urls_in_order() {
        let arena = sample();
        let mut urls = Vec::new();
        arena.collect_urls(1, &mut urls);
        assert_eq!(urls, vec!["https://b".to_string(), "https://a".to_string()]);
    }
}
