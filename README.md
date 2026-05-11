# LDS COSMIC Applet

Panel applet for [LDS](https://github.com/BYTE-6D65/lds) — the Linguistic Dispatch System. Provides mic toggle, status display, and live config tuning for the LDS speech-to-text daemon.

## Features

- **Mic toggle** — Click to start/stop recording
- **State display** — Shows Idle, Recording, Streaming, Transcribing status
- **Last transcript** — Preview of the most recent transcription
- **Mode toggle** — Switch between Streaming and Batch modes with one click
- **Settings sliders** — Min audio ms (100-3000), VAD threshold (0.00-1.00)
- **Live updates** — All changes apply immediately, no daemon restart

## Requirements

- COSMIC desktop environment
- [ldsd](https://github.com/BYTE-6D65/lds) running on `/run/user/1000/ldsd.sock`

## Build

```bash
cargo build --release
```

## Install

```bash
sudo cp target/release/lds-cosmic-applet /usr/bin/lds-cosmic-applet
```

Then add "LDS" to your COSMIC panel via Settings → Panel.

## Usage

1. Start `ldsd` (via systemd or CLI)
2. Click the mic icon in the panel to toggle recording
3. Click the mic icon again to open the popup for settings
4. Drag sliders to tune behavior in real-time
5. Click the mode button to swap between Streaming and Batch

## IPC Protocol

Communicates with ldsd over Unix domain WebSocket. Messages:

- `{"type": "status"}` — Query current state
- `{"type": "start_session"}` — Begin recording
- `{"type": "stop_session"}` — Stop recording
- `{"type": "get_config"}` — Fetch runtime config
- `{"type": "update_config", "min_audio_ms": 1000}` — Update config live

## Credits

- [libcosmic](https://github.com/pop-os/libcosmic) — COSMIC DE toolkit
- [cosmic-applet-template](https://github.com/pop-os/cosmic-applet-template) — Original template

## License

MIT
