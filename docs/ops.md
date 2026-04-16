# rnb Operation Guide

This document is a practical user guide for daily workflow: start, navigate, edit, run, save, and open resources.

## 1. Start

- Start directly: rnb
- Open a notebook file: rnb path/to/notebook.ipynb
- Show help: rnb -h or rnb --help
- Show version: rnb --version

After startup, you enter the main UI and see status hints at the bottom.

## 2. Basic Navigation

- Move between cells: j / k or arrow keys
- Enter current cell: Enter
- Leave cell editing: Esc
- Page scroll: Ctrl-b (up) / Ctrl-f (down)

Note: scrolling also updates focus to the corresponding visible cell.

## 3. Run Code

- Run current cell: r or Ctrl-r
- Run current cell: Ctrl-Enter or Shift-Enter
- Run all code cells: Ctrl-R or :ra
- Run current and above: :ro or Ctrl-k
- Run current and below: :rb or Ctrl-j

## 4. In-Cell Editing (Vim-like)

Inside a cell, Normal and Insert modes are supported.

### 4.1 Enter Insert Mode

- i: insert at cursor
- I: insert at first non-whitespace of the line
- a: append after cursor
- A: append at end of line
- o: open line below and enter insert mode
- O: open line above and enter insert mode
- Enter (normal mode): open line below and enter insert mode

### 4.2 Movement in Normal Mode

- h / j / k / l: left/down/up/right
- Arrow keys: left/down/up/right
- gg: jump to first line
- G: jump to last line

### 4.3 Common Keys in Insert Mode

- Enter: newline
- Backspace: delete previous character
- Delete: delete current character
- Esc: back to normal mode

## 5. Open and Save Files

### 5.1 Open Notebook

- :open <path>

Example: :open assets/rich_demo.ipynb

### 5.2 Save Notebook

- :w: save
- :w <path>: save as
- :wq / :wq <path>: save and quit
- :x / :x <path>: save and quit
- Ctrl-s: quick save

Important behavior:

- If the session is not yet bound to a file path (for example started with rnb only), using :w, :wq, :x, or Ctrl-s will ask for a save path.
- Type the path and press Enter to finish saving.

## 6. Open Links and Images

### 6.1 Open Markdown Links

- :ln

If the current markdown cell contains multiple links, a selection popup appears. Use j/k or arrow keys, then Enter.

### 6.2 Open Image Resources

- :img

Sources include:

- image URLs in markdown
- image outputs in code cells

When multiple candidates exist, a selection popup appears as well.

External resources are opened via xdg-open on Linux.

## 7. Kernel Selection

- :kernel

Inside kernel selector:

- Move selection: j/k or arrow keys
- Apply: Enter
- Cancel: Esc

## 8. Help Panel

- :h or ?
- Close: Esc

Recommended: read the help panel first, then use this guide as a reference.

## 9. FAQ

### 9.1 Why do images look less sharp in terminal?

Terminal rendering is character-grid based, so detail is limited. Use :img to open in system image viewer for full quality.

### 9.2 Why do :ln or :img only show a status message sometimes?

If there are no candidates in the current cell, only status text is shown. If there are multiple candidates, a popup selector is shown.

### 9.3 Why am I asked for save path on first save?

This prevents accidental save to an unintended default filename and makes save destination explicit.

## 10. Recommended Workflow

1. Start rnb (with or without file)
2. Enter a cell with Enter, edit using i/I/a/A/o/O/Enter
3. Run with r or Ctrl-r
4. Open resources with :ln / :img
5. First save with :w <path>, then quick save via Ctrl-s
