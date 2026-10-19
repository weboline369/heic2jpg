import AVFoundation
import CoreImage
import Foundation

final class Delegate: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    let context = CIContext()
    let output = FileHandle.standardOutput

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        guard let buffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let image = CIImage(cvPixelBuffer: buffer)
        let options: [CIImageRepresentationOption: Any] = [
            kCGImageDestinationLossyCompressionQuality as CIImageRepresentationOption: 0.95
        ]
        guard let data = context.jpegRepresentation(
            of: image,
            colorSpace: CGColorSpaceCreateDeviceRGB(),
            options: options
        ) else { return }
        var length = UInt32(data.count).bigEndian
        let header = Data(bytes: &length, count: 4)
        try? self.output.write(contentsOf: header)
        try? self.output.write(contentsOf: data)
    }
}

let session = AVCaptureSession()
// Résolution maximale disponible sans ralentir le flux d’aperçu.
session.sessionPreset = session.canSetSessionPreset(.photo) ? .photo : .high
guard let device = AVCaptureDevice.default(for: .video),
      let input = try? AVCaptureDeviceInput(device: device),
      session.canAddInput(input) else { exit(2) }
session.addInput(input)

let video = AVCaptureVideoDataOutput()
video.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA]
video.alwaysDiscardsLateVideoFrames = true
let delegate = Delegate()
video.setSampleBufferDelegate(delegate, queue: DispatchQueue(label: "heic2jpg.camera"))
guard session.canAddOutput(video) else { exit(3) }
session.addOutput(video)
session.startRunning()
RunLoop.main.run()
