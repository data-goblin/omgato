import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "io.github.data-goblin.omarchy-elgato"
  ipcTarget: "io.github.data-goblin.omarchy-elgato"

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  property var lights: []
  property var deck: ({ devices: [], pages: [], pedal: {}, services: {}, brightness: 0, default_page: "", auto_paginate: false, history: { can_undo: false, can_redo: false } })
  property var camera: ({ history: { can_undo: false, can_redo: false } })
  property var record: ({ active: false, seconds: 0, directory: "", options: { desktop_audio: false, mic: false } })
  property bool recDesktopAudio: false
  property bool recMic: false
  property bool recOptionsLoaded: false
  property int recSeconds: 0
  property var history: ({ can_undo: false, can_redo: false })
  property string renameIp: ""
  property string view: "lights"
  property int pageIndex: 0
  property var selection: []
  property int selectionAnchor: -1
  readonly property int editIndex: selection.length === 1 ? selection[0] : -1
  readonly property bool multiSelect: selection.length > 1
  property string device: "deck"
  property string pedalPosition: "left"
  property string lightsJson: ""
  property string deckJson: ""
  property string deckPagesJson: ""
  property var deckPages: []
  property bool interacting: false
  property int pendingDeckBrightness: -1
  property int deckBrightnessLocal: -1
  readonly property int deckBrightness: deckBrightnessLocal >= 0 ? deckBrightnessLocal : deck.brightness
  property double statusStartedAt: 0
  property string lastError: ""
  property var shortcuts: ({ installed: false, shortcuts: [] })
  property string capturingShortcut: ""
  property int dragFrom: -1
  property int dragTo: -1

  readonly property bool anyOn: lights.some(function(l) { return l.on })
  readonly property bool anyUnreachable: lights.some(function(l) { return !l.reachable })
  readonly property var page: deckPages.length ? deckPages[Math.min(pageIndex, deckPages.length - 1)] : null
  readonly property var deckDevice: deck.devices.find(function(d) { return !d.pedal })
    || ({ cols: 5, rows: 3, keys: 15, kind: "Deck", name: "No Stream Deck", encoders: 0 })
  readonly property var pedalDevice: deck.devices.find(function(d) { return d.pedal }) || null
  readonly property var sections: {
    var out = []
    if (settings.showLights !== false) out.push({ id: "lights", label: "Lights", glyph: "󰌵" })
    if (settings.showDeck !== false) out.push({ id: "deck", label: "StreamDeck", glyph: "󰌌" })
    if (settings.showCamera !== false) out.push({ id: "camera", label: "CamLink", glyph: "󰄀" })
    return out
  }

  onSectionsChanged: {
    if (!sections.length) return
    for (var i = 0; i < sections.length; i++) if (sections[i].id === view) return
    view = sections[0].id
  }

  readonly property string deckDaemonKey: device === "pedal" ? "streamdeck-ctl.service" : "streamdeck-ctl-deck.service"

  readonly property string contextLabel: view === "lights" ? "Lights"
    : view === "deck" ? (device === "pedal" ? "Pedal" : "Deck")
    : "Polling"
  readonly property bool contextChecked: view === "lights" ? anyOn
    : view === "deck" ? (deck.services[deckDaemonKey] === "active")
    : !camera.paused

  function kelvinColor(kelvin) {
    var t = Math.max(1000, Math.min(40000, kelvin)) / 100
    var r = t <= 66 ? 255 : 329.698727446 * Math.pow(t - 60, -0.1332047592)
    var g = t <= 66 ? 99.4708025861 * Math.log(t) - 161.1195681661
                    : 288.1221695283 * Math.pow(t - 60, -0.0755148492)
    var b = t >= 66 ? 255 : (t <= 19 ? 0 : 138.5177312231 * Math.log(t - 10) - 305.0447927307)
    function channel(v) { return Math.max(0, Math.min(255, v)) / 255 }
    return Qt.rgba(channel(r), channel(g), channel(b), 1)
  }

  function contextToggle() {
    if (view === "lights") { setLights("all", { on: !anyOn }); return }
    if (view === "deck") { act(["systemctl", "--user", deck.services[deckDaemonKey] === "active" ? "stop" : "start", deckDaemonKey]); return }
    act(["camctl", camera.paused ? "resume" : "pause"])
  }

  function refresh() {
    if (statusProc.running) {
      if (Date.now() - statusStartedAt > 8000) statusProc.signal(15)
      return
    }
    if (interacting || renameIp !== "") return
    statusProc.command = root.opened ? ["elgato-panel"] : ["elgato-panel", "--lights-only"]
    statusStartedAt = Date.now()
    statusProc.running = true
  }

  function act(cmd) {
    lastError = ""
    actionQueue.push(cmd)
    runNextAction()
  }

  property var actionQueue: []
  function runNextAction() {
    if (actionProc.running || !actionQueue.length) return
    actionProc.command = actionQueue.shift()
    actionProc.running = true
  }

  function setLights(target, patch) {
    root.lights = lights.map(function(l) {
      if (target !== "all" && l.name !== target) return l
      var next = {}
      for (var k in l) next[k] = l[k]
      for (var p in patch) next[p] = patch[p]
      return next
    })
    root.lightsJson = ""
    var cmd = ["elgatoctl", "set"]
    if (patch.on !== undefined) cmd.push(patch.on ? "--on" : "--off")
    if (patch.brightness !== undefined) cmd.push("--brightness", String(patch.brightness))
    if (patch.kelvin !== undefined) cmd.push("--temp", String(patch.kelvin))
    cmd.push(target)
    act(cmd)
  }

  function hexValid(text) {
    return /^#?[0-9a-fA-F]{6}$/.test((text || "").trim())
  }

  function hexColor(text) {
    var value = (text || "").trim()
    return value.charAt(0) === "#" ? value : "#" + value
  }

  function recClock() {
    var total = Math.max(0, recSeconds)
    var minutes = Math.floor(total / 60)
    var seconds = total % 60
    return (minutes < 10 ? "0" : "") + minutes + ":" + (seconds < 10 ? "0" : "") + seconds
  }

  function startRecording(target) {
    var cmd = ["elgato-panel", "record", "--target", target]
    if (recDesktopAudio) cmd.push("--desktop-audio")
    if (recMic) cmd.push("--mic")
    act(cmd)
  }

  function pedalBinding(position, gesture) {
    var table = deck.pedal ? deck.pedal[position] : null
    return table && table[gesture] ? table[gesture] : ""
  }

  function pedalBoundCount(position) {
    var gestures = ["tap", "long", "double"]
    var bound = 0
    for (var i = 0; i < gestures.length; i++) if (pedalBinding(position, gestures[i])) bound++
    return bound
  }

  function stepPage(delta) {
    var count = deckPages.length
    if (!count) return
    pageIndex = (pageIndex + delta + count) % count
    selection = []
  }

  function gotoPage(name) {
    for (var i = 0; i < deckPages.length; i++) {
      if (deckPages[i].name === name) { pageIndex = i; selection = []; return }
    }
  }

  function queueDeckBrightness(value) {
    deckBrightnessLocal = Math.round(value)
    pendingDeckBrightness = Math.round(value)
    if (!deckBrightnessTimer.running) deckBrightnessTimer.start()
  }

  function flushDeckBrightness(value) {
    deckBrightnessTimer.stop()
    deckBrightnessLocal = Math.round(value)
    pendingDeckBrightness = Math.round(value)
    sendDeckBrightness()
  }

  function sendDeckBrightness() {
    if (pendingDeckBrightness < 0) return
    act(["streamdeck-ctl", "deck", "brightness", String(pendingDeckBrightness)])
    pendingDeckBrightness = -1
  }

  function selectKey(index, modifiers) {
    if ((modifiers & Qt.ShiftModifier) && selectionAnchor >= 0) {
      var low = Math.min(selectionAnchor, index)
      var high = Math.max(selectionAnchor, index)
      var run = []
      for (var i = low; i <= high; i++) run.push(i)
      selection = run
      return
    }
    if (modifiers & Qt.ControlModifier) {
      var next = selection.slice()
      var at = next.indexOf(index)
      if (at >= 0) next.splice(at, 1)
      else next.push(index)
      next.sort(function(a, b) { return a - b })
      selection = next
      selectionAnchor = index
      return
    }
    selection = (selection.length === 1 && selection[0] === index) ? [] : [index]
    selectionAnchor = index
  }

  readonly property var viewShortcuts: (shortcuts.shortcuts || []).filter(function(s) { return s.view === root.view })

  function comboFromEvent(event) {
    if (event.key === Qt.Key_Escape) return ""
    var parts = []
    if (event.modifiers & Qt.MetaModifier) parts.push("SUPER")
    if (event.modifiers & Qt.ControlModifier) parts.push("CTRL")
    if (event.modifiers & Qt.AltModifier) parts.push("ALT")
    if (event.modifiers & Qt.ShiftModifier) parts.push("SHIFT")
    var name = keyName(event)
    if (!name) return ""
    parts.push(name)
    return parts.join(" + ")
  }

  function keyName(event) {
    var named = {}
    named[Qt.Key_Space] = "space"; named[Qt.Key_Return] = "Return"; named[Qt.Key_Enter] = "Return"
    named[Qt.Key_Tab] = "Tab"; named[Qt.Key_Backspace] = "BackSpace"; named[Qt.Key_Delete] = "Delete"
    named[Qt.Key_Left] = "Left"; named[Qt.Key_Right] = "Right"; named[Qt.Key_Up] = "Up"; named[Qt.Key_Down] = "Down"
    named[Qt.Key_BracketLeft] = "bracketleft"; named[Qt.Key_BracketRight] = "bracketright"
    named[Qt.Key_Semicolon] = "semicolon"; named[Qt.Key_Apostrophe] = "apostrophe"
    named[Qt.Key_Backslash] = "backslash"; named[Qt.Key_Comma] = "comma"; named[Qt.Key_Period] = "period"
    named[Qt.Key_Slash] = "slash"; named[Qt.Key_Minus] = "minus"; named[Qt.Key_Equal] = "equal"
    if (named[event.key]) return named[event.key]
    if (event.key >= Qt.Key_F1 && event.key <= Qt.Key_F12) return "F" + (event.key - Qt.Key_F1 + 1)
    if (event.key >= Qt.Key_A && event.key <= Qt.Key_Z) return String.fromCharCode(event.key)
    if (event.key >= Qt.Key_0 && event.key <= Qt.Key_9) return String.fromCharCode(event.key)
    return ""
  }

  function captureShortcut(event) {
    if (capturingShortcut === "") return false
    if (event.key === Qt.Key_Shift || event.key === Qt.Key_Control
        || event.key === Qt.Key_Alt || event.key === Qt.Key_Meta) return true
    var combo = comboFromEvent(event)
    if (combo !== "") act(["elgato-panel", "set-shortcut", "--id", capturingShortcut, "--keys", combo])
    capturingShortcut = ""
    return true
  }

  function selectionContains(index) {
    return selection.indexOf(index) >= 0
  }

  function applyToSelection(field, value) {
    if (!page) return
    for (var i = 0; i < selection.length; i++) {
      act(["streamdeck-ctl", "deck", "set", page.name, String(selection[i]), "--" + field, value])
    }
  }

  function editKey() {
    if (!page || editIndex < 0) return ({})
    for (var i = 0; i < page.keys.length; i++) if (page.keys[i].index === editIndex) return page.keys[i]
    return ({})
  }

  function cancelOrder() {
    dragFrom = -1
    dragTo = -1
    interacting = false
  }

  function commitOrder() {
    var from = dragFrom
    var to = dragTo
    cancelOrder()
    if (from < 0 || to < 0 || from === to) return
    var addresses = lights.map(function(l) { return l.ip })
    addresses.splice(to, 0, addresses.splice(from, 1)[0])
    lightsJson = ""
    act(["elgato-panel", "order", "--ips", addresses.join(",")])
  }

  function deckSet(pageName, index, field, value) {
    act(["streamdeck-ctl", "deck", "set", pageName, String(index), "--" + field, value])
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Process {
    id: statusProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var data = JSON.parse(text || "{}")
          var lightsText = JSON.stringify(data.lights || [])
          if (lightsText !== root.lightsJson) {
            root.lightsJson = lightsText
            root.lights = data.lights || []
          }
          if (data.deck) {
            var pagesText = JSON.stringify(data.deck.pages || [])
            if (pagesText !== root.deckPagesJson) {
              root.deckPagesJson = pagesText
              root.deckPages = data.deck.pages || []
            }
            data.deck.pages = []
            var deckText = JSON.stringify(data.deck)
            if (deckText !== root.deckJson) {
              root.deckJson = deckText
              root.deck = data.deck
            }
          }
          if (data.camera) root.camera = data.camera
          if (data.shortcuts) root.shortcuts = data.shortcuts
          if (data.record) {
            root.record = data.record
            root.recSeconds = data.record.seconds
            if (!root.recOptionsLoaded && data.record.options) {
              root.recDesktopAudio = !!data.record.options.desktop_audio
              root.recMic = !!data.record.options.mic
              root.recOptionsLoaded = true
            }
          }
          if (data.history) root.history = data.history
        } catch (e) {
        }
      }
    }
  }

  Process {
    id: actionProc
    stderr: StdioCollector {
      onStreamFinished: if (text.trim() !== "") root.lastError = text.trim().split("\n").pop()
    }
    onExited: function(code) {
      if (code !== 0 && root.lastError === "") root.lastError = "command failed (" + code + ")"
      if (root.actionQueue.length) root.runNextAction()
      else root.refresh()
    }
  }

  Timer {
    interval: root.opened ? 3000 : 15000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  Timer {
    id: deckBrightnessTimer
    interval: 140
    repeat: false
    onTriggered: root.sendDeckBrightness()
  }

  Timer {
    interval: 1000
    running: root.record.active && root.opened
    repeat: true
    onTriggered: root.recSeconds += 1
  }

  onDeckChanged: if (deckBrightnessLocal >= 0 && deck.brightness === deckBrightnessLocal) deckBrightnessLocal = -1
  onDeckBrightnessLocalChanged: if (deckBrightnessLocal >= 0) deckBrightnessHold.restart()

  Timer {
    id: deckBrightnessHold
    interval: 4000
    repeat: false
    onTriggered: root.deckBrightnessLocal = -1
  }

  onOpenedChanged: {
    if (!opened) {
      renameIp = ""
      selection = []
      interacting = false
      cancelOrder()
    }
    refresh()
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.anyUnreachable ? "󰂑" : (root.anyOn ? "󰌵" : "󰌶")
    dimmed: !root.anyOn
    slotSize: Style.bar.statusSlot
    fontSize: Style.font.caption
    tooltipText: ""
    onPressed: function(b) {
      if (b === Qt.RightButton) root.act(["elgatoctl", "click"])
      else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(380))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      Keys.onPressed: function(event) { if (root.captureShortcut(event)) event.accepted = true }
      onCloseRequested: if (root.capturingShortcut !== "") root.capturingShortcut = ""; else root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(12)

        PanelHero {
          width: parent.width
          title: "Elgato"
          meta: "Lights, camera, action!"
          foreground: root.foreground
          fontFamily: root.fontFamily
          iconComponent: Component {
            Rectangle {
              implicitWidth: Style.font.display * 1.2
              implicitHeight: implicitWidth
              radius: Style.space(7)
              color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.13)
              Text {
                anchors.centerIn: parent
                text: "E"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.title
                font.bold: true
              }
            }
          }
          trailingControl: Component {
            Row {
              spacing: Style.space(8)
              Text {
                text: root.contextLabel
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
                font.bold: true
                anchors.verticalCenter: parent.verticalCenter
              }
              ToggleSwitch {
                checked: root.contextChecked
                foreground: root.foreground
                anchors.verticalCenter: parent.verticalCenter
                onToggled: root.contextToggle()
              }
            }
          }
        }

        Row {
          id: selector
          width: parent.width
          visible: root.sections.length > 1
          spacing: Style.space(6)
          readonly property real cellWidth: (width - spacing * (root.sections.length - 1)) / Math.max(1, root.sections.length)
          Repeater {
            model: root.sections
            Button {
              required property var modelData
              width: selector.cellWidth
              iconText: modelData.glyph
              text: modelData.label
              fontSize: Style.font.bodySmall
              foreground: root.foreground
              fontFamily: root.fontFamily
              bordered: true
              active: root.view === modelData.id
              onClicked: root.view = modelData.id
            }
          }
        }

        PanelSeparator { foreground: root.foreground; visible: selector.visible }

        Item {
          width: parent.width
          visible: root.lastError !== ""
          implicitHeight: visible ? errorText.implicitHeight + Style.space(8) : 0

          Rectangle {
            anchors.fill: parent
            radius: Style.space(5)
            color: Qt.rgba(root.urgent.r, root.urgent.g, root.urgent.b, 0.12)
          }
          Text {
            id: errorText
            anchors.left: parent.left
            anchors.right: dismissError.left
            anchors.leftMargin: Style.space(8)
            anchors.rightMargin: Style.space(4)
            anchors.verticalCenter: parent.verticalCenter
            text: root.lastError
            color: root.urgent
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }
          WidgetButton {
            id: dismissError
            bar: root.bar
            text: "×"
            fontSize: Style.font.body
            foreground: root.urgent
            labelVisible: true
            horizontalMargin: Style.space(6)
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            onPressed: root.lastError = ""
          }
        }

        Loader {
          width: parent.width
          sourceComponent: root.view === "lights" ? lightsView : (root.view === "deck" ? deckView : cameraView)
        }
      }
    }
  }

  Component {
    id: lightsView
    Column {
      width: parent ? parent.width : 0
      spacing: Style.space(12)

      ActionRow {
        primaryIcon: "󰓡"
        primaryText: "Sync"
        canUndo: root.history.can_undo
        canRedo: root.history.can_redo
        onPrimary: root.act(["elgato-panel", "sync"])
        onUndo: root.act(["elgato-panel", "undo"])
        onRedo: root.act(["elgato-panel", "redo"])
      }

      Grid {
        id: lightGrid
        width: parent.width
        columns: Math.max(1, Math.min(2, root.lights.length))
        spacing: Style.space(14)
        readonly property real cellWidth: columns > 1 ? (width - spacing) / 2 : width

        Repeater {
          model: root.lights

          Item {
            id: lightCell
            required property var modelData
            required property int index
            readonly property int cellIndex: index
            readonly property bool renaming: root.renameIp === modelData.ip
            readonly property bool handlesVisible: cellHover.hovered || root.dragFrom >= 0
            readonly property bool dropTarget: root.dragFrom >= 0 && root.dragTo === index && root.dragFrom !== index

            width: lightGrid.cellWidth
            height: cellBody.implicitHeight
            opacity: root.dragFrom === index ? 0.45 : 1

            HoverHandler { id: cellHover }

            Rectangle {
              anchors.fill: parent
              anchors.margins: -Style.space(5)
              radius: Style.space(6)
              color: lightCell.dropTarget ? Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.10) : "transparent"
              border.width: lightCell.dropTarget ? 1 : 0
              border.color: root.foreground
            }

            Column {
              id: cellBody
              width: parent.width
              spacing: Style.space(8)

              Item {
                width: parent.width
                implicitHeight: Style.spacing.controlHeight

                Row {
                  visible: !lightCell.renaming
                  anchors.left: parent.left
                  anchors.right: lightPower.left
                  anchors.rightMargin: Style.spacing.sm
                  anchors.verticalCenter: parent.verticalCenter
                  spacing: Style.space(2)

                  Text {
                    text: lightCell.modelData.display
                    color: lightCell.modelData.reachable ? root.foreground : root.urgent
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.body
                    elide: Text.ElideRight
                    width: Math.min(implicitWidth, Math.max(0, parent.width - renameBtn.width - dragBtn.width - parent.spacing * 2))
                    anchors.verticalCenter: parent.verticalCenter
                  }

                  WidgetButton {
                    id: renameBtn
                    bar: root.bar
                    text: "󰏫"
                    fontSize: Style.font.caption
                    foreground: root.dim
                    labelVisible: true
                    horizontalMargin: 2
                    opacity: lightCell.handlesVisible ? 1 : 0
                    enabled: lightCell.handlesVisible
                    anchors.verticalCenter: parent.verticalCenter
                    onPressed: root.renameIp = lightCell.modelData.ip
                    Behavior on opacity { NumberAnimation { duration: 120 } }
                  }

                  Item {
                    id: dragBtn
                    implicitWidth: dragGlyph.implicitWidth + Style.space(6)
                    implicitHeight: dragGlyph.implicitHeight
                    visible: root.lights.length > 1
                    opacity: lightCell.handlesVisible ? 1 : 0
                    anchors.verticalCenter: parent.verticalCenter
                    Behavior on opacity { NumberAnimation { duration: 120 } }

                    Text {
                      id: dragGlyph
                      anchors.centerIn: parent
                      text: "󰇛"
                      color: root.dragFrom === lightCell.cellIndex ? root.foreground : root.dim
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.caption
                    }

                    MouseArea {
                      anchors.fill: parent
                      enabled: lightCell.handlesVisible
                      cursorShape: Qt.SizeAllCursor
                      onPressed: { root.interacting = true; root.dragFrom = lightCell.cellIndex; root.dragTo = lightCell.cellIndex }
                      onPositionChanged: function(mouse) {
                        var point = mapToItem(lightGrid, mouse.x, mouse.y)
                        var over = lightGrid.childAt(point.x, point.y)
                        if (over && over.cellIndex !== undefined) root.dragTo = over.cellIndex
                      }
                      onReleased: root.commitOrder()
                      onCanceled: root.cancelOrder()
                    }
                  }
                }

                EditField {
                  visible: lightCell.renaming
                  anchors.left: parent.left
                  anchors.right: lightPower.left
                  anchors.rightMargin: Style.spacing.sm
                  anchors.verticalCenter: parent.verticalCenter
                  text: lightCell.modelData.display
                  placeholderText: lightCell.modelData.name
                  font.pixelSize: Style.font.body
                  onVisibleChanged: if (visible) { forceActiveFocus(); selectAll() }
                onActiveFocusChanged: if (!activeFocus && root.renameIp === lightCell.modelData.ip) root.renameIp = ""
                  onCommitted: function(v) {
                    if (v !== lightCell.modelData.display) root.act(["elgato-panel", "rename", "--ip", lightCell.modelData.ip, "--name", v])
                    root.renameIp = ""
                  }
                }

                Button {
                  id: lightPower
                  anchors.right: parent.right
                  anchors.verticalCenter: parent.verticalCenter
                  iconText: lightCell.modelData.on ? "󰌵" : "󰌶"
                  iconSize: Style.font.title
                  foreground: lightCell.modelData.on ? root.foreground : root.dim
                  fontFamily: root.fontFamily
                  onClicked: root.setLights(lightCell.modelData.name, { on: !lightCell.modelData.on })
                }
              }

              SliderRow {
                title: "Brightness"
                valueText: lightCell.modelData.brightness + "%"
                minimum: 0; maximum: 100; step: 1
                value: lightCell.modelData.brightness
                enabled: lightCell.modelData.reachable
                onCommitted: function(v) { root.setLights(lightCell.modelData.name, { brightness: Math.round(v) }) }
              }

              SliderRow {
                title: "Temp"
                unit: " K"
                swatch: true
                valueText: lightCell.modelData.kelvin + " K"
                minimum: 2900; maximum: 7000; step: 50
                value: lightCell.modelData.kelvin
                enabled: lightCell.modelData.reachable
                onCommitted: function(v) { root.setLights(lightCell.modelData.name, { kelvin: Math.round(v / 50) * 50 }) }
              }

              Text {
                visible: !lightCell.modelData.reachable
                text: "unreachable"
                color: root.urgent
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
              }
            }
          }
        }
      }

      ShortcutList {}

      Button {
        width: parent.width
        text: "Rediscover lights"
        fontSize: Style.font.caption
        foreground: root.foreground
        fontFamily: root.fontFamily
        bordered: true
        onClicked: root.act(["elgatoctl", "discover"])
      }
    }
  }

  Component {
    id: deckView
    Column {
      width: parent ? parent.width : 0
      spacing: Style.space(10)

      ActionRow {
        primaryIcon: "󰑐"
        primaryText: "Reload"
        canUndo: root.deck.history ? root.deck.history.can_undo : false
        canRedo: root.deck.history ? root.deck.history.can_redo : false
        onPrimary: root.act(["streamdeck-ctl", "deck", "reload"])
        onUndo: root.act(["elgato-panel", "deck-undo"])
        onRedo: root.act(["elgato-panel", "deck-redo"])
      }

      Row {
        width: parent.width
        spacing: Style.space(6)
        Repeater {
          model: [ { id: "deck", label: root.deckDevice.name || root.deckDevice.kind },
                   { id: "pedal", label: root.pedalDevice ? "Pedal" : "Pedal (none)" } ]
          Button {
            required property var modelData
            width: (parent.width - parent.spacing) / 2
            text: modelData.label
            fontSize: Style.font.caption
            foreground: root.foreground
            fontFamily: root.fontFamily
            bordered: true
            active: root.device === modelData.id
            onClicked: root.device = modelData.id
          }
        }
      }

      Loader {
        width: parent.width
        sourceComponent: root.device === "pedal" ? pedalView : gridView
      }
    }
  }

  Component {
    id: gridView
    Column {
      width: parent ? parent.width : 0
      spacing: Style.space(10)

      Item {
        width: parent.width
        implicitHeight: Style.spacing.controlHeight
        Button {
          iconText: "󰅁"
          foreground: root.foreground
          fontFamily: root.fontFamily
          bordered: true
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          onClicked: root.stepPage(-1)
        }
        Text {
          text: root.page ? (root.page.name + (root.page.name === root.deck.default_page ? "  ·  default" : "")) : "no pages"
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          anchors.centerIn: parent
        }
        Button {
          iconText: "󰅂"
          foreground: root.foreground
          fontFamily: root.fontFamily
          bordered: true
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          onClicked: root.stepPage(1)
        }
      }

      Grid {
        id: keyGrid
        width: parent.width
        columns: root.deckDevice.cols
        spacing: Style.space(4)
        opacity: root.deck.display_off ? 0.16 : 0.25 + 0.75 * (root.deckBrightness / 100)
        Behavior on opacity { NumberAnimation { duration: 120 } }
        readonly property real cell: (width - spacing * (columns - 1)) / columns

        Repeater {
          model: root.page ? root.page.keys : []
          Item {
            id: keyCell
            required property var modelData
            width: keyGrid.cell
            height: keyGrid.cell

            Image {
              id: keyImage
              anchors.fill: parent
              source: keyCell.modelData.preview ? "file://" + keyCell.modelData.preview : ""
              visible: status === Image.Ready
              cache: false
              smooth: true
              fillMode: Image.PreserveAspectFit
            }

            Rectangle {
              anchors.fill: parent
              visible: !keyImage.visible
              radius: Style.space(4)
              color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.06)
              Text {
                anchors.centerIn: parent
                text: String(keyCell.modelData.index)
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
              }
            }

            Rectangle {
              anchors.fill: parent
              color: "transparent"
              radius: Style.space(5)
              border.width: root.selectionContains(keyCell.modelData.index) ? 2 : (keyArea.containsMouse ? 1 : 0)
              border.color: root.foreground
            }

            MouseArea {
              id: keyArea
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              acceptedButtons: Qt.LeftButton
              onClicked: function(mouse) {
                if (keyCell.modelData.kind === "page" && mouse.modifiers === Qt.NoModifier) {
                  root.gotoPage(keyCell.modelData.target)
                  return
                }
                root.selectKey(keyCell.modelData.index, mouse.modifiers)
              }
            }
          }
        }
      }

      Column {
        visible: root.selection.length > 0 && !!root.page
        width: parent.width
        spacing: Style.space(6)
        readonly property var key: root.editKey()

        PanelSectionHeader {
          text: (root.multiSelect ? root.selection.length + " KEYS" : "KEY " + root.editIndex)
                + " ON " + (root.page ? root.page.name.toUpperCase() : "")
          foreground: root.foreground
          fontFamily: root.fontFamily
        }

        FieldRow {
          label: "Label"
          placeholder: "Shown under the icon"
          value: parent.key.label || ""
          visible: !root.multiSelect
          onCommitted: function(v) { root.deckSet(root.page.name, root.editIndex, "label", v) }
        }
        FieldRow {
          label: "Glyph"
          placeholder: "Nerd Font character"
          value: parent.key.glyph || ""
          visible: !root.multiSelect
          onCommitted: function(v) { root.deckSet(root.page.name, root.editIndex, "glyph", v) }
        }
        FieldRow {
          label: "Action"
          placeholder: "exec:obs or key:KEY_F13"
          value: parent.key.action || ""
          visible: !root.multiSelect
          onCommitted: function(v) { root.deckSet(root.page.name, root.editIndex, "action", v) }
        }
        FieldRow {
          label: "Icon"
          placeholder: "Path to a PNG"
          value: root.multiSelect ? "" : (parent.key.icon || "")
          onCommitted: function(v) { root.applyToSelection("icon", v) }
        }
        FieldRow {
          label: "Background"
          placeholder: "#rrggbb"
          colorPreview: true
          value: parent.key.bg || ""
          onCommitted: function(v) { root.applyToSelection("bg", v) }
        }
        FieldRow {
          label: "Foreground"
          placeholder: "#rrggbb"
          colorPreview: true
          value: parent.key.fg || ""
          onCommitted: function(v) { root.applyToSelection("fg", v) }
        }
        Row {
          width: parent.width
          spacing: Style.space(6)
          Button {
            width: (parent.width - parent.spacing) / 2
            text: root.multiSelect ? "Clear keys" : "Clear key"
            fontSize: Style.font.caption
            foreground: root.foreground
            fontFamily: root.fontFamily
            bordered: true
            onClicked: {
              for (var i = 0; i < root.selection.length; i++) {
                root.act(["streamdeck-ctl", "deck", "unset", root.page.name, String(root.selection[i])])
              }
              root.selection = []
            }
          }
          Button {
            width: (parent.width - parent.spacing) / 2
            text: "Done"
            fontSize: Style.font.caption
            foreground: root.foreground
            fontFamily: root.fontFamily
            bordered: true
            onClicked: root.selection = []
          }
        }
      }

      Item {
        width: parent.width
        implicitHeight: brightnessHolder.implicitHeight

        Item {
          id: brightnessHolder
          anchors.left: parent.left
          anchors.right: deckPower.left
          anchors.rightMargin: Style.space(8)
          implicitHeight: deckBrightnessRow.implicitHeight
          height: implicitHeight

          SliderRow {
            id: deckBrightnessRow
            title: "Deck brightness"
            valueText: root.deckBrightness + "%"
            minimum: 0; maximum: 100; step: 1
            value: root.deckBrightness
            enabled: !root.deck.display_off
            onMoved: function(v) { root.queueDeckBrightness(v) }
            onCommitted: function(v) { root.flushDeckBrightness(v) }
          }
        }

        Button {
          id: deckPower
          anchors.right: parent.right
          anchors.top: parent.top
          anchors.bottom: parent.bottom
          iconText: root.deck.display_off ? "󰤂" : "󰐥"
          iconSize: Style.font.title
          foreground: root.deck.display_off ? root.dim : root.foreground
          fontFamily: root.fontFamily
          bordered: true
          active: !root.deck.display_off
          tooltipText: root.deck.display_off ? "Switch the key display on" : "Switch the key display off"
          onClicked: root.act(["streamdeck-ctl", "deck", "power", root.deck.display_off ? "on" : "off"])
        }
      }

      ShortcutList {}

      Button {
        width: parent.width
        text: root.page && root.page.name === root.deck.default_page ? "Default page" : "Make default"
        fontSize: Style.font.caption
        foreground: root.foreground
        fontFamily: root.fontFamily
        bordered: true
        active: root.page && root.page.name === root.deck.default_page
        onClicked: if (root.page) root.act(["streamdeck-ctl", "deck", "default", root.page.name])
      }
    }
  }

  Component {
    id: pedalView
    Column {
      width: parent ? parent.width : 0
      spacing: Style.space(12)

      Item {
        width: parent.width
        height: Style.space(96)

        Rectangle {
          id: chassis
          anchors.fill: parent
          anchors.topMargin: Style.space(10)
          radius: Style.space(10)
          color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.07)
          border.width: 1
          border.color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.16)
        }

        Row {
          anchors.fill: parent
          anchors.margins: Style.space(10)
          spacing: Style.space(8)

          Repeater {
            model: [
              { id: "left", label: "Left", wide: false },
              { id: "center", label: "Centre", wide: true },
              { id: "right", label: "Right", wide: false }
            ]
            Rectangle {
              id: pedalShape
              required property var modelData
              readonly property bool selected: root.pedalPosition === modelData.id
              readonly property int bound: root.pedalBoundCount(modelData.id)

              width: modelData.wide ? (parent.width - parent.spacing * 2) * 0.42 : (parent.width - parent.spacing * 2) * 0.29
              height: modelData.wide ? parent.height - Style.space(12) : parent.height
              anchors.bottom: parent.bottom
              radius: Style.space(8)
              color: selected ? Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.20)
                              : Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.11)
              border.width: selected ? 2 : 1
              border.color: selected ? root.foreground : Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.22)

              Column {
                anchors.centerIn: parent
                spacing: Style.space(3)
                Text {
                  anchors.horizontalCenter: parent.horizontalCenter
                  text: pedalShape.modelData.wide ? "E" : ""
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: pedalShape.modelData.wide ? Style.font.title : Style.font.body
                  font.bold: pedalShape.modelData.wide
                }
                Text {
                  anchors.horizontalCenter: parent.horizontalCenter
                  text: pedalShape.modelData.label
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                }
                Text {
                  anchors.horizontalCenter: parent.horizontalCenter
                  text: pedalShape.bound + " of 3"
                  color: pedalShape.bound ? root.dim : Qt.rgba(root.dim.r, root.dim.g, root.dim.b, 0.6)
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                }
              }

              MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: root.pedalPosition = pedalShape.modelData.id
              }
            }
          }
        }
      }

      PanelSectionHeader {
        text: root.pedalPosition.toUpperCase() + " PEDAL"
        foreground: root.foreground
        fontFamily: root.fontFamily
      }

      Repeater {
        model: [
          { id: "tap", label: "Tap", glyph: "󰝁" },
          { id: "long", label: "Hold", glyph: "󰵷" },
          { id: "double", label: "Double", glyph: "󰜼" }
        ]
        Item {
          id: gestureRow
          required property var modelData
          width: parent.width
          implicitHeight: Style.spacing.controlHeight

          Text {
            id: gestureIcon
            text: gestureRow.modelData.glyph
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
          }
          Text {
            id: gestureLabel
            text: gestureRow.modelData.label
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            width: Style.space(48)
            anchors.left: gestureIcon.right
            anchors.leftMargin: Style.space(6)
            anchors.verticalCenter: parent.verticalCenter
          }
          EditField {
            anchors.left: gestureLabel.right
            anchors.leftMargin: Style.space(4)
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: root.pedalBinding(root.pedalPosition, gestureRow.modelData.id)
            placeholderText: "Not set"
            onCommitted: function(v) {
              root.act(["streamdeck-ctl", "pedal", "set", root.pedalPosition, gestureRow.modelData.id, v.trim() || "noop"])
              root.act(["streamdeck-ctl", "pedal", "reload"])
            }
          }
        }
      }
    }
  }

  Component {
    id: cameraView
    Column {
      width: parent ? parent.width : 0
      spacing: Style.space(10)

      ActionRow {
        primaryIcon: "󰜉"
        primaryText: "Reset"
        canUndo: root.camera.history ? root.camera.history.can_undo : false
        canRedo: root.camera.history ? root.camera.history.can_redo : false
        onPrimary: root.act(["camctl", "reset"])
        onUndo: root.act(["elgato-panel", "cam-undo"])
        onRedo: root.act(["elgato-panel", "cam-redo"])
      }

      PanelSectionHeader { text: "RECORD"; foreground: root.foreground; fontFamily: root.fontFamily }

      Item {
        width: parent.width
        implicitHeight: recordIdle.visible ? recordIdle.implicitHeight : stopButton.implicitHeight

        Row {
          id: recordIdle
          visible: !root.record.active
          width: parent.width
          spacing: Style.space(6)
          readonly property real cellWidth: (width - spacing) / 2

          Button {
            width: recordIdle.cellWidth
            iconText: "󰆟"
            text: "Pick area"
            fontSize: Style.font.caption
            foreground: root.foreground
            fontFamily: root.fontFamily
            bordered: true
            onClicked: root.startRecording("pick")
          }
          Button {
            width: recordIdle.cellWidth
            iconText: "󰍹"
            text: "Full screen"
            fontSize: Style.font.caption
            foreground: root.foreground
            fontFamily: root.fontFamily
            bordered: true
            onClicked: root.startRecording("screen")
          }
        }

        Button {
          id: stopButton
          visible: root.record.active
          width: parent.width
          iconText: "󰓛"
          text: "Stop recording   " + root.recClock()
          fontSize: Style.font.caption
          foreground: root.urgent
          accent: root.urgent
          fontFamily: root.fontFamily
          bordered: true
          active: true
          onClicked: root.act(["elgato-panel", "record", "--stop"])
        }
      }

      Item {
        width: parent.width
        visible: !root.record.active && root.record.scope !== ""
        implicitHeight: Style.spacing.controlHeight

        Text {
          text: "Area"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
        }
        Text {
          text: root.record.scope
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
          anchors.left: parent.left
          anchors.leftMargin: Style.space(46)
          anchors.right: scopeSteps.left
          anchors.rightMargin: Style.space(6)
          anchors.verticalCenter: parent.verticalCenter
        }
        Row {
          id: scopeSteps
          spacing: Style.space(4)
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          WidgetButton {
            bar: root.bar
            text: "◀"
            fontSize: Style.font.caption
            foreground: root.record.history && root.record.history.can_undo ? root.foreground : root.dim
            labelVisible: true
            horizontalMargin: Style.space(4)
            onPressed: if (root.record.history && root.record.history.can_undo) root.act(["elgato-panel", "scope-undo"])
          }
          WidgetButton {
            bar: root.bar
            text: "▶"
            fontSize: Style.font.caption
            foreground: root.record.history && root.record.history.can_redo ? root.foreground : root.dim
            labelVisible: true
            horizontalMargin: Style.space(4)
            onPressed: if (root.record.history && root.record.history.can_redo) root.act(["elgato-panel", "scope-redo"])
          }
        }
      }

      Button {
        width: parent.width
        visible: !root.record.active && root.record.scope !== ""
        iconText: "󰑊"
        text: "Record " + root.record.scope + " again"
        fontSize: Style.font.caption
        foreground: root.foreground
        fontFamily: root.fontFamily
        bordered: true
        onClicked: root.startRecording("last")
      }

      Row {
        id: audioRow
        visible: !root.record.active
        width: parent.width
        spacing: Style.space(6)
        readonly property real cellWidth: (width - spacing) / 2

        Button {
          width: audioRow.cellWidth
          iconText: "󰕾"
          text: "Desktop audio"
          fontSize: Style.font.caption
          foreground: root.foreground
          fontFamily: root.fontFamily
          bordered: true
          active: root.recDesktopAudio
          onClicked: root.recDesktopAudio = !root.recDesktopAudio
        }
        Button {
          width: audioRow.cellWidth
          iconText: root.recMic ? "󰍬" : "󰍭"
          text: "Microphone"
          fontSize: Style.font.caption
          foreground: root.foreground
          fontFamily: root.fontFamily
          bordered: true
          active: root.recMic
          onClicked: root.recMic = !root.recMic
        }
      }

      PanelSectionHeader { text: "OVERLAY"; foreground: root.foreground; fontFamily: root.fontFamily }

      Row {
        width: parent.width
        spacing: Style.space(6)
        Button {
          width: (parent.width - parent.spacing) / 2
          iconText: root.camera.overlay ? "󰈉" : "󰈈"
          text: root.camera.overlay ? "Hide" : "Show"
          fontSize: Style.font.bodySmall
          foreground: root.foreground
          fontFamily: root.fontFamily
          bordered: true
          active: !!root.camera.overlay
          onClicked: root.act(["camctl", "toggle"])
        }
        Button {
          width: (parent.width - parent.spacing) / 2
          iconText: "󰊓"
          text: "Fullscreen"
          fontSize: Style.font.bodySmall
          foreground: root.foreground
          fontFamily: root.fontFamily
          bordered: true
          onClicked: root.act(["camctl", "full"])
        }
      }

      Row {
        width: parent.width
        spacing: Style.space(6)
        Repeater {
          model: [ { id: "tl", label: "◰" }, { id: "tr", label: "◳" },
                   { id: "bl", label: "◱" }, { id: "br", label: "◲" } ]
          Button {
            required property var modelData
            width: (parent.width - parent.spacing * 3) / 4
            text: modelData.label
            foreground: root.foreground
            fontFamily: root.fontFamily
            bordered: true
            active: root.camera.corner === modelData.id
            onClicked: root.act(["camctl", "move", modelData.id])
          }
        }
      }

      Button {
        width: parent.width
        iconText: "󰩭"
        text: "PiP position"
        tooltipText: "Drag out exactly where the camera overlay sits"
        fontSize: Style.font.caption
        foreground: root.foreground
        fontFamily: root.fontFamily
        bordered: true
        active: root.camera.corner === "area"
        onClicked: root.act(["camctl", "pick"])
      }

      InfoPair { label: "Cam Link 4K"; value: String(root.camera.tooltip || root.camera.state || "unknown") }

      ShortcutList {}
    }
  }

  component ShortcutList: Column {
    width: parent ? parent.width : 0
    spacing: Style.space(4)
    visible: root.viewShortcuts.length > 0

    PanelSeparator { foreground: root.foreground }

    Item {
      width: parent.width
      implicitHeight: Style.spacing.controlHeight
      Text {
        text: "SHORTCUTS"
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        font.letterSpacing: 1.2
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
      }
      Button {
        visible: !root.shortcuts.installed
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        iconText: "󰐕"
        text: "Install"
        fontSize: Style.font.caption
        foreground: root.foreground
        fontFamily: root.fontFamily
        bordered: true
        tooltipText: "Write these shortcuts and source them from your hypr config"
        onClicked: root.act(["elgato-panel", "install-shortcuts"])
      }

      WidgetButton {
        visible: root.shortcuts.installed
        bar: root.bar
        text: "󰩺"
        fontSize: Style.font.caption
        foreground: root.dim
        labelVisible: true
        horizontalMargin: Style.space(4)
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        tooltipText: "Remove these shortcuts from your hypr config"
        onPressed: root.act(["elgato-panel", "uninstall-shortcuts"])
      }
    }

    Repeater {
      model: root.viewShortcuts
      Item {
        id: shortcutRow
        required property var modelData
        readonly property bool capturing: root.capturingShortcut === modelData.id
        width: parent.width
        implicitHeight: Style.spacing.controlHeight

        HoverHandler { id: shortcutHover }

        Text {
          id: shortcutLabel
          text: shortcutRow.modelData.label
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
          width: Math.max(0, parent.width - shortcutKeys.width - editShortcut.width - Style.space(16))
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
        }

        Text {
          id: shortcutKeys
          text: shortcutRow.capturing ? "press a combination"
              : (shortcutRow.modelData.display || "not set")
          color: shortcutRow.capturing ? root.foreground
               : shortcutRow.modelData.conflict ? root.urgent
               : shortcutRow.modelData.display ? root.foreground : root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          anchors.right: editShortcut.left
          anchors.rightMargin: Style.space(6)
          anchors.verticalCenter: parent.verticalCenter
        }

        WidgetButton {
          id: editShortcut
          bar: root.bar
          text: "󰏫"
          fontSize: Style.font.caption
          foreground: root.dim
          labelVisible: true
          horizontalMargin: 2
          opacity: shortcutHover.hovered || shortcutRow.capturing ? 1 : 0
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          onPressed: root.capturingShortcut = shortcutRow.capturing ? "" : shortcutRow.modelData.id
          Behavior on opacity { NumberAnimation { duration: 120 } }
        }

        ToolTip {
          visible: shortcutHover.hovered && shortcutRow.modelData.conflict !== ""
          text: "Already used by: " + shortcutRow.modelData.conflict
          delay: 300
        }
      }
    }
  }

  component ActionRow: Row {
    id: actionRow
    property string primaryIcon: ""
    property string primaryText: ""
    property bool canUndo: false
    property bool canRedo: false
    signal primary()
    signal undo()
    signal redo()

    width: parent ? parent.width : 0
    spacing: Style.space(6)
    readonly property real cellWidth: (width - spacing * 2) / 3

    Button {
      width: actionRow.cellWidth
      iconText: actionRow.primaryIcon
      text: actionRow.primaryText
      fontSize: Style.font.caption
      foreground: root.foreground
      fontFamily: root.fontFamily
      bordered: true
      onClicked: actionRow.primary()
    }
    Button {
      width: actionRow.cellWidth
      iconText: "󰕌"
      text: "Undo"
      fontSize: Style.font.caption
      foreground: root.foreground
      fontFamily: root.fontFamily
      bordered: true
      opacity: actionRow.canUndo ? 1 : 0.35
      onClicked: if (actionRow.canUndo) actionRow.undo()
    }
    Button {
      width: actionRow.cellWidth
      iconText: "󰑎"
      text: "Redo"
      fontSize: Style.font.caption
      foreground: root.foreground
      fontFamily: root.fontFamily
      bordered: true
      opacity: actionRow.canRedo ? 1 : 0.35
      onClicked: if (actionRow.canRedo) actionRow.redo()
    }
  }

  component SliderRow: Column {
    id: sliderRow
    property string unit: "%"
    property bool swatch: false
    property bool enabled: true
    opacity: enabled ? 1 : 0.4
    property string title: ""
    property string valueText: ""
    property real minimum: 0
    property real maximum: 1
    property real step: 1
    property real value: 0
    signal committed(real value)
    signal moved(real value)
    width: parent ? parent.width : 0
    spacing: Style.space(4)

    Item {
      width: parent.width
      implicitHeight: Math.max(sliderTitle.implicitHeight, sliderValue.implicitHeight)
      Text { id: sliderTitle; text: sliderRow.title; color: root.foreground; font.family: root.fontFamily; font.pixelSize: Style.font.body; anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter }
      Row {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.space(6)
        Rectangle {
          visible: sliderRow.swatch && sliderRow.enabled
          width: Style.font.caption
          height: width
          radius: width / 2
          color: root.kelvinColor(slider.dragging ? slider.liveValue : sliderRow.value)
          anchors.verticalCenter: parent.verticalCenter
        }
        Text { id: sliderValue; text: slider.dragging ? Math.round(slider.liveValue) + sliderRow.unit : sliderRow.valueText; color: root.foreground; font.family: root.fontFamily; font.pixelSize: Style.font.caption; anchors.verticalCenter: parent.verticalCenter }
      }
    }
    PanelSlider {
      id: slider
      width: parent.width
      bar: root.bar
      minimum: sliderRow.minimum
      maximum: sliderRow.maximum
      step: sliderRow.step
      integer: true
      value: sliderRow.value
      onMoved: function(v) { sliderRow.moved(v) }
      onReleased: function(v) { sliderRow.committed(v) }

      Binding {
        target: root
        property: "interacting"
        value: true
        when: slider.dragging
      }
    }
  }

  component FieldRow: Item {
    id: fieldRow
    property string label: ""
    property string placeholder: ""
    property string value: ""
    property bool colorPreview: false
    signal committed(string value)

    width: parent ? parent.width : 0
    implicitHeight: Style.spacing.controlHeight
    readonly property bool previewVisible: colorPreview && root.hexValid(field.text)

    Text {
      id: fieldLabel
      text: fieldRow.label
      color: root.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      width: Style.space(74)
      elide: Text.ElideRight
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
    }

    Rectangle {
      id: fieldSwatch
      visible: fieldRow.previewVisible
      width: Style.font.caption
      height: width
      radius: width / 2
      color: fieldRow.previewVisible ? root.hexColor(field.text) : "transparent"
      border.width: 1
      border.color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.35)
      anchors.left: fieldLabel.right
      anchors.leftMargin: Style.space(4)
      anchors.verticalCenter: parent.verticalCenter
    }

    EditField {
      id: field
      readonly property real fieldLeft: (fieldRow.previewVisible ? fieldSwatch.x + fieldSwatch.width : fieldLabel.width) + Style.space(6)
      x: fieldLeft
      width: Math.max(0, fieldRow.width - fieldLeft)
      anchors.verticalCenter: parent.verticalCenter
      placeholderText: fieldRow.placeholder
      onCommitted: function(v) { fieldRow.committed(v) }

      Component.onCompleted: text = fieldRow.value
      Connections {
        target: fieldRow
        function onValueChanged() {
          if (!field.activeFocus) field.text = fieldRow.value
        }
      }
    }
  }

  component EditField: TextField {
    signal committed(string value)
    width: parent ? parent.width : 0
    foreground: root.foreground
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    onAccepted: committed(text)
  }

  component InfoPair: Item {
    property string label: ""
    property string value: ""
    width: parent ? parent.width : 0
    implicitHeight: Math.max(pairLabel.implicitHeight, pairValue.implicitHeight)
    Text { id: pairLabel; text: label; color: root.dim; font.family: root.fontFamily; font.pixelSize: Style.font.bodySmall; anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter }
    Text { id: pairValue; text: value; color: root.foreground; font.family: root.fontFamily; font.pixelSize: Style.font.bodySmall; elide: Text.ElideLeft; anchors.right: parent.right; anchors.left: pairLabel.right; anchors.leftMargin: Style.spacing.sm; horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter }
  }
}
