# Integrations

## Note and Calendar

Settings edit three shared action types: URI, stable desktop application ID, and executable plus JSON argv. Desktop applications are discovered through GIO on native installs. Defaults are `obsidian://new` and the Google Calendar event creation page. No credentials are stored.

`src/integrations.rs` exposes `ExternalLauncher`, `NativeLauncher`, `PortalLauncher`, capabilities, validation and launching. Flatpak is detected through `FLATPAK_ID` or `/.flatpak-info`. In the sandbox, host application discovery/launch and custom commands are disabled; URI opening delegates to GIO's portal-aware path. These sandbox actions still require an end-to-end Flatpak test.

URI templates support `{timestamp}`, `{date}`, `{time}` and `{language}`. Date/time use local time; inserted values are percent-encoded. Unknown or malformed placeholders and invalid/dangerous schemes are rejected. Custom commands use literal argv, never an inserted shell, and do not expand templates. Invoking an explicitly configured shell executable is still the user's own trusted advanced action.

A failed external action leaves the launcher open and shows an error; it is not reported as a successful action.

## Global shortcut

Settings offers explicit enable/disable controls for the XDG Desktop Portal GlobalShortcuts API. Availability is detected asynchronously. The desktop may ask the user to approve or change the suggested `Ctrl+Alt+Space` shortcut. No binding is installed merely by starting ZFG.

The portal session keeps the existing GApplication process alive to receive activation. Activation invokes the same `launcher` application action as CLI/panel launch requests. A second enable call reuses the current registration; disabling or losing the portal releases the session and application hold. Approval and delivery remain desktop-backend-dependent. The local backend advertised interface version 1; interactive binding approval has not been tested.

For an explicit compositor fallback, **Copy Hyprland binding** copies:

```ini
bind = SUPER, Z, exec, zfg launcher
```

Review conflicts and configure it through Ryoku Hub > Keybinds. ZFG never silently edits compositor configuration.

## Ryoku status adapter

`zfg status --json` preserves task status fields and additionally exposes panel preferences, the localized empty-Focus label, and localized panel action labels. The widget polls every two seconds, honors counter/width preferences, and opens the same CLI actions. The Later counter is subdued and hidden when empty. It does not read or mutate task files directly.
