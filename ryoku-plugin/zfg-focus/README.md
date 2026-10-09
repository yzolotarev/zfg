# ZFG Focus for Ryoku

A small QS Bar adapter for ZFG's plain-text Focus/Later lists. It does not read or write task files itself; it runs `zfg status --json` and uses the ZFG CLI to open views. The widget polls at the configured 1–10 second interval (default 2 seconds), shows the first Focus task and `N/5`, and routes `+` to `zfg launcher`.

## Commands and data access

- Runs only the configured executable (default `zfg`) with argv arrays: `status --json`, `focus`, `later`, and `launcher`.
- Reads the status output from stdout. It does not read/write files, run a shell, use the network, or request privileges.
- Configure the executable name in QS Bar Settings > Community if it is not on the shell's PATH.
- `focusFirst` is elided by the bar's available width. The Focus count and launcher action remain visible.

## Install

```sh
ryoku plugin validate ~/Documents/ryoku-plugins/zfg-focus
ryoku plugin add ~/Documents/ryoku-plugins/zfg-focus --bar --yes
ryoku-shell bar move zfg-focus --section center --after clock
```

The widget appears near the clock. QS Bar Settings > Layout can move it; QS Bar Settings > Community can remove it. The plugin is local user integration and is not published.
