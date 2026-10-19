import AVFoundation
import Foundation

let semaphore = DispatchSemaphore(value: 0)
AVCaptureDevice.requestAccess(for: .video) { granted in
    print(granted ? "granted" : "denied")
    semaphore.signal()
}
semaphore.wait()
