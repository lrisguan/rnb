<p align="center">
<img src="./assets/rnb.png"></img>
</p>

<h3 align="center">
rnb
</h3>

<p align="center">
<a ><b>English</b></a> | <a href="README.zh-CN.md"><b>中文</b></a>
</p>

## 📌 Introduction
**rnb** is a **terminal-first** Notebook editor and runner with Vim-like in-cell editing, 
execution workflow, and external resource opening for links and images.
> [!Important]
> Why called rnb?
> For this project comes with built-in R&B groove — you don’t need external audio libraries.
> Just run it, and your CPU fan will automatically perform a smooth, soulful, 
> syncopated beat.The louder the groove, the harder the code is working.

## Highlights

- Terminal UI for multi-cell browsing, editing, and scrolling
- Code cell execution and output rendering
- Markdown rendering (including common syntax, math, and tables)
> [!Note]
> Math formula rendering still has problems to be solved. One day maybe it will 
> be fixed or not yet anymore.
- Image preview in terminal, with external viewer opening for full quality
- Markdown link opening via command with selection popup when multiple items exist
- Kernel selector for Python environments
- Rhythm comes from your hardware, not a speaker.

## Requirements

- Rust toolchain (stable recommended)
- Linux/macOS terminal
- Python + Jupyter (recommended if you run Python code cells)
- xdg-open (for opening links/images externally on Linux)

## 🚀 Quick start
```bash
cargo install rnb
```
## CLI
```bash
rnb [notebook.ipynb]
rnb -h
rnb --help
rnb --version
```
## In-App Commands

| Commands      | Description                            |
|---------------|----------------------------------------|
| :open <file>  | Open a notebook file                   |
| :w [file]     | Save notebook                          |
| :wq [file]    | Save and quit                          |
| :x [file]     | Save and quit (same as :wq)            |
| :img          | Open images from current cell          |
| :ln           | Open links from current markdown cell  |
| :kernel       | Open kernel selector                   |
| :h            | Show help                              |

## Save Behavior

If no notebook path is currently bound (for example, when starting with just rnb),
save actions will prompt for a path.

This includes:

- Ctrl-s
- :w
- :wq
- :x

## Common Shortcuts

- Enter: enter current cell (normal mode)
- Esc (inside cell): leave cell editing
- r or Ctrl-r: run current cell
- Ctrl-Enter or Shift-Enter: run current cell
- Ctrl-R: run all code cells
- Ctrl-s: save
- Ctrl-b / Ctrl-f: scroll up/down and focus visible cell
- : enter command mode

## Vim-like In-Cell Editing

- i / I / a / A: enter insert mode
- o / O: open line below/above and enter insert mode
- Enter (normal mode): open line below and enter insert mode
- gg / G: go to first/last line
- h j k l or arrow keys: cursor movement

## Documentation

- English operation guide: docs/ops.md
- Chinese operation guide: docs/ops.zh-CN.md

## License
GPLv2
