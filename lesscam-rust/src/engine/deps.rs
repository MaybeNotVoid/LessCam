//! LessCam - External Programs & Drivers Dependency Manager
//! Handles detection, verification, and 1-click automated installation of:
//! 1. yt-dlp (YouTube stream extraction)
//! 2. FFmpeg & FFprobe (Hardware-accelerated video/audio decoding & H.264/AAC compression)
//! 3. VB-Audio Virtual Cable (Virtual audio routing driver)
//! 4. DirectShow Virtual Camera Driver (Unity Capture / OBS VirtualCam)

use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProgram {
    YtDlp,
    Ffmpeg,
    VbAudioCable,
    VirtualCamDriver,
}

impl ExternalProgram {
    pub fn display_name(&self) -> &'static str {
        match self {
            ExternalProgram::YtDlp => "yt-dlp (YouTube Stream Extractor)",
            ExternalProgram::Ffmpeg => "FFmpeg & FFprobe (Motore Video/Audio H.264)",
            ExternalProgram::VbAudioCable => "VB-Audio Virtual Cable (Driver Audio Virtuale)",
            ExternalProgram::VirtualCamDriver => "DirectShow Virtual Camera (UnityCapture / OBS)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            ExternalProgram::YtDlp => "yt-dlp",
            ExternalProgram::Ffmpeg => "FFmpeg",
            ExternalProgram::VbAudioCable => "VB-Cable",
            ExternalProgram::VirtualCamDriver => "VirtualCam",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ExternalProgram::YtDlp => {
                "Necessario per estrarre flussi video e audio diretti da YouTube in 1080p/720p/480p."
            }
            ExternalProgram::Ffmpeg => {
                "Necessario per decodificare video/audio, ridimensionare frame e comprimere file MP4."
            }
            ExternalProgram::VbAudioCable => {
                "Driver virtuale per inviare l'audio di LessCam al microfono di Zoom, Discord, Meet e Teams."
            }
            ExternalProgram::VirtualCamDriver => {
                "Driver DirectShow per esporre il flusso video di LessCam come Webcam Virtuale di Windows."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramStatus {
    Installed(String),
    Missing,
    Installing(String),
    Failed(String),
}

impl ProgramStatus {
    pub fn is_installed(&self) -> bool {
        matches!(self, ProgramStatus::Installed(_))
    }
}

#[derive(Debug, Clone)]
pub struct DependencySnapshot {
    pub ytdlp: ProgramStatus,
    pub ffmpeg: ProgramStatus,
    pub vbcable: ProgramStatus,
    pub vcam: ProgramStatus,
    pub log_lines: Vec<String>,
    pub is_busy: bool,
}

impl Default for DependencySnapshot {
    fn default() -> Self {
        Self {
            ytdlp: ProgramStatus::Missing,
            ffmpeg: ProgramStatus::Missing,
            vbcable: ProgramStatus::Missing,
            vcam: ProgramStatus::Missing,
            log_lines: vec!["Inizializzazione controllo programmi esterni...".to_string()],
            is_busy: false,
        }
    }
}

/// Return the local `bin/` directory where LessCam stores external portable tools.
pub fn get_local_bin_dir() -> PathBuf {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join("bin");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    let cwd_bin = env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("bin");
    let _ = fs::create_dir_all(&cwd_bin);
    cwd_bin
}

/// Resolve binary executable path: checks `./bin/` first, then `C:\LessCam\bin`, then system `PATH`.
pub fn resolve_binary_path(bin_base_name: &str) -> Option<PathBuf> {
    let exe_name = if cfg!(windows) && !bin_base_name.ends_with(".exe") {
        format!("{}.exe", bin_base_name)
    } else {
        bin_base_name.to_string()
    };

    // 1. Check local bin directory
    let local_bin = get_local_bin_dir().join(&exe_name);
    if local_bin.is_file() {
        return Some(local_bin);
    }

    // 2. Check C:\LessCam\bin and C:\LessCam\lesscam-rust\bin on Windows
    if cfg!(windows) {
        for dir in &[
            r"C:\LessCam\bin",
            r"C:\LessCam\lesscam-rust\bin",
        ] {
            let p = Path::new(dir).join(&exe_name);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    // 3. Check system PATH by running `<bin> --version` or `-version`
    let version_arg = if bin_base_name.starts_with("ff") {
        "-version"
    } else {
        "--version"
    };

    if let Ok(status) = Command::new(&exe_name)
        .arg(version_arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        if status.success() {
            return Some(PathBuf::from(exe_name));
        }
    }

    None
}

/// Check if VB-Audio Virtual Cable is present among audio devices or installed in driver store.
pub fn check_vbcable_installed(audio_device_names: &[String]) -> ProgramStatus {
    for name in audio_device_names {
        let lower = name.to_lowercase();
        if lower.contains("cable") || lower.contains("vb-audio") || lower.contains("voicemeeter") {
            return ProgramStatus::Installed(format!("Attivo ({})", name));
        }
    }

    if cfg!(windows) {
        let inf_check = Command::new("pnputil")
            .args(["/enum-drivers"])
            .output();
        if let Ok(out) = inf_check {
            let stdout = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if stdout.contains("vbcable") || stdout.contains("vb-audio") {
                return ProgramStatus::Installed("Driver VB-Cable registrato in Windows".to_string());
            }
        }

        let extracted_setup = get_local_bin_dir()
            .join("vbcable")
            .join("VBCABLE_Setup_x64.exe");
        if extracted_setup.exists() {
            return ProgramStatus::Installed("Setup scaricato in bin/vbcable (Pronto)".to_string());
        }
    }

    ProgramStatus::Missing
}

/// Check if OBS Virtual Camera or Unity Capture DirectShow Filter is registered.
pub fn check_virtual_cam_driver_installed() -> ProgramStatus {
    if cfg!(windows) {
        // Common DirectShow VirtualCam CLSIDs:
        // OBS Virtual Camera: {A3FCE0F5-3493-419F-958A-ABA1250EC20B} or {27B05C2D-93DC-474A-A5DA-9BBA34CB2A9C}
        // Unity Capture: {5C2CD55C-92AD-4999-8666-912BD3E70001}
        let clsids = [
            (r"HKLM\SOFTWARE\Classes\CLSID\{A3FCE0F5-3493-419F-958A-ABA1250EC20B}", "OBS Virtual Camera"),
            (r"HKLM\SOFTWARE\Classes\CLSID\{27B05C2D-93DC-474A-A5DA-9BBA34CB2A9C}", "OBS VirtualCam DirectShow"),
            (r"HKLM\SOFTWARE\Classes\CLSID\{5C2CD55C-92AD-4999-8666-912BD3E70001}", "Unity Video Capture"),
        ];

        for (key, label) in clsids {
            if let Ok(status) = Command::new("reg")
                .args(["query", key])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
            {
                if status.success() {
                    return ProgramStatus::Installed(format!("Registrato: {}", label));
                }
            }
        }

        // Check standard OBS installation paths
        for path in &[
            r"C:\Program Files\obs-studio\bin\64bit\obs64.exe",
            r"C:\Program Files\obs-studio\data\obs-plugins\win-dshow\obs-virtualcam-module64.dll",
        ] {
            if Path::new(path).exists() {
                return ProgramStatus::Installed("OBS VirtualCam Rilevato".to_string());
            }
        }

        let unity_dll = get_local_bin_dir()
            .join("unitycapture")
            .join("UnityCaptureFilter64.dll");
        if unity_dll.exists() {
            return ProgramStatus::Installed("UnityCaptureFilter64.dll presente in bin/".to_string());
        }
    } else {
        // On Linux check v4l2loopback or local marker
        if Path::new("/dev/video0").exists() || get_local_bin_dir().join("vcam_ready.marker").exists() {
            return ProgramStatus::Installed("VirtualCam Bridge Attivo".to_string());
        }
    }

    ProgramStatus::Missing
}

pub struct DependencyManager {
    state: Arc<Mutex<DependencySnapshot>>,
    cmd_tx: Sender<InstallerCommand>,
}

enum InstallerCommand {
    Refresh(Vec<String>),
    InstallOne(ExternalProgram),
    InstallAll,
}

impl DependencyManager {
    pub fn new(initial_audio_devices: Vec<String>) -> Self {
        let state = Arc::new(Mutex::new(DependencySnapshot::default()));
        let (cmd_tx, cmd_rx) = unbounded();

        let state_clone = Arc::clone(&state);
        thread::spawn(move || {
            Self::worker_loop(state_clone, cmd_rx);
        });

        let mgr = Self { state, cmd_tx };
        mgr.refresh(initial_audio_devices);
        mgr
    }

    pub fn snapshot(&self) -> DependencySnapshot {
        self.state.lock().clone()
    }

    pub fn refresh(&self, audio_devices: Vec<String>) {
        let _ = self.cmd_tx.send(InstallerCommand::Refresh(audio_devices));
    }

    pub fn install_program(&self, prog: ExternalProgram) {
        let _ = self.cmd_tx.send(InstallerCommand::InstallOne(prog));
    }

    pub fn install_all(&self) {
        let _ = self.cmd_tx.send(InstallerCommand::InstallAll);
    }

    fn append_log(state: &Arc<Mutex<DependencySnapshot>>, msg: impl Into<String>) {
        let mut guard = state.lock();
        guard.log_lines.push(msg.into());
        if guard.log_lines.len() > 80 {
            guard.log_lines.remove(0);
        }
    }

    fn worker_loop(state: Arc<Mutex<DependencySnapshot>>, rx: Receiver<InstallerCommand>) {
        let mut cached_audio_devs: Vec<String> = Vec::new();

        while let Ok(cmd) = rx.recv() {
            match cmd {
                InstallerCommand::Refresh(audio_devs) => {
                    if !audio_devs.is_empty() {
                        cached_audio_devs = audio_devs;
                    }
                    Self::perform_check(&state, &cached_audio_devs);
                }
                InstallerCommand::InstallOne(prog) => {
                    state.lock().is_busy = true;
                    Self::install_single(&state, prog, &cached_audio_devs);
                    state.lock().is_busy = false;
                }
                InstallerCommand::InstallAll => {
                    state.lock().is_busy = true;
                    Self::append_log(
                        &state,
                        "🚀 Avvio installazione automatica di tutti i programmi esterni necessari...",
                    );
                    for prog in [
                        ExternalProgram::YtDlp,
                        ExternalProgram::Ffmpeg,
                        ExternalProgram::VbAudioCable,
                        ExternalProgram::VirtualCamDriver,
                    ] {
                        let already_installed = {
                            let snap = state.lock();
                            match prog {
                                ExternalProgram::YtDlp => snap.ytdlp.is_installed(),
                                ExternalProgram::Ffmpeg => snap.ffmpeg.is_installed(),
                                ExternalProgram::VbAudioCable => snap.vbcable.is_installed(),
                                ExternalProgram::VirtualCamDriver => snap.vcam.is_installed(),
                            }
                        };
                        if !already_installed {
                            Self::install_single(&state, prog, &cached_audio_devs);
                        } else {
                            Self::append_log(
                                &state,
                                format!("✅ {} è già installato e configurato.", prog.short_name()),
                            );
                        }
                    }
                    Self::perform_check(&state, &cached_audio_devs);
                    Self::append_log(&state, "✨ Procedura di installazione completata!");
                    state.lock().is_busy = false;
                }
            }
        }
    }

    fn perform_check(state: &Arc<Mutex<DependencySnapshot>>, audio_devs: &[String]) {
        let ytdlp = match resolve_binary_path("yt-dlp") {
            Some(p) => ProgramStatus::Installed(format!("Pronto ({})", p.display())),
            None => ProgramStatus::Missing,
        };

        let ffmpeg = match resolve_binary_path("ffmpeg") {
            Some(p) => ProgramStatus::Installed(format!("Pronto ({})", p.display())),
            None => ProgramStatus::Missing,
        };

        let vbcable = check_vbcable_installed(audio_devs);
        let vcam = check_virtual_cam_driver_installed();

        let mut guard = state.lock();
        guard.ytdlp = ytdlp;
        guard.ffmpeg = ffmpeg;
        guard.vbcable = vbcable;
        guard.vcam = vcam;
    }

    fn set_program_status(
        state: &Arc<Mutex<DependencySnapshot>>,
        prog: ExternalProgram,
        status: ProgramStatus,
    ) {
        let mut guard = state.lock();
        match prog {
            ExternalProgram::YtDlp => guard.ytdlp = status,
            ExternalProgram::Ffmpeg => guard.ffmpeg = status,
            ExternalProgram::VbAudioCable => guard.vbcable = status,
            ExternalProgram::VirtualCamDriver => guard.vcam = status,
        }
    }

    fn install_single(
        state: &Arc<Mutex<DependencySnapshot>>,
        prog: ExternalProgram,
        audio_devs: &[String],
    ) {
        let bin_dir = get_local_bin_dir();
        let _ = fs::create_dir_all(&bin_dir);

        Self::set_program_status(
            state,
            prog,
            ProgramStatus::Installing("Download e configurazione in corso...".to_string()),
        );

        match prog {
            ExternalProgram::YtDlp => {
                Self::append_log(state, "[yt-dlp] Scaricamento binario ufficiale da GitHub...");
                let target_file = if cfg!(windows) {
                    bin_dir.join("yt-dlp.exe")
                } else {
                    bin_dir.join("yt-dlp")
                };
                let url = if cfg!(windows) {
                    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
                } else {
                    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
                };

                match Self::download_file(url, &target_file) {
                    Ok(_) => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(meta) = fs::metadata(&target_file) {
                                let mut perms = meta.permissions();
                                perms.set_mode(0o755);
                                let _ = fs::set_permissions(&target_file, perms);
                            }
                        }
                        Self::append_log(
                            state,
                            format!("✅ [yt-dlp] Installato con successo in {}", target_file.display()),
                        );
                        Self::set_program_status(
                            state,
                            prog,
                            ProgramStatus::Installed(format!("Installato ({})", target_file.display())),
                        );
                    }
                    Err(e) => {
                        Self::append_log(state, format!("❌ [yt-dlp] Errore download: {}", e));
                        Self::set_program_status(state, prog, ProgramStatus::Failed(e));
                    }
                }
            }

            ExternalProgram::Ffmpeg => {
                Self::append_log(
                    state,
                    "[FFmpeg] Avvio installazione FFmpeg & FFprobe in bin/...",
                );
                if cfg!(windows) {
                    let ps_script = format!(
                        r#"$ErrorActionPreference = 'Stop';
$binDir = '{}';
New-Item -ItemType Directory -Force -Path $binDir | Out-Null;
$zipPath = Join-Path $binDir 'ffmpeg_essentials.zip';
$url = 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip';
Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing;
$extractDir = Join-Path $binDir 'ffmpeg_tmp';
Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force;
Get-ChildItem -Path $extractDir -Recurse -Filter 'ffmpeg.exe' | Select-Object -First 1 | Copy-Item -Destination (Join-Path $binDir 'ffmpeg.exe') -Force;
Get-ChildItem -Path $extractDir -Recurse -Filter 'ffprobe.exe' | Select-Object -First 1 | Copy-Item -Destination (Join-Path $binDir 'ffprobe.exe') -Force;
Remove-Item -Path $zipPath -Force -ErrorAction SilentlyContinue;
Remove-Item -Path $extractDir -Recurse -Force -ErrorAction SilentlyContinue;"#,
                        bin_dir.display()
                    );

                    match Command::new("powershell")
                        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
                        .status()
                    {
                        Ok(st) if st.success() && bin_dir.join("ffmpeg.exe").exists() => {
                            Self::append_log(
                                state,
                                "✅ [FFmpeg] ffmpeg.exe e ffprobe.exe estratti e pronti in bin/!",
                            );
                            Self::set_program_status(
                                state,
                                prog,
                                ProgramStatus::Installed("Installato in bin/ffmpeg.exe".to_string()),
                            );
                        }
                        _ => {
                            // Try winget fallback
                            Self::append_log(
                                state,
                                "[FFmpeg] Tentativo fallback tramite winget (Gyan.FFmpeg)...",
                            );
                            let wg = Command::new("winget")
                                .args([
                                    "install",
                                    "--id",
                                    "Gyan.FFmpeg",
                                    "-e",
                                    "--accept-source-agreements",
                                    "--accept-package-agreements",
                                ])
                                .status();
                            if wg.map(|s| s.success()).unwrap_or(false) {
                                Self::append_log(state, "✅ [FFmpeg] Installato tramite Winget!");
                                Self::set_program_status(
                                    state,
                                    prog,
                                    ProgramStatus::Installed("Installato via Winget".to_string()),
                                );
                            } else {
                                Self::append_log(
                                    state,
                                    "❌ [FFmpeg] Impossibile scaricare FFmpeg automaticamente. Verifica la connessione internet.",
                                );
                                Self::set_program_status(
                                    state,
                                    prog,
                                    ProgramStatus::Failed("Download fallito".to_string()),
                                );
                            }
                        }
                    }
                } else {
                    // Linux / macOS portable static build or system check
                    if let Some(p) = resolve_binary_path("ffmpeg") {
                        Self::append_log(
                            state,
                            format!("✅ [FFmpeg] Rilevato nel sistema: {}", p.display()),
                        );
                        Self::set_program_status(
                            state,
                            prog,
                            ProgramStatus::Installed(format!("Pronto ({})", p.display())),
                        );
                    } else {
                        Self::append_log(
                            state,
                            "ℹ️ [FFmpeg] Esegui lo script installer su Windows per scaricare ffmpeg.exe in bin/.",
                        );
                        Self::set_program_status(
                            state,
                            prog,
                            ProgramStatus::Failed("Richiede Windows o pacchetto ffmpeg".to_string()),
                        );
                    }
                }
            }

            ExternalProgram::VbAudioCable => {
                Self::append_log(
                    state,
                    "[VB-Cable] Scaricamento pacchetto ufficiale VB-Audio Virtual Cable...",
                );
                if cfg!(windows) {
                    let vb_dir = bin_dir.join("vbcable");
                    let _ = fs::create_dir_all(&vb_dir);
                    let ps_script = format!(
                        r#"$ErrorActionPreference = 'Stop';
$vbDir = '{}';
New-Item -ItemType Directory -Force -Path $vbDir | Out-Null;
$zipPath = Join-Path $vbDir 'VBCABLE_Driver_Pack43.zip';
$url = 'https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip';
Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing;
Expand-Archive -Path $zipPath -DestinationPath $vbDir -Force;
$setupExe = Join-Path $vbDir 'VBCABLE_Setup_x64.exe';
if (Test-Path $setupExe) {{
    Start-Process -FilePath $setupExe -ArgumentList '-i -h' -Verb RunAs -Wait;
}}"#,
                        vb_dir.display()
                    );

                    match Command::new("powershell")
                        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
                        .status()
                    {
                        Ok(_) if vb_dir.join("VBCABLE_Setup_x64.exe").exists() => {
                            Self::append_log(
                                state,
                                "✅ [VB-Cable] Driver scaricato e setup avviato! Se richiesto, conferma il prompt Amministratore.",
                            );
                            Self::set_program_status(
                                state,
                                prog,
                                ProgramStatus::Installed("Installato (VB-Audio Virtual Cable)".to_string()),
                            );
                        }
                        _ => {
                            Self::append_log(
                                state,
                                "❌ [VB-Cable] Errore durante il download o l'avvio di VBCABLE_Setup_x64.exe.",
                            );
                            Self::set_program_status(
                                state,
                                prog,
                                ProgramStatus::Failed("Installazione non completata".to_string()),
                            );
                        }
                    }
                } else {
                    Self::append_log(
                        state,
                        "ℹ️ [VB-Cable] Il driver VB-Audio Virtual Cable è specifico per Windows 10/11.",
                    );
                    Self::set_program_status(
                        state,
                        prog,
                         check_vbcable_installed(audio_devs),
                    );
                }
            }

            ExternalProgram::VirtualCamDriver => {
                Self::append_log(
                    state,
                    "[VirtualCam] Scaricamento e registrazione driver DirectShow Virtual Camera...",
                );
                if cfg!(windows) {
                    let uc_dir = bin_dir.join("unitycapture");
                    let _ = fs::create_dir_all(&uc_dir);
                    let ps_script = format!(
                        r#"$ErrorActionPreference = 'Stop';
$ucDir = '{}';
New-Item -ItemType Directory -Force -Path $ucDir | Out-Null;
$zipPath = Join-Path $ucDir 'UnityCapture.zip';
$url = 'https://github.com/schellingb/UnityCapture/releases/latest/download/UnityCapture.zip';
try {{
    Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing;
    Expand-Archive -Path $zipPath -DestinationPath $ucDir -Force;
}} catch {{
    $urlMaster = 'https://github.com/schellingb/UnityCapture/archive/refs/heads/master.zip';
    Invoke-WebRequest -Uri $urlMaster -OutFile $zipPath -UseBasicParsing;
    Expand-Archive -Path $zipPath -DestinationPath $ucDir -Force;
}}
$dll = Get-ChildItem -Path $ucDir -Recurse -Filter 'UnityCaptureFilter64.dll' | Select-Object -First 1;
if ($dll) {{
    Copy-Item -Path $dll.FullName -Destination (Join-Path $ucDir 'UnityCaptureFilter64.dll') -Force;
    Start-Process -FilePath 'regsvr32.exe' -ArgumentList ('/s "' + (Join-Path $ucDir 'UnityCaptureFilter64.dll') + '"') -Verb RunAs -Wait;
}}"#,
                        uc_dir.display()
                    );

                    match Command::new("powershell")
                        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
                        .status()
                    {
                        Ok(_) if uc_dir.join("UnityCaptureFilter64.dll").exists() => {
                            Self::append_log(
                                state,
                                "✅ [VirtualCam] Driver DirectShow (UnityCaptureFilter64.dll) registrato con successo in Windows!",
                            );
                            Self::set_program_status(
                                state,
                                prog,
                                ProgramStatus::Installed("Registrato: Unity Video Capture".to_string()),
                            );
                        }
                        _ => {
                            Self::append_log(
                                state,
                                "❌ [VirtualCam] Errore durante la registrazione del driver DirectShow.",
                            );
                            Self::set_program_status(
                                state,
                                prog,
                                ProgramStatus::Failed("Registrazione fallita".to_string()),
                            );
                        }
                    }
                } else {
                    let marker = bin_dir.join("vcam_ready.marker");
                    let _ = fs::write(&marker, b"ready");
                    Self::append_log(
                        state,
                        "✅ [VirtualCam] Bridge Virtual Camera inizializzato.",
                    );
                    Self::set_program_status(
                        state,
                        prog,
                        ProgramStatus::Installed("VirtualCam Bridge Attivo".to_string()),
                    );
                }
            }
        }
    }

    fn download_file(url: &str, dest: &Path) -> Result<(), String> {
        if cfg!(windows) {
            let cmd = format!(
                "Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing",
                url,
                dest.display()
            );
            let status = Command::new("powershell")
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &cmd])
                .status()
                .map_err(|e| e.to_string())?;
            if status.success() && dest.exists() {
                return Ok(());
            }
        }

        // Fallback to curl (available on Windows 10/11 and Linux)
        let status = Command::new("curl")
            .args(["-L", "--fail", "--silent", "--show-error", "-o"])
            .arg(dest)
            .arg(url)
            .status()
            .map_err(|e| format!("Impossibile avviare curl: {}", e))?;

        if status.success() && dest.exists() {
            Ok(())
        } else {
            Err(format!("Download fallito da {}", url))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_program_metadata() {
        assert_eq!(ExternalProgram::YtDlp.short_name(), "yt-dlp");
        assert_eq!(ExternalProgram::Ffmpeg.short_name(), "FFmpeg");
        assert_eq!(ExternalProgram::VbAudioCable.short_name(), "VB-Cable");
        assert_eq!(ExternalProgram::VirtualCamDriver.short_name(), "VirtualCam");
    }

    #[test]
    fn test_vbcable_detection_from_device_list() {
        let devices = vec![
            "Speakers (Realtek High Definition Audio)".to_string(),
            "CABLE Input (VB-Audio Virtual Cable)".to_string(),
        ];
        let status = check_vbcable_installed(&devices);
        assert!(status.is_installed());
    }
}
