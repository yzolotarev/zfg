# Ryoku integration

The `ryoku-plugin/zfg-focus/` source implements the `zfg-focus` QS Bar plugin. The deployed user copy is under `~/Documents/ryoku-plugins/zfg-focus/`; Ryoku's installer maintains the installed copy.

The plugin polls `zfg status --json` (default 2 seconds), shows the first Focus task, the current count/limit, and `+`. The task/count region launches `zfg focus`; `+` launches `zfg launcher`. Its panel provides Focus/Later/launcher shortcuts.

On this development machine, the plugin was installed using `ryoku plugin add`, and placed after `clock` with `ryoku-shell bar move`. Existing `todo-file` was not edited or removed. The widget is local-only and has not been shared to Ryostore.

If ZFG is not found in the shell's PATH, change the executable setting under QS Bar Settings > Community. Current default `zfg` resolves via the user's PATH (`~/.local/bin` is present in the observed login PATH).
