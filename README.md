# cdlog

A fast `cd` history logger and fuzzy-finder navigation plugin for ZSH.

## Installation

Build the binary:

```zsh
cargo build --release
cargo install --path .
```

## Setup

Add the following to your `~/.zshrc`:

```zsh
source /path/to/cdlog.zsh

# Bind widget to a shortcut (e.g. Ctrl+X Ctrl+F)
bindkey '^x^f' cdlog-widget
```

## Usage

- Press your configured keybinding (e.g. `Ctrl+X Ctrl+F`) to fuzzy-find and jump to a previous directory.
- Or run `cdlog-cd` directly in your terminal.

## Configuration

| Variable             | Description                                      | Default        |
| ---                  | ---                                              | ---            |
| `CDLOG_PATH`         | Path to the history log file                     | `$HOME/.cdlog` |
| `CDLOG_FUZZY_FINDER` | Fuzzy finder command (`fzf`, `sk`, `peco`, etc.) | `fzf`          |
