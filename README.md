# LessCam Studio 2.0 🎥🦀✨
> **A Modern Virtual Camera, Audio Router & Computer Vision Automation Studio for Windows 11 (Rust 2.0 & Python Reference)**

<div align="center">

[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-orange?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![GUI](https://img.shields.io/badge/GUI-eframe%20%2F%20egui%20GPU%2060FPS-00A8FF?logo=opengl&logoColor=white)](https://github.com/emilk/egui)
[![Platform](https://img.shields.io/badge/Platform-Windows%2011%20%7C%2010-0078D4?logo=windows&logoColor=white)](https://microsoft.com)
[![Python](https://img.shields.io/badge/Python%20Legacy-3.10%2B-blue?logo=python&logoColor=white)](https://python.org)
[![License](https://img.shields.io/badge/License-MIT-purple.svg)](LICENSE)

<br/>

**Regia multimediale desktop nativa in Rust, ultra-veloce, con interfaccia grafica GPU-accelerata "Obsidian Studio Modern" e Installer Automatico 1-Click per tutti i programmi e driver esterni.**  
Trasmetti video da YouTube, file locali compressi in H.264 o immagini su **Discord, Zoom, Microsoft Teams, Google Meet e OBS Studio** con audio routing virtuale e automazione mouse Computer Vision integrata.

</div>

---

## 🦀 Novità di LessCam Studio 2.0 (Rust Edition in `lesscam-rust/`)

1. **Motore Nativo in Rust Ultra-Veloce**:
   - Zero overhead dell'interprete Python, multi-threading nativo con `rayon` e `crossbeam-channel`, consumo minimo di RAM e CPU.
   - **Template Matching Computer Vision in puro Rust (`match_template_ncc`)** parallelizzato su tutti i core della CPU.
   - **Audio Mixer Low-Latency WASAPI (`cpal`)** con guadagno `0% - 200%` (`-∞ dB` a `+6.0 dB`), limitatore soft-clipping `tanh()` e calcolo RMS/Peak stereo in tempo reale.
2. **Interfaccia Grafica GPU-Accelerata ("Obsidian Studio Modern")**:
   - Costruita con `eframe` / `egui` e rendering OpenGL/Glow a **60 FPS costanti**.
   - Palette dark stratificata, card arrotondate con bordi luminosi sottili, switch animati, badge `● LIVE ON AIR`, **Stereo VU-Meter LED** a 28 segmenti con decadimento balistico e **Peak-Hold**.
3. **Installer Automatico Programmi & Driver Esterni (Script + Pannello GUI 1-Click)**:
   - Scarica, estrae e configura automaticamente nella cartella locale `bin/`:
     - **`yt-dlp.exe`** (estrazione flussi diretti YouTube 1080p/720p/480p)
     - **`ffmpeg.exe` & `ffprobe.exe`** (decodifica stream universale e trimming/compressione H.264/AAC)
     - **`VB-Audio Virtual Cable`** (download automatico ed esecuzione setup driver audio virtuale)
     - **`DirectShow Virtual Camera Driver`** (registrazione automatica di `UnityCaptureFilter64.dll` via `regsvr32` / compatibilità `OBS Virtual Camera`)

---

## 🚀 Installazione Rapida in `C:\LessCam` (Windows 10 / 11)

Apri **PowerShell** o il **Prompt dei Comandi (CMD)** ed esegui:

```powershell
git clone https://github.com/MaybeNotVoid/LessCam.git C:\LessCam
cd C:\LessCam\lesscam-rust\installer
.\Install_LessCam_Rust.bat
```

Oppure, se hai già scaricato il repository, fai semplicemente doppio clic su:
- **`Installa_Su_C_LessCam.bat`** (clona/copia automaticamente il progetto in `C:\LessCam`, scarica `FFmpeg`, `yt-dlp`, `VB-Audio Cable`, registra la `Virtual Camera` e compila `lesscam.exe` in modalità `--release`).
- Per avviare l'app Rust in qualsiasi momento: doppio clic su **`lesscam-rust\Avvia_LessCam_Rust.bat`** (oppure `C:\LessCam\lesscam-rust\lesscam.exe`).

---

## 📂 Struttura del Repository

```text
LessCam/
│
├── Installa_Su_C_LessCam.bat         # Script 1-Click per clonare e installare tutto in C:\LessCam
├── README.md                         # Documentazione del progetto
│
├── lesscam-rust/                     # 🦀 NUOVA EDIZIONE NATIVA IN RUST (LessCam Studio 2.0)
│   ├── Cargo.toml                    # Configurazione crate e dipendenze Rust
│   ├── Avvia_LessCam_Rust.bat        # Launcher rapido per la versione Rust
│   ├── installer/                    # Installer automatici per programmi esterni e setup Windows
│   │   ├── Install_LessCam_Rust.bat  # Installer completo (scarica dipendenze + compila release)
│   │   ├── setup_dependencies.ps1    # Auto-downloader PowerShell (yt-dlp, FFmpeg, VB-Cable, VirtualCam)
│   │   └── LessCam_Setup.iss         # Script Inno Setup per generare LessCam_Studio_2.0_Rust_Setup_x64.exe
│   └── src/
│       ├── main.rs                   # Entrypoint applicazione Rust
│       ├── engine/                   # Core Engine multithread in Rust
│       │   ├── deps.rs               # Gestore e installer 1-click programmi esterni
│       │   ├── youtube.rs            # Estrazione asincrona stream YouTube via yt-dlp
│       │   ├── video_stream.rs       # Pipeline video RGB24, letterboxing e DirectShow VirtualCam
│       │   ├── video_trimmer.rs      # Analisi ffprobe e compressione H.264/AAC con progresso live
│       │   ├── audio_stream.rs       # Mixer audio WASAPI/cpal, gain 0-200%, tanh limiter & RMS
│       │   └── mouse_automation.rs   # Computer Vision NCC multi-core (rayon), 4 slot, scroll & hotkeys
│       └── gui/                      # Interfaccia GPU-accelerata Obsidian Studio Modern (eframe/egui)
│           ├── theme.rs              # Design system dark, card arrotondate, switch animati e pillole
│           ├── app.rs                # Finestra principale e coordinamento pannelli
│           ├── preview.rs            # Video canvas GPU con badge LIVE e HUD risoluzione
│           ├── vu_meter.rs           # Stereo VU-Meter LED a 28 segmenti con Peak-Hold
│           ├── trim_modal.rs         # Finestra modale di taglio e compressione video
│           └── deps_modal.rs         # Finestra modale per installazione 1-click dei programmi esterni
│
├── main.py                           # 🐍 Versione Python originale (conservata come riferimento)
├── Avvia_LessCam.bat                 # Launcher versione Python
├── engine/                           # Moduli motore Python
└── gui/                              # Moduli interfaccia PyQt6
```

---

## 🧪 Test Automatizzati (Rust)

Per eseguire la suite di unit test del motore Rust (validazione YouTube, letterbox aspect ratio, template matching NCC multi-core, gain/soft-clipping audio e rilevamento dipendenze):

```bash
cargo test --manifest-path lesscam-rust/Cargo.toml
```

---

## 📜 Licenza
Rilasciato sotto licenza MIT.
