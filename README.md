<div align="center">
<h1>Web Favorites Fast Scrnshot</h1>
<img src="assets\icon.ico" width="128" alt="logo" />

[![Python](https://img.shields.io/badge/Python-3.11-blue.svg)](#)
[![Platform](https://img.shields.io/badge/Platform-Windows-win.svg)](#)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

本地 **网页收藏夹 HTML** 树形查看器 + **快速长图截图** 小工具（Netscape Bookmark 格式）

</div>

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

## 📚 依赖库

运行 / 编译前请先安装：

| 库 | 用途 |
| --- | --- |
| Pillow | 长图截图绘制 |
| windnd | 拖放文件支持 |

## 🛠️ 本地编译

1. 克隆仓库到本地
```bash
git clone https://github.com/NeetheCheeBao/WebFavoritesFastScrnshot.git
```

或

```bash
gh repo clone NeetheCheeBao/WebFavoritesFastScrnshot
```

2. 安装依赖库
```bash
pip install Pillow windnd
```

3. 一键编译
```bash
.\build.bat
```

产物位置：`dist\WebFavoritesFastScrnshot.exe`

## ⚖️ 许可证

本项目采用 MIT 许可证 - 详情请参阅 [LICENSE](LICENSE) 文件。