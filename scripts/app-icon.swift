// Bereitet ein Logo als macOS-App-Icon auf (Anleitung: docs/app-icon.md).
//
// Aufruf: swift scripts/app-icon.swift <logo.png> [ausgabe.png]
//
// Schneidet die sichtbare Form aus dem Logo aus, skaliert sie auf Apples
// Icon-Raster (824 × 824 px mittig auf 1024 px), maskiert sie mit Apples
// Eckenform und macht die Fläche voll deckend. Sonst verkleinert macOS 26
// das Icon und setzt es auf eine graue Fläche.

import AppKit
import SwiftUI

let args = CommandLine.arguments
guard args.count >= 2 else {
    print("Aufruf: swift scripts/app-icon.swift <logo.png> [ausgabe.png]")
    exit(1)
}
let output = args.count >= 3 ? args[2] : "app-icon.png"
guard let src = NSImage(contentsOfFile: args[1])?.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
    print("Bild nicht lesbar: \(args[1])")
    exit(1)
}

func context(_ w: Int, _ h: Int) -> CGContext {
    CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
              space: CGColorSpace(name: CGColorSpace.sRGB)!,
              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
}

// Sichtbare Form finden: Bounding Box aller Pixel mit Deckkraft über 50 %
let probe = context(src.width, src.height)
probe.draw(src, in: CGRect(x: 0, y: 0, width: src.width, height: src.height))
let px = probe.data!.assumingMemoryBound(to: UInt8.self)
var (minX, minY, maxX, maxY) = (src.width, src.height, -1, -1)
for y in 0..<src.height {
    for x in 0..<src.width where px[(y * src.width + x) * 4 + 3] > 128 {
        minX = min(minX, x); maxX = max(maxX, x)
        minY = min(minY, y); maxY = max(maxY, y)
    }
}
guard maxX >= 0 else {
    print("Das Bild ist vollständig transparent.")
    exit(1)
}
let box = CGRect(x: minX, y: minY, width: maxX - minX + 1, height: maxY - minY + 1)
let shape = src.cropping(to: box)!
print("Form gefunden: \(Int(box.width)) × \(Int(box.height)) px bei (\(minX), \(minY))")

let stretch = box.width / box.height
if abs(stretch - 1) > 0.08 {
    print("Warnung: Form ist nicht annähernd quadratisch (Verhältnis \(String(format: "%.2f", stretch))), das Motiv wird sichtbar verzerrt.")
}
if box.width < 824 || box.height < 824 {
    print("Warnung: Form ist kleiner als 824 px und wird hochskaliert – besser ein größeres Original verwenden.")
}

// Apple-Raster: 1024er Leinwand, Form 824×824 bei Versatz 100, kontinuierliche Ecken r=185.4
let size = 1024, inset: CGFloat = 100, side: CGFloat = 824
// etwas größer zeichnen, damit Originalrand und -ecken sicher außerhalb der Maske liegen
let overscan: CGFloat = 18

let art = context(size, size)
art.interpolationQuality = .high
art.draw(shape, in: CGRect(x: inset - overscan, y: inset - overscan,
                           width: side + 2 * overscan, height: side + 2 * overscan))

let mask = context(size, size)
mask.addPath(RoundedRectangle(cornerRadius: 185.4, style: .continuous)
    .path(in: CGRect(x: inset, y: inset, width: side, height: side)).cgPath)
mask.setFillColor(.white)
mask.fillPath()

// Farbe aus dem Bild, Deckkraft allein aus der Maske (KI-Bilder sind oft nur zu ~99 % deckend)
let a = art.data!.assumingMemoryBound(to: UInt8.self)
let m = mask.data!.assumingMemoryBound(to: UInt8.self)
for i in stride(from: 0, to: size * size * 4, by: 4) {
    let srcA = Double(a[i + 3]), outA = Double(m[i + 3])
    for c in 0..<3 {
        let straight = srcA > 0 ? min(255, Double(a[i + c]) * 255 / srcA) : 0
        a[i + c] = UInt8((straight * outA / 255).rounded())
    }
    a[i + 3] = m[i + 3]
}

let rep = NSBitmapImageRep(cgImage: art.makeImage()!)
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: output))
print("Geschrieben: \(output)")
