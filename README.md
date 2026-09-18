# LessCam 🎥✨
> **A Modern Virtual Camera, Audio Router & Computer Vision Automation Studio for Windows 11**

<div align="center">

[![Python](https://img.shields.io/badge/Python-3.10%20%7C%203.11%20%7C%203.12-blue?logo=python&logoColor=white)](https://python.org)
[![Platform](https://img.shields.io/badge/Platform-Windows%2011%20%7C%2010-0078D4?logo=windows&logoColor=white)](https://microsoft.com)
[![GUI](https://img.shields.io/badge/GUI-PyQt6%20%26%20QFluentWidgets-darkgreen?logo=qt)](https://github.com/zhiyiYo/PyQt-Fluent-Widgets)
[![License](https://img.shields.io/badge/License-MIT-purple.svg)](LICENSE)

<br/>

**Regia multimediale desktop moderna, leggera e fluida con interfaccia nativa Fluent Design per Windows 11.**  
Trasmetti video da YouTube, file locali compressi o immagini su **Discord, Zoom, Microsoft Teams, Google Meet e OBS Studio** con audio routing virtuale e automazione mouse integrata.

</div>

---

## 🎭 La Nostra Ispirazione: *"Un certo software chiamato ManyCam"*

> 💬 **Dichiarazione di Non-Responsabilità (Totalmente Ironica):**  
> *"Ogni somiglianza con un leggendario, mastodontico software proprietario noto come **ManyCam** è puramente voluta... tranne per il fatto che abbiamo tolto tutto ciò che lo rendeva insopportabile!*  
> 
> *Perché pagare abbonamenti mensili salatissimi, farsi piazzare un watermark gigante sul viso a meno che non tiri fuori la carta di credito, o trasformare la ventola del PC in una turbina Boeing 747 solo per trasmettere un video su Zoom?*  
>
> *Come diceva il saggio: **Less is More**.*  
> *Meno RAM sprecata, Meno CPU fusa, Meno pop-up di rinnovo licenza... e 100% più libertà.*  
> *Da qui: **LessCam**.* 🚀

---

## 🌟 Funzionalità Principali

### 🎨 1. Interfaccia Fluent Design per Windows 11
- **Look & Feel Nativo**: Tema Scuro (`setTheme(Theme.DARK)`), angoli arrotondati, animazioni fluide, palette colori Windows 11 e componenti *QFluentWidgets*.
- **Controlli Ergonomici**: Anteprima video a basso consumo con badge LIVE, indicatore di buffering/risoluzione e controlli a scomparsa.
- **Standby Screen**: Schermata di standby moderna con pattern dark ed indicatore animato quando nessuna sorgente è in riproduzione.

### 🎥 2. Multi-Sorgente Video ad Alte Prestazioni
- **YouTube Live/Video Streaming**:
  - Incolla qualsiasi URL di YouTube per estrarre istantaneamente i flussi video/audio diretti tramite `yt-dlp`.
  - Risoluzione selezionabile in tempo reale (**1080p, 720p, 480p**).
- **Video Locali con Smart Trim & Compressione**:
  - Supporto universale: **MP4, MKV, AVI, MOV, WEBM**.
  - **Trimmer Integrato**: imposta punto di inizio e punto di fine direttamente prima del caricamento.
  - **Compressione Intelligente (H.264 / AAC via PyAV)**: riduce drasticamente il peso del file mantenendo un'altissima qualità visiva (con notifica dello spazio risparmiato).
  - Timeline interattiva con scrubbing fluido e riproduzione continua in loop.
- **Immagini Statiche**: visualizzazione continua in streaming di file PNG, JPG, JPEG, WEBP e BMP.

### 📡 3. Virtual Camera Output (DirectShow / OBS VirtualCam)
- Trasmissione continua dei frame tramite `pyvirtualcam` verso la webcam virtuale di sistema.
- **Rescaling & Aspect Ratio**: Adattamento automatico della risoluzione (1080p / 720p a 30 o 60 FPS) con letterbox/pillarbox su sfondo nero senza distorsioni o allungamenti.
- **High-Precision Clock Pacing**: Sincronizzazione precisa dei frame (`time.perf_counter()`) per eliminare frame drop e micro-stuttering.
- **Failsafe**: Se il driver della webcam virtuale non è attivo, l'anteprima continua a funzionare regolarmente mostrando un avviso chiaro senza mai andare in crash.

### 🔊 4. Mixer Audio & Routing Virtuale Stereo
- Decodifica audio multiformato in streaming tramite `PyAV`.
- Instradamento diretto a bassa latenza su periferiche fisiche o virtuali tramite `sounddevice` (es. **VB-Audio Virtual Cable**).
- **Controllo Gain da 0% a 200%** (+6 dB) con limitatore dinamico soft-clipping per prevenire distorsioni digitali.
- **VU-Meter Stereo (Left / Right)** ad alta reattività con calcolo RMS, indicatore di picco (**Peak Hold**) e decadimento balistico a 30 FPS.
- Funzione **Mute** istantanea a un clic.

### 🖱️ 5. Automazione Mouse con Computer Vision (Mouse Bot)
- **Targeting Flessibile**: Scegli una finestra attiva specifica di Windows o l'intero Desktop primario.
- **Anti-AFK Jiggler**: Movimento naturale del mouse a intervalli regolari (con campo numerico chiaro e visibile in secondi).
- **Riconoscimento Immagini Multi-Target (Max 4 Slot)**:
  - Carica fino a 4 immagini target (.png, .jpg, .webp).
  - Template Matching OpenCV in tempo reale (`cv2.matchTemplate`).
- **Barra Regolazione Precisione (50% - 99%)**:
  - Slider dedicato per tarare la soglia di tolleranza di riconoscimento.
  - Impostando la precisione al 90-95% si evitano falsi positivi su pulsanti o icone graficamente simili.
- **Scroll Down Post-Click Configurabile**:
  - Scatti di rotella regolabili (da 1 a 20) e ritardo post-click personalizzabile.
  - Spunta `[x] Scroll` attivabile o disattivabile singolarmente per ciascun target.
- **Due Pulsanti Dedicati**: Avvia e Arresta separati, eliminando qualsiasi sovrapposizione visiva di icone.
- **Scorciatoie Globali da Tastiera**:
  - **`CTRL + ALT + A`**: Avvia il bot mouse da qualsiasi applicazione.
  - **`CTRL + ALT + D`**: Arresta immediatamente il bot.
  - **Failsafe Corner**: Sposta il mouse nell'angolo estremo dello schermo per un arresto immediato d'emergenza.

---

## 🛠️ Requisiti Software Consigliati

Per sfruttare al massimo LessCam nelle tue app di videoconferenza:

1. **Webcam Virtuale**:
   - Installa [OBS Studio](https://obsproject.com/).
   - Aprilo una volta, clicca su *"Avvia fotocamera virtuale"* e richiudilo: il driver DirectShow rimarrà installato in Windows.
   - Su Zoom/Meet/Discord seleziona **"OBS Virtual Camera"**.
2. **Audio Routing Virtuale**:
   - Installa il driver gratuito [VB-Audio Virtual Cable](https://vb-audio.com/Cable/).
   - In LessCam seleziona come uscita audio **`CABLE Input (VB-Audio Virtual Cable)`**.
   - Su Zoom/Discord/Meet imposta il tuo microfono su **`CABLE Output (VB-Audio Virtual Cable)`**.

---

## 🚀 Installazione ed Esecuzione

### Requisiti
- **Windows 10 / 11 (64-bit)**
- **Python 3.10+** (consigliato 3.12)

### 1. Clonazione del Repository
```powershell
git clone https://github.com/MaybeNotVoid/LessCam.git
cd LessCam
```

### 2. Installazione Dipendenze
```powershell
pip install -r requirements.txt
```

### 3. Avvio dell'Applicazione
Puoi avviare l'applicazione in due modi:
- **Con il launcher batch**: doppio clic sul file **`Avvia_LessCam.bat`**.
- **Da terminale**:
  ```powershell
  python main.py
  ```

---

## 📂 Struttura del Progetto

```
LessCam/
│
├── Avvia_LessCam.bat        # Launcher batch Windows con auto-rilevamento Python
├── main.py                  # Entrypoint dell'applicazione e configurazione High-DPI
├── requirements.txt         # Dipendenze Python verificate
├── README.md                # Questo file
├── test_engine.py           # Suite di unit test automatizzati
│
├── engine/                  # Core e logica streaming/multimediale
│   ├── youtube.py           # Estrazione stream video/audio con yt-dlp
│   ├── video_stream.py      # Pipeline OpenCV, letterboxing e pyvirtualcam
│   ├── video_trimmer.py     # Taglio e compressione intelligente H.264/AAC via PyAV
│   ├── audio_stream.py      # Decodifica PyAV, sounddevice callback, gain & VU-meter
│   └── mouse_automation.py  # Worker OpenCV, hotkey globali, template matching e scroll
│
└── gui/                     # Interfaccia grafica Windows 11 Fluent Design
    ├── main_window.py       # Finestra principale con gestione tema dark
    ├── preview_widget.py    # Display video canvas con overlay LIVE e buffering
    ├── source_panel.py      # Gestione sorgenti (YouTube, File, Immagine) e scrubber
    ├── trim_dialog.py       # Dialog di taglio e compressione video
    ├── camera_panel.py      # Gestione webcam virtuale, risoluzioni e framerate
    ├── audio_panel.py       # Mixer audio con routing periferiche e VU-meter
    ├── mouse_panel.py       # Controlli automazione mouse, slider precisione e hotkey
    └── vu_meter.py          # Widget VU-meter stereo custom con Peak Hold
```

---

## 🧪 Esecuzione dei Test
Per verificare che l'intero stack multimediale sia perfettamente funzionante:
```powershell
python test_engine.py
```

---

## 📜 Licenza
Rilasciato sotto licenza MIT. Sentiti libero di utilizzare, modificare e distribuire questo software.
