# HEIC → JPG pour macOS

Application native macOS écrite en Rust pour convertir des images HEIC/HEIF en JPEG et prendre des photos via une webcam.

## Fonctionnalités

- Conversion locale HEIC/HEIF → JPG avec `sips`.
- Onglet `WEBCAM → JPG` avec aperçu live AVFoundation.
- Capture webcam au format JPEG.
- Dossier `Downloads` par défaut.
- Persistance des dossiers, qualité JPEG, remplacement des fichiers et dernier onglet.
- Ouverture automatique du dossier de sortie après conversion ou capture.
- Interface egui inspirée de Bulma et adaptée macOS.
- Menu natif macOS `À propos` avec les informations Weboline.

## Pré-requis

- macOS.
- Rust stable et Cargo.
- Permission macOS d’accès à la caméra pour l’onglet webcam.

La conversion HEIC utilise `/usr/bin/sips`. FFmpeg n’est pas nécessaire pour le flux webcam actuel.

## Installation et lancement

```sh
rustup toolchain install stable
cargo run
```

Pour créer le bundle macOS :

```sh
sh packaging/build_app.sh
open "target/HEIC JPG.app"
```

## Configuration persistante

Les préférences sont stockées localement dans :

```text
~/Library/Application Support/HEIC2JPG/directories.conf
```

Aucune donnée image n’est envoyée à un service distant.

## Développement

```sh
cargo fmt
cargo check
cargo build --release
```

## Éditeur

Weboline — <https://weboline.fr>

## Licence

MIT. Voir [`Cargo.toml`](Cargo.toml).
