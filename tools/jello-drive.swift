// jello-drive: uses the real Jello window like a person would, so Claude can
// check the UI itself. macOS only.
//
// Build it once (the binary is not checked in):
//
//   mkdir -p tools/build
//   swiftc -O tools/jello-drive.swift -o tools/build/jello-drive
//
// Commands:
//
//   jello-drive info              where the window is
//   jello-drive front             bring Jello to the front
//   jello-drive back              bring the Claude app back to the front
//   jello-drive tree [depth]      list what is on the page (buttons, text)
//   jello-drive press <text>      press the first button whose name has <text>
//   jello-drive click <x> <y>     click at a point in the window
//   jello-drive key <name> [n]    press a key n times (left, right, up, down,
//                                 esc, return, space, tab)
//   jello-drive shot <file.png>   capture the window
//
// Clicks and keys are real macOS events, the same as a person's, so they go
// to the app in front: press, click and key refuse to run unless Jello is.
// Places are in points from the window's top-left corner; captures are at
// the screen's pixel scale (2x on a Retina screen).
//
// The app that runs it (Claude, or a terminal) needs Accessibility and
// Screen Recording in System Settings > Privacy & Security.
//
// Jello runs fullscreen in its own Space. macOS cannot capture a window on a
// Space that is not showing, so run `front` first and `back` when done.

import AppKit
import ApplicationServices
import CoreGraphics

struct JelloWindow {
  let id: Int
  let pid: pid_t
  let bounds: CGRect
}

func fail(_ message: String) -> Never {
  FileHandle.standardError.write((message + "\n").data(using: .utf8)!)
  exit(1)
}

// Finds Jello's main window among every window macOS knows about.
func findJello() -> JelloWindow? {
  let options: CGWindowListOption = [.optionAll, .excludeDesktopElements]
  let list = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
  for window in list {
    let owner = window[kCGWindowOwnerName as String] as? String ?? ""
    let name = window[kCGWindowName as String] as? String ?? ""
    let layer = window[kCGWindowLayer as String] as? Int ?? -1
    if owner.lowercased() != "jello" {
      continue
    }
    // Layer 0 holds ordinary windows; the menu bar and such sit above it.
    if layer != 0 {
      continue
    }
    if name != "Jello" {
      continue
    }
    let id = window[kCGWindowNumber as String] as? Int ?? 0
    let pid = window[kCGWindowOwnerPID as String] as? Int ?? 0
    let boundsDict = window[kCGWindowBounds as String] as! CFDictionary
    var bounds = CGRect.zero
    CGRectMakeWithDictionaryRepresentation(boundsDict, &bounds)
    return JelloWindow(id: id, pid: pid_t(pid), bounds: bounds)
  }
  return nil
}

// Accessibility: read the page the way a screen reader does.

func attribute(_ element: AXUIElement, _ name: String) -> AnyObject? {
  var value: AnyObject?
  let result = AXUIElementCopyAttributeValue(element, name as CFString, &value)
  if result != .success {
    return nil
  }
  return value
}

func text(_ element: AXUIElement, _ name: String) -> String {
  let value = attribute(element, name)
  if let string = value as? String {
    return string
  }
  if let number = value as? NSNumber {
    return number.stringValue
  }
  return ""
}

func children(_ element: AXUIElement) -> [AXUIElement] {
  let value = attribute(element, kAXChildrenAttribute)
  return value as? [AXUIElement] ?? []
}

// Where an element is on screen, if it has a place.
func frame(_ element: AXUIElement) -> CGRect? {
  guard let positionValue = attribute(element, kAXPositionAttribute) else {
    return nil
  }
  guard let sizeValue = attribute(element, kAXSizeAttribute) else {
    return nil
  }
  var position = CGPoint.zero
  var size = CGSize.zero
  AXValueGetValue(positionValue as! AXValue, .cgPoint, &position)
  AXValueGetValue(sizeValue as! AXValue, .cgSize, &size)
  return CGRect(origin: position, size: size)
}

// Everything a person could read on an element: its title, description and
// value, joined.
func label(_ element: AXUIElement) -> String {
  var parts: [String] = []
  for name in [kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute] {
    let part = text(element, name)
    if !part.isEmpty {
      parts.append(part)
    }
  }
  return parts.joined(separator: " | ")
}

// The window's own element. WebKit only builds the page's tree once the
// window has been shown, and can take a moment to answer, so wait up to 5
// seconds per question.
func jelloWindowElement(_ jello: JelloWindow) -> AXUIElement {
  let app = AXUIElementCreateApplication(jello.pid)
  AXUIElementSetMessagingTimeout(app, 5)
  let focused = attribute(app, kAXFocusedWindowAttribute)
  if focused == nil {
    fail("Jello has no focused window; run `jello-drive front` first")
  }
  return focused as! AXUIElement
}

func printTree(_ element: AXUIElement, depth: Int, maxDepth: Int, window: CGRect) {
  let role = text(element, kAXRoleAttribute)
  let words = label(element)
  var place = ""
  if let box = frame(element) {
    let x = Int(box.minX - window.minX)
    let y = Int(box.minY - window.minY)
    place = " @\(x),\(y) \(Int(box.width))x\(Int(box.height))"
  }
  let indent = String(repeating: "  ", count: depth)
  print("\(indent)\(role) \(words)\(place)")
  if depth >= maxDepth {
    return
  }
  for child in children(element) {
    printTree(child, depth: depth + 1, maxDepth: maxDepth, window: window)
  }
}

let pressableRoles = [kAXButtonRole, "AXRadioButton", "AXTab"]

// The first button, in page order, whose words contain `wanted`.
func findButton(_ element: AXUIElement, containing wanted: String) -> AXUIElement? {
  let role = text(element, kAXRoleAttribute)
  let words = label(element).lowercased()
  if pressableRoles.contains(role) {
    if words.contains(wanted.lowercased()) {
      return element
    }
  }
  for child in children(element) {
    if let found = findButton(child, containing: wanted) {
      return found
    }
  }
  return nil
}

// Real events.

func bringToFront(pid: pid_t) {
  let app = AXUIElementCreateApplication(pid)
  AXUIElementSetAttributeValue(app, kAXFrontmostAttribute as CFString, kCFBooleanTrue)
}

func bringClaudeBack() {
  let bundle = "com.anthropic.claudefordesktop"
  let apps = NSRunningApplication.runningApplications(withBundleIdentifier: bundle)
  if apps.isEmpty {
    fail("the Claude app is not running")
  }
  bringToFront(pid: apps[0].processIdentifier)
}

func frontAppName() -> String {
  let front = NSWorkspace.shared.frontmostApplication
  return front?.localizedName ?? "nothing"
}

func click(at point: CGPoint) {
  let source = CGEventSource(stateID: .hidSystemState)
  let types: [CGEventType] = [.leftMouseDown, .leftMouseUp]
  for type in types {
    let event = CGEvent(
      mouseEventSource: source,
      mouseType: type,
      mouseCursorPosition: point,
      mouseButton: .left
    )!
    event.post(tap: .cghidEventTap)
    usleep(50_000)
  }
}

// macOS key codes, the place of each key on the keyboard.
let keyCodes: [String: CGKeyCode] = [
  "left": 123,
  "right": 124,
  "down": 125,
  "up": 126,
  "esc": 53,
  "return": 36,
  "space": 49,
  "tab": 48,
]

func pressKey(_ code: CGKeyCode) {
  let source = CGEventSource(stateID: .hidSystemState)
  let down = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: true)!
  down.post(tap: .cghidEventTap)
  usleep(60_000)
  let up = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: false)!
  up.post(tap: .cghidEventTap)
  usleep(60_000)
}

// Events go to whatever app is in front, so make sure it is Jello.
func requireFront(_ jello: JelloWindow) {
  let front = NSWorkspace.shared.frontmostApplication?.processIdentifier
  if front != jello.pid {
    fail("Jello is not in front; run `jello-drive front` first")
  }
}

// Main.

if !AXIsProcessTrusted() {
  fail("no Accessibility permission for the app running this")
}
let arguments = Array(CommandLine.arguments.dropFirst())
if arguments.isEmpty {
  fail("usage: jello-drive info|front|back|tree|press|click|key|shot")
}
let command = arguments[0]

// `back` works even when Jello is not running.
if command == "back" {
  bringClaudeBack()
  usleep(800_000)
  print("front: \(frontAppName())")
  exit(0)
}

guard let jello = findJello() else {
  fail("no Jello window; start it with `npm run tauri dev`")
}

if command == "info" {
  let b = jello.bounds
  print("window \(jello.id) pid \(jello.pid) at \(Int(b.minX)),\(Int(b.minY)) size \(Int(b.width))x\(Int(b.height))")
} else if command == "front" {
  bringToFront(pid: jello.pid)
  // Switching to Jello's fullscreen Space takes a moment.
  usleep(1_500_000)
  print("front: \(frontAppName())")
} else if command == "tree" {
  var maxDepth = 40
  if arguments.count > 1 {
    maxDepth = Int(arguments[1]) ?? 40
  }
  printTree(jelloWindowElement(jello), depth: 0, maxDepth: maxDepth, window: jello.bounds)
} else if command == "press" {
  if arguments.count < 2 {
    fail("usage: jello-drive press <text>")
  }
  requireFront(jello)
  guard let button = findButton(jelloWindowElement(jello), containing: arguments[1]) else {
    fail("no button with \"\(arguments[1])\"")
  }
  guard let box = frame(button) else {
    fail("that button has no place on screen")
  }
  click(at: CGPoint(x: box.midX, y: box.midY))
  print("pressed \(label(button))")
} else if command == "click" {
  if arguments.count < 3 {
    fail("usage: jello-drive click <x> <y>")
  }
  requireFront(jello)
  let x = jello.bounds.minX + CGFloat(Double(arguments[1]) ?? 0)
  let y = jello.bounds.minY + CGFloat(Double(arguments[2]) ?? 0)
  click(at: CGPoint(x: x, y: y))
  print("clicked \(Int(x)),\(Int(y)) on screen")
} else if command == "key" {
  if arguments.count < 2 {
    fail("usage: jello-drive key <name> [count]")
  }
  guard let code = keyCodes[arguments[1]] else {
    fail("unknown key \(arguments[1])")
  }
  var count = 1
  if arguments.count > 2 {
    count = Int(arguments[2]) ?? 1
  }
  requireFront(jello)
  for _ in 0..<count {
    pressKey(code)
  }
  print("pressed \(arguments[1]) x\(count)")
} else if command == "shot" {
  if arguments.count < 2 {
    fail("usage: jello-drive shot <file.png>")
  }
  let capture = Process()
  capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
  // -x: no sound, -o: no window shadow, -l: just this window.
  capture.arguments = ["-x", "-o", "-l", String(jello.id), arguments[1]]
  try capture.run()
  capture.waitUntilExit()
  if capture.terminationStatus != 0 {
    fail("could not capture; is Jello in front?")
  }
  print("saved \(arguments[1])")
} else {
  fail("unknown command \(command)")
}
