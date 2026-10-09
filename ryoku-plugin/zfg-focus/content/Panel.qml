import QtQuick
import Ryoku.PluginKit.Singletons

Item {
    id: root
    property var pluginApi
    property string density: "full"
    property real s: 1
    property real widthBudget: 360
    property bool active: false
    readonly property var service: pluginApi ? pluginApi.mainInstance : null
    implicitWidth: root.widthBudget
    implicitHeight: column.implicitHeight + 24 * root.s

    Column {
        id: column
        x: 12 * root.s
        y: 12 * root.s
        width: root.width - 24 * root.s
        spacing: 10 * root.s
        Text {
            text: "ZFG · " + (root.service ? root.service.labels.focus || "" : "")
            color: Theme.bright
            font.family: Theme.display
            font.pixelSize: 16 * root.s
        }
        Text {
            text: root.service && root.service.focusFirst.length ? root.service.focusFirst : (root.service ? root.service.focusEmptyLabel : "…")
            color: Theme.bright
            font.family: Theme.font
            font.pixelSize: 13 * root.s
            width: column.width
            wrapMode: Text.WordWrap
            maximumLineCount: 3
            elide: Text.ElideRight
        }
        Text {
            text: root.service ? (root.service.labels.focus || "") + " " + root.service.focusCount + "/" + root.service.focusLimit + "   ·   " + (root.service.labels.later || "") + " " + root.service.laterCount : "…"
            color: root.service && root.service.focusFull ? Theme.vermDeep : Theme.dim
            font.family: Theme.mono
            font.pixelSize: 12 * root.s
        }
        Row {
            spacing: 8 * root.s
            Repeater {
                model: [{label:root.service ? root.service.labels.open_focus || "…" : "…", command:"focus"}, {label:root.service ? root.service.labels.open_later || "…" : "…", command:"later"}, {label:root.service ? root.service.labels.capture || "+" : "+", command:"launcher"}]
                delegate: Rectangle {
                    required property var modelData
                    width: buttonText.implicitWidth + 20 * root.s
                    height: buttonText.implicitHeight + 12 * root.s
                    radius: Theme.radius
                    color: mouse.pressed ? Theme.vermDeep : Theme.accent
                    Text { id: buttonText; anchors.centerIn: parent; text: modelData.label; color: Theme.cardBot; font.family: Theme.font; font.pixelSize: 11 * root.s }
                    MouseArea { id: mouse; anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: if (root.service) root.service.launch(modelData.command) }
                }
            }
        }
    }
}
