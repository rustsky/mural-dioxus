// Builds a macOS app icon (rounded square with the standard 100px margin) from the iPhone artwork.
import AppKit

let args = CommandLine.arguments
let source = NSImage(contentsOfFile: args[1])!
let size = 1024.0, inset = 100.0, radius = 185.0
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size), pixelsHigh: Int(size), bitsPerSample: 8,
                           samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
let rect = NSRect(x: inset, y: inset, width: size - inset * 2, height: size - inset * 2)
let shadow = NSShadow()
shadow.shadowColor = NSColor.black.withAlphaComponent(0.22)
shadow.shadowOffset = NSSize(width: 0, height: -10)
shadow.shadowBlurRadius = 24
NSGraphicsContext.saveGraphicsState()
shadow.set()
NSColor.white.setFill()
NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).fill()
NSGraphicsContext.restoreGraphicsState()
NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).addClip()
source.draw(in: rect)
NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: args[2]))
