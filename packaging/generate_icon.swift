import CoreGraphics
import ImageIO
import Foundation

let output = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "AppIcon.png"
let size = 1024
let colorSpace = CGColorSpaceCreateDeviceRGB()
let context = CGContext(
    data: nil,
    width: size,
    height: size,
    bitsPerComponent: 8,
    bytesPerRow: size * 4,
    space: colorSpace,
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
)!

context.setFillColor(CGColor(red: 0.93, green: 0.96, blue: 1.0, alpha: 1.0))
context.fill(CGRect(x: 0, y: 0, width: size, height: size))

let body = CGRect(x: 170, y: 290, width: 684, height: 420)
context.setFillColor(CGColor(red: 0.16, green: 0.42, blue: 0.72, alpha: 1.0))
context.addPath(CGPath(roundedRect: body, cornerWidth: 86, cornerHeight: 86, transform: nil))
context.fillPath()

let top = CGRect(x: 275, y: 240, width: 220, height: 100)
context.setFillColor(CGColor(red: 0.16, green: 0.42, blue: 0.72, alpha: 1.0))
context.addPath(CGPath(roundedRect: top, cornerWidth: 28, cornerHeight: 28, transform: nil))
context.fillPath()

context.setFillColor(CGColor(red: 0.08, green: 0.25, blue: 0.45, alpha: 1.0))
context.fillEllipse(in: CGRect(x: 350, y: 350, width: 324, height: 324))
context.setFillColor(CGColor(red: 0.88, green: 0.95, blue: 1.0, alpha: 1.0))
context.fillEllipse(in: CGRect(x: 402, y: 402, width: 220, height: 220))
context.setFillColor(CGColor.white)
context.fillEllipse(in: CGRect(x: 442, y: 472, width: 70, height: 70))
context.setFillColor(CGColor(red: 0.88, green: 0.95, blue: 1.0, alpha: 1.0))
context.fillEllipse(in: CGRect(x: 754, y: 366, width: 46, height: 46))

let destinationURL = URL(fileURLWithPath: output) as CFURL
let destination = CGImageDestinationCreateWithURL(destinationURL, "public.png" as CFString, 1, nil)!
CGImageDestinationAddImage(destination, context.makeImage()!, nil)
guard CGImageDestinationFinalize(destination) else { exit(1) }
