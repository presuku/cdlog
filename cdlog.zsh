# cdlog.zsh - Fast cd history logger and fuzzy-finder navigation for ZSH
# https://github.com/presuku/cdlog

# Log file destination ($CDLOG_PATH or ~/.cdlog)
_cdlog_path="${CDLOG_PATH:-$HOME/.cdlog}"

# Binary executable resolution
_cdlog_bin="cdlog"
if (( ! $+commands[cdlog] )) && [[ -x "${0:A:h}/target/release/cdlog" ]]; then
  _cdlog_bin="${0:A:h}/target/release/cdlog"
fi

# Record path on directory change
__cdlog_chpwd_hook() {
  print -r -- "${(q)PWD}" | "$_cdlog_bin" "$_cdlog_path"
}

# Select directory and cd (used directly and as ZLE widget)
cdlog-cd() {
  local target
  target=$(print -r -- "${(q)PWD}" | "$_cdlog_bin" -g "$_cdlog_path" | "${(@Q)${(@z)CDLOG_FUZZY_FINDER:-fzf}}")
  if [[ -n "$target" ]]; then
    BUFFER="cd ${(Q)target}"
    zle accept-line
  fi
  zle redisplay
}

autoload -Uz add-zsh-hook
add-zsh-hook chpwd __cdlog_chpwd_hook

# Register as ZLE widget
zle -N cdlog-widget cdlog-cd
