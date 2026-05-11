use cosmic::app::{Core, Task};
use cosmic::iced::core::window;
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Rectangle};
use cosmic::prelude::*;
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget;
use cosmic::Element;
use futures_util::{SinkExt, StreamExt};

#[derive(Debug, Clone)]
pub enum AppMsg {
    PopupClosed(Id),
    ToggleRecording,
    Surface(cosmic::surface::Action),
    IpcConnected,
    IpcDisconnected,
    IpcState(DaemonState),
    IpcTranscript(String),
    ConfigReceived(DaemonConfig),
    MinAudioMsChanged(u64),
    VadThresholdChanged(f32),
    ToggleMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DaemonState {
    Idle,
    Recording,
    Transcribing,
    Streaming,
    ClipboardWritten,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub min_audio_ms: u64,
    pub vad_threshold: f32,
    pub chunk_interval_ms: u64,
    pub vad_min_silence_ms: u32,
    pub mode: String,
    pub auto_type: bool,
}

impl Default for DaemonState {
    fn default() -> Self {
        DaemonState::Idle
    }
}

pub struct AppModel {
    core: Core,
    popup: Option<Id>,
    state: DaemonState,
    connected: bool,
    last_transcript: String,
    config: Option<DaemonConfig>,
}

impl Default for AppModel {
    fn default() -> Self {
        Self {
            core: Core::default(),
            popup: None,
            state: DaemonState::Idle,
            connected: false,
            last_transcript: String::new(),
            config: None,
        }
    }
}

const SOCKET_PATH: &str = "/run/user/1000/ldsd.sock";
const APP_ID: &str = "com.byte6d65.lds.CosmicApplet";

impl cosmic::Application for AppModel {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = AppMsg;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<AppMsg>) {
        (AppModel { core, ..Default::default() }, Task::none())
    }

    fn on_close_requested(&self, id: window::Id) -> Option<AppMsg> {
        Some(AppMsg::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, AppMsg> {
        let icon_name = match &self.state {
            DaemonState::Idle | DaemonState::ClipboardWritten | DaemonState::Unknown => {
                "audio-input-microphone-symbolic"
            }
            DaemonState::Recording | DaemonState::Streaming => "lds-recording-symbolic",
            DaemonState::Transcribing => "content-loading-symbolic",
        };

        let have_popup = self.popup.clone();

        let btn = self
            .core
            .applet
            .icon_button(icon_name)
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = have_popup.clone() {
                    AppMsg::Surface(destroy_popup(id))
                } else {
                    AppMsg::Surface(app_popup::<AppModel>(
                        move |app: &mut AppModel| {
                            let new_id = Id::unique();
                            app.popup = Some(new_id);
                            let mut popup_settings = app.core.applet.get_popup_settings(
                                app.core.main_window_id().unwrap(),
                                new_id,
                                None,
                                None,
                                None,
                            );
                            popup_settings.positioner.anchor_rect = Rectangle {
                                x: (bounds.x - offset.x) as i32,
                                y: (bounds.y - offset.y) as i32,
                                width: bounds.width as i32,
                                height: bounds.height as i32,
                            };
                            popup_settings
                        },
                        Some(Box::new(move |app: &AppModel| {
                            let status_text = match &app.state {
                                DaemonState::Idle => "Idle",
                                DaemonState::Recording | DaemonState::Streaming => "● Recording...",
                                DaemonState::Transcribing => "Transcribing...",
                                DaemonState::ClipboardWritten => {
                                    if app.last_transcript.is_empty() {
                                        "✓ Copied"
                                    } else {
                                        "✓ Copied — see transcript"
                                    }
                                }
                                DaemonState::Unknown => "Unknown",
                            };

                            let toggle_label = match &app.state {
                                DaemonState::Idle
                                | DaemonState::ClipboardWritten
                                | DaemonState::Unknown => "Start Recording",
                                DaemonState::Recording | DaemonState::Streaming | DaemonState::Transcribing => {
                                    "Stop Recording"
                                }
                            };

                            let conn = if app.connected {
                                "● Connected"
                            } else {
                                "○ Offline"
                            };

                            let transcript_preview = if !app.last_transcript.is_empty() {
                                app.last_transcript.clone()
                            } else {
                                String::new()
                            };

                            let content = widget::list_column()
                                .add(widget::text::body(status_text))
                                .add(widget::text::caption(transcript_preview))
                                .add(
                                    widget::button::text(toggle_label)
                                        .on_press(AppMsg::ToggleRecording),
                                )
                                .add(widget::text::caption(conn));

                            // Config sliders (shown when connected)
                            let content = if let Some(ref cfg) = app.config {
                                let min_audio_f = cfg.min_audio_ms as f64;
                                let mode_label = if cfg.mode == "streaming" {
                                    "Mode: Streaming (click for Batch)"
                                } else {
                                    "Mode: Batch (click for Streaming)"
                                };
                                content
                                    .add(widget::text::body("── Settings ──"))
                                    .add(
                                        widget::button::text(mode_label)
                                            .on_press(AppMsg::ToggleMode),
                                    )
                                    .add(widget::text::caption(format!("Min audio: {}ms", cfg.min_audio_ms)))
                                    .add(
                                        widget::slider(
                                            100.0..=3000.0,
                                            min_audio_f,
                                            |v: f64| AppMsg::MinAudioMsChanged(v as u64),
                                        )
                                    )
                                    .add(widget::text::caption(format!("VAD threshold: {:.2}", cfg.vad_threshold)))
                                    .add(
                                        widget::slider(
                                            0.0..=1.0,
                                            cfg.vad_threshold as f64,
                                            |v: f64| AppMsg::VadThresholdChanged(v as f32),
                                        )
                                        .step(0.01)
                                    )
                            } else {
                                content
                            };

                            Element::from(app.core.applet.popup_container(content))
                                .map(cosmic::Action::App)
                        })),
                    ))
                }
            });

        let class = if !self.connected {
            cosmic::theme::Button::Destructive
        } else {
            cosmic::theme::Button::Standard
        };

        Element::from(
            self.core
                .applet
                .applet_tooltip::<AppMsg>(btn.class(class), "LDS", self.popup.is_some(), |a| {
                    AppMsg::Surface(a)
                }, None),
        )
    }

    fn view_window(&self, _id: Id) -> Element<'_, AppMsg> {
        "unused".into()
    }

    fn subscription(&self) -> cosmic::iced::Subscription<AppMsg> {
        struct IpcSub;
        cosmic::iced::Subscription::run_with(std::any::TypeId::of::<IpcSub>(), ipc_subscription)
    }

    fn update(&mut self, message: AppMsg) -> Task<AppMsg> {
        match message {
            AppMsg::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            AppMsg::ToggleRecording => {
                let state = self.state.clone();
                std::thread::spawn(move || {
                    toggle_recording_sync(&state);
                });
            }
            AppMsg::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(a),
                ));
            }
            AppMsg::IpcConnected => {
                self.connected = true;
            }
            AppMsg::IpcDisconnected => {
                self.connected = false;
            }
            AppMsg::IpcState(state) => {
                self.state = state;
            }
            AppMsg::IpcTranscript(text) => {
                self.last_transcript = text;
            }
            AppMsg::ConfigReceived(cfg) => {
                self.config = Some(cfg);
            }
            AppMsg::MinAudioMsChanged(ms) => {
                if let Some(ref mut cfg) = self.config {
                    cfg.min_audio_ms = ms;
                    let ms_val = ms;
                    std::thread::spawn(move || {
                        send_config_update("min_audio_ms", &serde_json::json!(ms_val));
                    });
                }
            }
            AppMsg::VadThresholdChanged(thresh) => {
                if let Some(ref mut cfg) = self.config {
                    cfg.vad_threshold = thresh;
                    let t = thresh;
                    std::thread::spawn(move || {
                        send_config_update("vad_threshold", &serde_json::json!(t));
                    });
                }
            }
            AppMsg::ToggleMode => {
                if let Some(ref mut cfg) = self.config {
                    let new_mode = if cfg.mode == "streaming" { "batch" } else { "streaming" };
                    cfg.mode = new_mode.to_string();
                    let m = new_mode.to_string();
                    std::thread::spawn(move || {
                        send_config_update("mode", &serde_json::json!(m));
                    });
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

fn ipc_subscription(
    _: &std::any::TypeId,
) -> std::pin::Pin<Box<dyn cosmic::iced::futures::Stream<Item = AppMsg> + Send + 'static>> {
    Box::pin(cosmic::iced::stream::channel(16, |mut tx: cosmic::iced::futures::channel::mpsc::Sender<AppMsg>| async move {
        loop {
            let stream = match tokio::net::UnixStream::connect(SOCKET_PATH).await {
                Ok(s) => s,
                Err(_) => {
                    let _ = tx.send(AppMsg::IpcDisconnected).await;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };

            let (mut ws_sender, mut ws_receiver) =
                match tokio_tungstenite::client_async("ws://localhost", stream).await {
                    Ok((ws, _)) => ws.split(),
                    Err(_) => {
                        let _ = tx.send(AppMsg::IpcDisconnected).await;
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    }
                };

            let _ = tx.send(AppMsg::IpcConnected).await;

            let req = serde_json::json!({"type": "status", "id": "applet"});
            let _ = ws_sender
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    req.to_string().into(),
                ))
                .await;

            // Also fetch current config
            let cfg_req = serde_json::json!({"type": "get_config", "id": "applet"});
            let _ = ws_sender
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    cfg_req.to_string().into(),
                ))
                .await;

            loop {
                match ws_receiver.next().await {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                            let t = parsed
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if t == "status" || t == "state" {
                                let state_val = if t == "status" {
                                    parsed.get("payload").and_then(|p| p.get("state"))
                                } else {
                                    parsed.get("payload")
                                };
                                if let Some(sv) = state_val {
                                    // Handle both string variants ("Recording") and
                                    // object variants ({"Streaming":{"partial_text":"..."}})
                                    let s = if sv.is_string() {
                                        match sv.as_str().unwrap_or("Idle") {
                                            "Recording" => DaemonState::Recording,
                                            "Transcribing" => DaemonState::Transcribing,
                                            "ClipboardWritten" => DaemonState::ClipboardWritten,
                                            _ => DaemonState::Idle,
                                        }
                                    } else if sv.is_object() {
                                        // Struct variants like Streaming
                                        if sv.get("Streaming").is_some() {
                                            DaemonState::Streaming
                                        } else if sv.get("Recording").is_some() {
                                            DaemonState::Recording
                                        } else if sv.get("Transcribing").is_some() {
                                            DaemonState::Transcribing
                                        } else {
                                            DaemonState::Idle
                                        }
                                    } else {
                                        DaemonState::Idle
                                    };
                                    let _ = tx.send(AppMsg::IpcState(s)).await;
                                }
                            } else if t == "final_transcript" {
                                if let Some(text) = parsed.get("payload").and_then(|p| p.get("text")).and_then(|v| v.as_str()) {
                                    let _ = tx.send(AppMsg::IpcTranscript(text.to_string())).await;
                                }
                            }
                            // Handle config response
                            if t == "config" {
                                if let Some(payload) = parsed.get("payload") {
                                    let cfg = DaemonConfig {
                                        min_audio_ms: payload.get("min_audio_ms").and_then(|v| v.as_u64()).unwrap_or(1500),
                                        vad_threshold: payload.get("vad_threshold").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
                                        chunk_interval_ms: payload.get("chunk_interval_ms").and_then(|v| v.as_u64()).unwrap_or(2000),
                                        vad_min_silence_ms: payload.get("vad_min_silence_ms").and_then(|v| v.as_u64()).unwrap_or(500) as u32,
                                        mode: payload.get("mode").and_then(|v| v.as_str()).unwrap_or("batch").to_string(),
                                        auto_type: payload.get("auto_type").and_then(|v| v.as_bool()).unwrap_or(true),
                                    };
                                    let _ = tx.send(AppMsg::ConfigReceived(cfg)).await;
                                }
                            }
                        }
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(data))) => {
                        let _ = ws_sender
                            .send(tokio_tungstenite::tungstenite::Message::Pong(data))
                            .await;
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) | None => {
                        let _ = tx.send(AppMsg::IpcDisconnected).await;
                        break;
                    }
                    _ => {}
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }))
}

fn toggle_recording_sync(state: &DaemonState) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok();
    let Some(rt) = rt else { return };

    rt.block_on(async {
        let stream = match tokio::net::UnixStream::connect(SOCKET_PATH).await {
            Ok(s) => s,
            Err(_) => return,
        };
        let (mut ws, _) = match tokio_tungstenite::client_async("ws://localhost", stream).await {
            Ok(r) => r,
            Err(_) => return,
        };

        let msg_type = match state {
            DaemonState::Idle | DaemonState::ClipboardWritten | DaemonState::Unknown => {
                "start_session"
            }
            DaemonState::Recording | DaemonState::Streaming | DaemonState::Transcribing => "stop_session",
        };

        let msg = serde_json::json!({"type": msg_type, "id": "applet-toggle"});
        let _ = ws
            .send(tokio_tungstenite::tungstenite::Message::Text(msg.to_string().into()))
            .await;
        let _ = ws.close(None).await;
    });
}

fn send_config_update(key: &str, value: &serde_json::Value) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok();
    let Some(rt) = rt else { return };

    let mut payload = serde_json::Map::new();
    payload.insert(key.to_string(), value.clone());

    rt.block_on(async {
        let stream = match tokio::net::UnixStream::connect(SOCKET_PATH).await {
            Ok(s) => s,
            Err(_) => return,
        };
        let (mut ws, _) = match tokio_tungstenite::client_async("ws://localhost", stream).await {
            Ok(r) => r,
            Err(_) => return,
        };

        let msg = serde_json::json!({
            "type": "update_config",
            "id": "applet-config",
            "payload": payload,
        });
        let _ = ws
            .send(tokio_tungstenite::tungstenite::Message::Text(msg.to_string().into()))
            .await;
        let _ = ws.close(None).await;
    });
}
