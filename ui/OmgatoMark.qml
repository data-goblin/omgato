import QtQuick
import QtQuick.Shapes

Item {
  id: root

  implicitWidth: 24
  implicitHeight: 24
  property color color: "#f7f7fa"
  readonly property real markSize: Math.min(width, height)
  readonly property real unit: markSize / 24

  Shape {
    anchors.centerIn: parent
    width: root.markSize
    height: root.markSize
    preferredRendererType: Shape.CurveRenderer
    layer.enabled: true
    layer.samples: 4

    ShapePath {
      fillColor: root.color
      strokeColor: "transparent"
      fillRule: ShapePath.OddEvenFill

      PathSvg {
        path: {
          var u = root.unit
          function p(n) { return (n * u).toFixed(3) }
          return "M " + p(6) + " " + p(0) +
                 " H " + p(18) + " V " + p(3) + " H " + p(21) + " V " + p(6) + " H " + p(24) +
                 " V " + p(18) + " H " + p(21) + " V " + p(21) + " H " + p(18) + " V " + p(24) +
                 " H " + p(6)  + " V " + p(21) + " H " + p(3)  + " V " + p(18) + " H " + p(0) +
                 " V " + p(6)  + " H " + p(3)  + " V " + p(3)  + " H " + p(6)  + " Z" +
                 " M " + p(5.5) + " " + p(5.5) +
                 " H " + p(18.5) + " V " + p(18.5) + " H " + p(5.5) + " Z"
        }
      }
    }

    ShapePath {
      fillColor: root.color
      strokeColor: "transparent"

      PathSvg {
        path: {
          var u = root.unit
          function p(n) { return (n * u).toFixed(3) }
          return "M " + p(8.8) + " " + p(7.4) +
                 " L " + p(17.0) + " " + p(12) +
                 " L " + p(8.8) + " " + p(16.6) + " Z"
        }
      }
    }
  }
}
