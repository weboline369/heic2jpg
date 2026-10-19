#!/bin/sh
set -eu

cargo build --release

swiftc packaging/generate_icon.swift -o /tmp/heic2jpg-generate-icon
mkdir -p target/AppIcon.iconset
/tmp/heic2jpg-generate-icon target/AppIcon-master.png
sips -z 16 16 target/AppIcon-master.png --out target/AppIcon.iconset/icon_16x16.png >/dev/null
sips -z 32 32 target/AppIcon-master.png --out target/AppIcon.iconset/icon_16x16@2x.png >/dev/null
sips -z 32 32 target/AppIcon-master.png --out target/AppIcon.iconset/icon_32x32.png >/dev/null
sips -z 64 64 target/AppIcon-master.png --out target/AppIcon.iconset/icon_32x32@2x.png >/dev/null
sips -z 128 128 target/AppIcon-master.png --out target/AppIcon.iconset/icon_128x128.png >/dev/null
sips -z 256 256 target/AppIcon-master.png --out target/AppIcon.iconset/icon_128x128@2x.png >/dev/null
sips -z 256 256 target/AppIcon-master.png --out target/AppIcon.iconset/icon_256x256.png >/dev/null
sips -z 512 512 target/AppIcon-master.png --out target/AppIcon.iconset/icon_256x256@2x.png >/dev/null
sips -z 512 512 target/AppIcon-master.png --out target/AppIcon.iconset/icon_512x512.png >/dev/null
sips -z 1024 1024 target/AppIcon-master.png --out target/AppIcon.iconset/icon_512x512@2x.png >/dev/null
iconutil -c icns target/AppIcon.iconset -o target/AppIcon.icns

app_dir="target/HEIC JPG.app/Contents"
mkdir -p "$app_dir/MacOS" "$app_dir/Resources"
swiftc packaging/camera_permission.swift -o "$app_dir/MacOS/camera_permission"
swiftc packaging/camera_stream.swift -o "$app_dir/MacOS/camera_stream"
cp target/release/heic2jpg "$app_dir/MacOS/heic2jpg"
cp packaging/Info.plist "$app_dir/Info.plist"
cp target/AppIcon.icns "$app_dir/Resources/AppIcon.icns"

echo "Application créée : target/HEIC JPG.app"
