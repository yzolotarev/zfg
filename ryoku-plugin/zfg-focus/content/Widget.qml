import QtQuick
import Ryoku.PluginKit
import Ryoku.PluginKit.Singletons

Item {
    id: root
    property var pluginApi
    property var screen
    property bool active: false
    property string density: "glyph"
    property real s: 1
    property real widthBudget: 0
    readonly property var service: pluginApi ? pluginApi.mainInstance : null

    implicitWidth: row.implicitWidth
    implicitHeight: Math.max(row.implicitHeight, 20 * root.s)

    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        spacing: 6 * root.s
        Item {
            anchors.verticalCenter: parent.verticalCenter
            width: 16 * root.s
            height: 16 * root.s
            Rectangle {
                anchors.centerIn: parent
                width: 13 * root.s
                height: width
                radius: width / 2
                color: "transparent"
                border.width: 1.5 * root.s
                border.color: root.service && root.service.focusFull ? Theme.vermLit : Theme.accent
            }
            Rectangle {
                anchors.centerIn: parent
                width: 4 * root.s
                height: width
                radius: width / 2
                color: root.service && root.service.focusFull ? Theme.vermLit : Theme.accent
            }
        }
        Text {
            id: focusText
            anchors.verticalCenter: parent.verticalCenter
            visible: true
            text: root.service && root.service.focusFirst.length ? root.service.focusFirst : (root.service ? root.service.focusEmptyLabel : "…")
            color: Theme.bright
            font.family: Theme.font
            font.pixelSize: 12 * root.s
            elide: Text.ElideRight
            width: Math.min(implicitWidth, root.widthBudget > 0 ? root.widthBudget * 0.55 : Math.max(40, root.service ? root.service.truncateAt * 7 : 196) * root.s)
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: !root.service || root.service.showFocusCounter
            text: root.service ? root.service.focusCount + "/" + root.service.focusLimit : "0/5"
            color: root.service && root.service.focusFull ? Theme.vermLit : Theme.dim
            font.family: Theme.mono
            font.pixelSize: 11 * root.s
        }
        Text {
            id: laterText
            anchors.verticalCenter: parent.verticalCenter
            visible: root.service && root.service.showLaterCounter && root.service.laterCount > 0
            text: root.service ? "≡ " + root.service.laterCount : ""
            color: Theme.dim
            font.family: Theme.mono
            font.pixelSize: 10 * root.s
        }
        Text {
            id: captureText
            anchors.verticalCenter: parent.verticalCenter
            text: "+"
            color: Theme.bright
            font.pixelSize: 17 * root.s
        }
    }
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: mouse => {
            if (!root.service) return;
            if (mouse.x >= captureText.x - 4 * root.s) root.service.launch("launcher");
            else if (laterText.visible && mouse.x >= laterText.x - 3 * root.s) root.service.launch("later");
            else root.service.launch("focus");
        }
    }
}
