<p align="center">
<img src="./assets/rnb.png"></img>
</p>

<h3 align="center">
rnb
</h3>

<p align="center">
<a href="README.md"><b>English</b></a> | <a><b>中文</b></a>
</p>

## 📌 简介
**rnb** 是一款**以终端为核心**的笔记本编辑器与运行器，支持类 Vim 的单元格内编辑、执行流程，以及链接、图片等外部资源打开功能。
> [!Important]
> 为什么取名为 rnb?
> 因为本项目自带原生 R&B 律动，无需额外音频库。运行即可享受 CPU 风扇带来
> 的丝滑灵魂乐切分节奏，律动越响，代码越努力。

## 核心特性

- 终端界面，支持多单元格浏览、编辑与滚动
- 代码单元格执行与输出渲染
- Markdown 渲染（包含常用语法、数学公式、表格）
> [!Note]
> 数学公式渲染仍存在待解决问题，未来可能修复，也可能不再处理。
- 终端内图片预览，可调用外部查看器打开高清原图
- 通过命令打开 Markdown 链接，多链接时弹出选择框
- Python 环境内核选择器
- 节奏来自硬件，而非扬声器。

## 环境要求

- Rust 工具链（推荐稳定版）
- Linux/macOS 终端
- Python + Jupyter（如需运行 Python 代码单元格）
- xdg-open（Linux 下用于外部打开链接/图片）
- chafa
> [!Tip]
> 这是依赖所需的库，要求版本高于 1.8.0。
> 可按如下方式安装：
> ```bash
> sudo apt install libchafa-dev
> # 然后可查看版本
> pkg-config --modversion chafa
> ```

## 🚀 快速开始
```bash
cargo install rnb
```

## 命令行使用
```bash
rnb [notebook.ipynb]
rnb -h
rnb --help
rnb --version
```

## 应用内命令

| 命令          | 说明                                |
|---------------|------------------------------------|
| :open <file>  | 打开笔记本文件                       |
| :w [file]     | 保存笔记本                          |
| :wq [file]    | 保存并退出                          |
| :x [file]     | 保存并退出（同 :wq）                 |
| :img          | 打开当前单元格中的图片                |
| :ln           | 打开当前 Markdown 单元格中的链接      |
| :kernel       | 打开内核选择器                       |
| :h            | 显示帮助                            |

## 保存行为

如果当前未绑定笔记本路径（例如直接运行 `rnb` 启动），
执行保存操作时会提示输入路径。

包括：

- Ctrl-s
- :w
- :wq
- :x

## 常用快捷键

- Enter：进入当前单元格（普通模式）
- Esc（单元格内）：退出单元格编辑
- r 或 Ctrl-r：运行当前单元格
- Ctrl-Enter 或 Shift-Enter：运行当前单元格
- Ctrl-R：运行所有代码单元格
- Ctrl-s：保存
- Ctrl-b / Ctrl-f：上下滚动并聚焦可见单元格
- : 进入命令模式

## 类 Vim 单元格内编辑

- i / I / a / A：进入插入模式
- o / O：在下方/上方新建行并进入插入模式
- Enter（普通模式）：在下方新建行并进入插入模式
- gg / G：跳转到首行/末行
- h j k l 或方向键：光标移动

## 文档

- 英文操作指南：docs/ops.md
- 中文操作指南：docs/ops.zh-CN.md

## 许可证
GPLv2