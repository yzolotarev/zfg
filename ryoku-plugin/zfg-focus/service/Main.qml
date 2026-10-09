pragma ComponentBehavior: Bound

import QtQuick
import Quickshell.Io

Item {
    id: root
    property var pluginApi
    readonly property var settings: pluginApi ? pluginApi.pluginSettings : null
    readonly property string executable: settings && settings.binary ? settings.binary : "zfg"
    readonly property int pollSeconds: settings && settings.pollSeconds ? settings.pollSeconds : 2
    property int focusCount: 0
    property int focusLimit: 5
    property string focusFirst: ""
    property int laterCount: 0
    property bool focusFull: false
    property int truncateAt: 28
    property bool showFocusCounter: true
    property bool showLaterCounter: true
    property string focusEmptyLabel: ""
    property var labels: ({})
    property string output: ""

    function refresh() {
        if (!statusProc.running) {
            statusProc.command = [root.executable, "status", "--json"];
            statusProc.running = true;
        }
    }
    function launch(command) {
        launchProc.command = [root.executable, command];
        launchProc.running = true;
    }

    Timer { interval: root.pollSeconds * 1000; running: true; repeat: true; triggeredOnStart: true; onTriggered: root.refresh() }
    Process {
        id: statusProc
        stdout: StdioCollector { onStreamFinished: root.output = text }
        onExited: {
            try {
                const data = JSON.parse(root.output.trim());
                root.focusCount = Number(data.focus_count) || 0;
                root.focusLimit = Number(data.focus_limit) || 5;
                root.focusFirst = String(data.focus_first || "");
                root.laterCount = Number(data.later_count) || 0;
                root.focusFull = data.focus_full === true;
                root.focusEmptyLabel = String(data.focus_empty_label || "");
                root.labels = data.panel_labels || {};
                if (data.panel) {
                    root.truncateAt = Number(data.panel.truncate_at);
                    root.showFocusCounter = data.panel.show_focus_counter !== false;
                    root.showLaterCounter = data.panel.show_later_counter !== false;
                }
            } catch (e) {}
            root.output = "";
        }
    }
    Process { id: launchProc }
}
