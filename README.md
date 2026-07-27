<div align="center">

# Web Favorites Fast Scrnshot

[![Python](https://img.shields.io/badge/Python-3.11-blue.svg)](#)
[![Platform](https://img.shields.io/badge/Platform-Windows-win.svg)](#)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

</div>

本地 **网页收藏夹 HTML** 树形查看器 + **快速长图截图** 小工具（Netscape Bookmark 格式）

## 📸 工具截图

![img](/screenshot/demo1.png)
![img](/screenshot/demo2.png)

## ✨ 特色功能

| 功能 | 说明 |
| --- | --- |
| 树形预览 | 文件夹可展开/折叠，层级清晰 |
| 筛选 | 按标题或 URL 子串过滤，并展开匹配路径 |
| 完整截图 | 按当前展开状态绘制长图 |
| 中性截图 | 对超长标题做截断绘制长图 |
| 信息栏统计 | 显示当前可见 / 文件总数（文件夹不含根行） |

## 💻 收藏夹模板兼容
- Edge
- Chrome

## ⬇️ 下载使用

前往 [Releases](https://github.com/NeetheCheeBao/WebFavoritesFastScrnshot/releases) 页面下载

## 🛠️ 本地编译

```bash
py -3 -m PyInstaller --noconfirm --clean --onefile --windowed --name WebFavoritesFastScrnshot --icon assets/icon.ico --add-data "assets/icon.ico;assets" main.py
```

产物：`dist\WebFavoritesFastScrnshot.exe`

## ⚖️ 许可证

本项目采用 MIT 许可证 - 详情请参阅 [LICENSE](LICENSE) 文件。