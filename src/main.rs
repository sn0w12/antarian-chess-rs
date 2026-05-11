#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod components;
mod theme;
mod startup;
mod dialogs;

use machine_uid::get as machine_uid;
use chess_engine::Color;
use chess_server::protocol::{ClientInfo, ClientMessage, LobbySummary, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use gpui::{
    App, AppContext, AsyncApp, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, SharedString, Size, Styled, Subscription,
    TitlebarOptions, UniformListScrollHandle, Window, WindowBounds, WindowOptions, div, px,
    uniform_list,
};
use gpui_component::{
    ActiveTheme, Disableable, Root, Sizable, TitleBar,
    button::Button,
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    scroll::Scrollbar,
    separator::Separator,
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
    WindowExt,
};
use gpui_component_assets::Assets;
use serde::{Deserialize, Serialize};
use std::{ops::Range, path::Path};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

use crate::components::chess_board::{ChessBoard, GameMode};

const WS_URL: &str = "wss://chess.arathia.net/";
const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub(crate) enum WsCommand {
    Text(String),
    Close(Option<String>),
}

fn build_client_info() -> ClientInfo {
    ClientInfo {
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        machine_id: machine_uid().unwrap_or_else(|_| "unknown".to_string()),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        debug: cfg!(debug_assertions),
        app_name: env!("CARGO_PKG_NAME").to_string(),
        protocol_version: PROTOCOL_VERSION,
    }
}

// ---------------------------------------------------------------------------
// Settings persistence
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Settings {
    player_name: String,
    volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            player_name: String::new(),
            volume: 0.5,
        }
    }
}

fn settings_path() -> PathBuf {
    let base = if let Some(appdata) = std::env::var_os("APPDATA") {
        PathBuf::from(appdata).join("antarian-chess")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config").join("antarian-chess")
    } else {
        PathBuf::from(".").join("antarian-chess")
    };
    base.join("settings.json")
}

fn load_settings() -> Settings {
    let path = settings_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &Settings) {
    if let Ok(s) = serde_json::to_string_pretty(settings) {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, s);
    }
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum View {
    Menu,
    OnlineWaiting,
    LobbyBrowser,
    Playing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum OnlineIntent {
    RandomMatchmaking,
    BrowseLobbies,
    CreatePublicLobby,
    CreatePrivateLobby,
    JoinLobby(String),
}

// ---------------------------------------------------------------------------
// ChessApp (root)
// ---------------------------------------------------------------------------

struct ChessApp {
    focus_handle: FocusHandle,
    view: View,
    board: Entity<ChessBoard>,
    volume_slider: Entity<SliderState>,
    _volume_subscription: Subscription,
    _name_subscription: Subscription,

    online_tx: Option<mpsc::UnboundedSender<WsCommand>>,

    name_input: Entity<InputState>,
    lobby_code_input: Entity<InputState>,
    online_intent: Option<OnlineIntent>,
    online_status: String,
    active_lobby_code: Option<String>,
    public_lobbies: Vec<LobbySummary>,
    public_lobbies_scroll_handle: UniformListScrollHandle,
}

impl ChessApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let window_handle = window.window_handle();
        let settings = load_settings();

        let focus_handle = cx.focus_handle();
        let board = cx.new(|cx| ChessBoard::new(window, cx));

        audio::set_volume(settings.volume);

        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Your name")
                .default_value(settings.player_name.as_str())
        });
        let lobby_code_input = cx.new(|cx| InputState::new(window, cx).placeholder("Lobby code"));

        let volume_slider = cx.new(|_cx| {
            SliderState::new()
                .min(0.0)
                .max(1.0)
                .step(0.01)
                .default_value(settings.volume)
        });

        let name_subscription =
            cx.subscribe(&name_input, move |this, _input, _: &InputEvent, cx| {
                let name = this.name_input.read(cx).value().to_string();
                let vol = this.volume_slider.read(cx).value().start();
                save_settings(&Settings {
                    player_name: name,
                    volume: vol,
                });
            });

        let volume_subscription = cx.subscribe(
            &volume_slider,
            move |this, _slider, event: &SliderEvent, cx| {
                let SliderEvent::Change(value) = event;
                let vol = value.start();
                audio::set_volume(vol);
                let name = this.name_input.read(cx).value().to_string();
                save_settings(&Settings {
                    player_name: name,
                    volume: vol,
                });
            },
        );

        {
            cx.defer(move |cx| {
                if startup::install::should_prompt_for_install() {
                    let _ = cx.update_window(window_handle, |_, window, app| {
                        ChessApp::show_install_prompt(window, app);
                    });
                }
            });

            #[cfg(feature = "auto_update")]
            {
                let bg = cx.background_executor().clone();
                let update_check = bg.spawn(async move { startup::update::has_update() });
                cx.spawn(async move |_, async_app: &mut AsyncApp| {
                    let result = update_check.await;
                    if let Ok(version) = result {
                        async_app.update(|cx: &mut App| {
                            let _ = cx.update_window(window_handle, |_, window, app| {
                                ChessApp::show_update_prompt(window, app, version.clone());
                            });
                        });
                    }
                })
                .detach();
            }
        }

        Self {
            focus_handle,
            view: View::Menu,
            board,
            volume_slider,
            _volume_subscription: volume_subscription,
            _name_subscription: name_subscription,
            online_tx: None,
            name_input,
            lobby_code_input,
            online_intent: None,
            online_status: String::new(),
            active_lobby_code: None,
            public_lobbies: Vec::new(),
            public_lobbies_scroll_handle: UniformListScrollHandle::new(),
        }
    }

    fn start_game(&mut self, mode: GameMode, color: Color, opponent: &str, cx: &mut Context<Self>) {
        self.board.update(cx, |board, cx| {
            board.start_game(mode, color, opponent);
            board.start_timer(cx);
        });
        self.view = View::Playing;
        cx.notify();
    }

    fn connect_online(&mut self, intent: OnlineIntent, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().to_string();
        self.online_intent = Some(intent.clone());
        self.active_lobby_code = None;
        self.public_lobbies.clear();
        self.online_status = match &intent {
            OnlineIntent::RandomMatchmaking => "Connecting to matchmaking...".into(),
            OnlineIntent::BrowseLobbies => "Loading public lobbies...".into(),
            OnlineIntent::CreatePublicLobby => "Creating public lobby...".into(),
            OnlineIntent::CreatePrivateLobby => "Creating private lobby...".into(),
            OnlineIntent::JoinLobby(code) => format!("Joining lobby {}...", code.to_uppercase()),
        };

        self.view = match intent {
            OnlineIntent::BrowseLobbies => View::LobbyBrowser,
            _ => View::OnlineWaiting,
        };
        cx.notify();

        let client_info = build_client_info();
        let (tx, mut rx) = mpsc::unbounded_channel::<WsCommand>();
        self.online_tx = Some(tx);

        let weak = cx.weak_entity();
        cx.spawn(move |_: gpui::WeakEntity<Self>, async_app: &mut AsyncApp| {
            let app = async_app.clone();
            async move {
                let (ws, _) = match connect_async(WS_URL).await {
                    Ok(ws) => ws,
                    Err(error) => {
                        eprintln!("Failed to connect to server: {}", error);
                        app.update(|app| {
                            if let Some(entity) = weak.upgrade() {
                                let _ = entity.update(app, |this, cx| {
                                    this.online_tx = None;
                                    this.online_intent = None;
                                    this.active_lobby_code = None;
                                    this.public_lobbies.clear();
                                    this.online_status = "Failed to connect to server.".into();
                                    this.view = View::Menu;
                                    cx.notify();
                                });
                            }
                        });
                        return;
                    }
                };

                let (mut write, mut read) = ws.split();

                let join = serde_json::to_string(&ClientMessage::Join {
                    name,
                    client: client_info,
                })
                .unwrap();
                if write.send(Message::Text(join)).await.is_err() {
                    app.update(|app| {
                        if let Some(entity) = weak.upgrade() {
                            let _ = entity.update(app, |this, cx| {
                                this.online_tx = None;
                                this.online_intent = None;
                                this.active_lobby_code = None;
                                this.public_lobbies.clear();
                                this.online_status = "Failed to join server.".into();
                                this.view = View::Menu;
                                cx.notify();
                            });
                        }
                    });
                    return;
                }

                let write_task = tokio::spawn(async move {
                    let mut heartbeat = time::interval(Duration::from_secs(20));
                    heartbeat.tick().await;

                    loop {
                        tokio::select! {
                            maybe_msg = rx.recv() => {
                                let Some(msg) = maybe_msg else {
                                    break;
                                };
                                match msg {
                                    WsCommand::Text(text) => {
                                        if write.send(Message::Text(text)).await.is_err() {
                                            break;
                                        }
                                    }
                                    WsCommand::Close(reason) => {
                                        let frame = CloseFrame {
                                            code: CloseCode::Normal,
                                            reason: reason.unwrap_or_default().into(),
                                        };
                                        let _ = write.send(Message::Close(Some(frame))).await;
                                        break;
                                    }
                                }
                            }
                            _ = heartbeat.tick() => {
                                if write.send(Message::Ping(Vec::new())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });

                while let Some(Ok(msg)) = read.next().await {
                    let text = match msg {
                        Message::Text(t) => t,
                        Message::Ping(_) | Message::Pong(_) => continue,
                        Message::Close(_) => break,
                        _ => continue,
                    };

                    let server_msg: ServerMessage = match serde_json::from_str(&text) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    if weak.upgrade().is_none() {
                        break;
                    }
                    app.update(|app| {
                        if let Some(entity) = weak.upgrade() {
                            let _ = entity.update(app, |this, cx| {
                                this.handle_server_message(server_msg, cx);
                            });
                        }
                    });
                }

                app.update(|app| {
                    if let Some(entity) = weak.upgrade() {
                        let _ = entity.update(app, |this, cx| {
                            this.online_tx = None;
                            this.online_intent = None;
                            this.active_lobby_code = None;
                            this.public_lobbies.clear();
                            if matches!(this.view, View::OnlineWaiting | View::LobbyBrowser) {
                                this.view = View::Menu;
                            }
                            cx.notify();
                        });
                    }
                });

                write_task.await.unwrap_or(());
            }
        })
        .detach();
    }

    fn disconnect_online(&mut self, reason: impl Into<String>) {
        if let Some(tx) = self.online_tx.take() {
            let leave_msg = match self.online_intent.as_ref() {
                Some(OnlineIntent::RandomMatchmaking) => Some(ClientMessage::LeaveMatchmaking),
                Some(OnlineIntent::CreatePublicLobby | OnlineIntent::CreatePrivateLobby) => {
                    Some(ClientMessage::LeaveLobby)
                }
                _ => None,
            };
            if let Some(msg) = leave_msg
                && let Ok(payload) = serde_json::to_string(&msg)
            {
                let _ = tx.send(WsCommand::Text(payload));
            }
            if let Ok(payload) = serde_json::to_string(&ClientMessage::Disconnect {
                reason: Some(reason.into()),
            }) {
                let _ = tx.send(WsCommand::Text(payload));
            }
            let _ = tx.send(WsCommand::Close(Some("client_shutdown".to_string())));
        }
    }

    fn cancel_matchmaking(&mut self, cx: &mut Context<Self>) {
        self.disconnect_online("cancel_waiting");
        self.online_intent = None;
        self.active_lobby_code = None;
        self.public_lobbies.clear();
        self.online_status.clear();
        self.view = View::Menu;
        cx.notify();
    }

    fn join_lobby_from_current_session(&mut self, lobby_code: String, cx: &mut Context<Self>) {
        let normalized = lobby_code.trim().to_uppercase();
        if normalized.is_empty() {
            self.online_status = "Lobby code is required.".into();
            cx.notify();
            return;
        }

        if let Some(tx) = &self.online_tx
            && let Ok(payload) = serde_json::to_string(&ClientMessage::JoinLobby {
                lobby_code: normalized.clone(),
            })
        {
            let _ = tx.send(WsCommand::Text(payload));
            self.online_status = format!("Joining lobby {normalized}...");
            cx.notify();
            return;
        }

        self.connect_online(OnlineIntent::JoinLobby(normalized), cx);
    }

    fn apply_public_lobby_list(&mut self, lobbies: Vec<LobbySummary>, cx: &mut Context<Self>) {
        self.public_lobbies = lobbies;
        if matches!(self.online_intent, Some(OnlineIntent::BrowseLobbies))
            && self.active_lobby_code.is_none()
        {
            self.online_status = if self.public_lobbies.is_empty() {
                "No public lobbies yet.".into()
            } else {
                "Select a public lobby to join.".into()
            };
        }
        cx.notify();
    }

    fn handle_server_message(&mut self, msg: ServerMessage, cx: &mut Context<Self>) {
        match msg {
            ServerMessage::Joined { .. } => {
                if let (Some(tx), Some(intent)) = (&self.online_tx, &self.online_intent) {
                    let next = match intent {
                        OnlineIntent::RandomMatchmaking => ClientMessage::StartMatchmaking,
                        OnlineIntent::BrowseLobbies => ClientMessage::RequestLobbyList,
                        OnlineIntent::CreatePublicLobby => ClientMessage::CreateLobby {
                            private_lobby: false,
                        },
                        OnlineIntent::CreatePrivateLobby => ClientMessage::CreateLobby {
                            private_lobby: true,
                        },
                        OnlineIntent::JoinLobby(code) => ClientMessage::JoinLobby {
                            lobby_code: code.clone(),
                        },
                    };
                    if let Ok(payload) = serde_json::to_string(&next) {
                        let _ = tx.send(WsCommand::Text(payload));
                    }
                }
            }
            ServerMessage::MatchmakingStarted => {
                self.online_status = "Looking for a random opponent...".into();
                cx.notify();
            }
            ServerMessage::MatchmakingLeft => {
                self.online_intent = None;
                self.active_lobby_code = None;
                self.public_lobbies.clear();
                self.online_status.clear();
                self.view = View::Menu;
                cx.notify();
            }
            ServerMessage::LobbyList { lobbies } => self.apply_public_lobby_list(lobbies, cx),
            ServerMessage::LobbyListUpdated { lobbies } => {
                self.apply_public_lobby_list(lobbies, cx)
            }
            ServerMessage::LobbyCreated {
                lobby_code,
                private_lobby,
            } => {
                self.active_lobby_code = Some(lobby_code.clone());
                self.online_status = if private_lobby {
                    "Private lobby created. Share the code below.".into()
                } else {
                    "Public lobby created. Waiting for another player.".into()
                };
                cx.notify();
            }
            ServerMessage::LobbyLeft => {
                self.online_intent = None;
                self.active_lobby_code = None;
                self.public_lobbies.clear();
                self.online_status.clear();
                self.view = View::Menu;
                cx.notify();
            }
            ServerMessage::MatchFound {
                game_id,
                opponent_name,
                your_color,
            } => {
                let color = if your_color == "white" {
                    Color::White
                } else {
                    Color::Black
                };
                let tx = self.online_tx.clone();
                self.board.update(cx, |board, cx| {
                    board.game_id = Some(game_id);
                    board.start_game(GameMode::Online, color, &opponent_name);
                    board.online_tx = tx;
                    board.start_timer(cx);
                });
                self.active_lobby_code = None;
                self.online_intent = None;
                self.public_lobbies.clear();
                self.online_status.clear();
                self.view = View::Playing;
                cx.notify();
            }
            ServerMessage::OpponentMove { from, to, .. } => {
                let is_capture = self.board.read(cx).game_state.get(to as usize).is_some();
                let mv = chess_engine::Move::new(from, to, is_capture);
                self.board.update(cx, |board, cx| {
                    board.apply_opponent_move(mv, cx);
                });
            }
            ServerMessage::GameOver { reason, result, .. } => {
                self.board.update(cx, |board, cx| {
                    board.is_our_turn = false;
                    board.status_message = match result.as_deref() {
                        Some("win") => "You win!".into(),
                        Some("loss") => "You lose".into(),
                        Some("draw") => "Draw".into(),
                        _ => format!("Game over: {reason}"),
                    };
                    board.enable_rematch_offer();
                    cx.notify();
                });
            }
            ServerMessage::RematchRequested { game_id } => {
                self.board.update(cx, |board, cx| {
                    if board.game_id.as_deref() != Some(game_id.as_str()) {
                        return;
                    }
                    board.note_rematch_requested();
                    board.push_system_chat_message("Opponent requested a rematch.".to_string());
                    cx.notify();
                });
            }
            ServerMessage::RematchAccepted {
                old_game_id,
                new_game_id,
                your_color,
            } => {
                let color = if your_color == "white" {
                    Color::White
                } else {
                    Color::Black
                };
                let tx = self.online_tx.clone();
                self.board.update(cx, |board, cx| {
                    if board.game_id.as_deref() != Some(old_game_id.as_str()) {
                        return;
                    }
                    board.note_rematch_starting();
                    let opponent_name = board.opponent_name.clone();
                    board.game_id = Some(new_game_id);
                    board.start_game(GameMode::Online, color, &opponent_name);
                    board.online_tx = tx;
                    board.push_system_chat_message("Rematch started.".to_string());
                    board.start_timer(cx);
                });
                self.view = View::Playing;
                cx.notify();
            }
            ServerMessage::RematchDeclined { game_id } => {
                self.board.update(cx, |board, cx| {
                    if board.game_id.as_deref() != Some(game_id.as_str()) {
                        return;
                    }
                    board.note_rematch_declined();
                    board.push_system_chat_message("Rematch declined.".to_string());
                    cx.notify();
                });
            }
            ServerMessage::OpponentDisconnected { .. } => {
                self.board.update(cx, |board, cx| {
                    board.is_our_turn = false;
                    board.status_message = "Opponent disconnected".into();
                    cx.notify();
                });
            }
            ServerMessage::ChatMessage {
                game_id,
                sender_name,
                message,
            } => {
                self.board.update(cx, |board, cx| {
                    if board.game_id.as_deref() != Some(game_id.as_str()) {
                        return;
                    }
                    let sender_name = if sender_name == board.opponent_name {
                        sender_name
                    } else {
                        "You".to_string()
                    };
                    board.push_chat_message(sender_name, message);
                    cx.notify();
                });
            }
            ServerMessage::Error { message } => {
                if self.view == View::OnlineWaiting {
                    self.online_status = format!("Server error: {message}");
                    cx.notify();
                } else {
                    self.board.update(cx, |board, cx| {
                        if board.game_mode == GameMode::Online {
                            board.push_system_chat_message(format!("Server error: {message}"));
                        } else {
                            board.status_message = format!("Server error: {message}");
                        }
                        cx.notify();
                    });
                }
            }
        }
    }

    #[allow(dead_code)]
    fn back_to_menu(&mut self, cx: &mut Context<Self>) {
        self.view = View::Menu;
        cx.notify();
    }

    fn size() -> Size<Pixels> {
        Size {
            width: px(920.),
            height: px(640.),
        }
    }
}

impl Focusable for ChessApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl Render for ChessApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);

        let content: gpui::AnyElement = match self.view {
            View::Menu => menu_view(self, window, cx),
            View::OnlineWaiting => matchmaking_view(self, cx),
            View::LobbyBrowser => lobby_browser_view(self, cx),
            View::Playing => game_view(self, window, cx),
        };

        v_flex()
            .font_family(cx.theme().font_family.clone())
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .key_context("Root")
            .track_focus(&self.focus_handle)
            .child(TitleBar::new().border_t_1())
            .child(div().flex_1().child(content))
            .children(sheet_layer)
            .children(dialog_layer)
    }
}

// ===================== Menu screen =====================

fn menu_view(
    app: &mut ChessApp,
    _window: &mut Window,
    cx: &mut Context<ChessApp>,
) -> gpui::AnyElement {
    let t = cx.theme();
    let weak = cx.weak_entity();
    let vol = app.volume_slider.read(cx).value().start();
    let name = app.name_input.read(cx).value();
    let name_empty = name.trim().is_empty();

    h_flex()
        .items_center()
        .justify_center()
        .size_full()
        .child(
            v_flex()
                .w(px(440.))
                .gap_4()
                .p_4()
                .rounded_lg()
                .bg(t.muted.opacity(0.12))
                .child(
                    v_flex()
                        .w_full()
                        .gap_1()
                        .child(
                            Label::new("Antarian Chess")
                                .text_color(t.foreground)
                                .font_family(t.font_family.clone()),
                        )
                        .child(Label::new("Play local, queue randomly, or host your own match.").text_color(t.muted_foreground)),
                )
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(Label::new("Player Name").text_color(t.muted_foreground))
                        .child(Input::new(&app.name_input).w_full()),
                )
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(Label::new("Solo").text_color(t.muted_foreground))
                        .child({
                            Button::new("bot-btn").large().label("vs Bot").on_click({
                                let weak = weak.clone();
                                move |_, _window, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.start_game(GameMode::Bot, Color::White, "Bot", cx);
                                    });
                                }
                            })
                            .w_full()
                        }),
                )
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(Separator::horizontal().w_full())
                        .child(Label::new("Online").text_color(t.muted_foreground))
                        .child({
                            let weak = weak.clone();
                            Button::new("online-random-btn")
                                .large()
                                .label("Random Online")
                                .w_full()
                                .disabled(name_empty)
                                .on_click(move |_, _window, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.connect_online(OnlineIntent::RandomMatchmaking, cx);
                                    });
                                })
                        })
                        .child({
                            let weak = weak.clone();
                            Button::new("browse-lobbies-btn")
                                .label("Browse Lobbies")
                                .w_full()
                                .disabled(name_empty)
                                .on_click(move |_, _window, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.connect_online(OnlineIntent::BrowseLobbies, cx);
                                    });
                                })
                        })
                        .child({
                            h_flex()
                                .w_full()
                                .gap_2()
                                .child({
                                    let weak = weak.clone();
                                    Button::new("create-public-lobby-btn")
                                        .label("Create Public Lobby")
                                        .flex_1()
                                        .disabled(name_empty)
                                        .on_click(move |_, _window, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.connect_online(OnlineIntent::CreatePublicLobby, cx);
                                            });
                                        })
                                })
                                .child({
                                    let weak = weak.clone();
                                    Button::new("create-private-lobby-btn")
                                        .label("Create Private Lobby")
                                        .flex_1()
                                        .disabled(name_empty)
                                        .on_click(move |_, _window, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.connect_online(OnlineIntent::CreatePrivateLobby, cx);
                                            });
                                        })
                                })
                        }),
                )
                .child(volume_bar(&app.volume_slider, vol, cx)),
        )
        .into_any_element()
}

fn lobby_browser_view(app: &mut ChessApp, cx: &mut Context<ChessApp>) -> gpui::AnyElement {
    let weak = cx.weak_entity();
    let private_code_empty = app.lobby_code_input.read(cx).value().trim().is_empty();

    v_flex()
        .items_center()
        .justify_center()
        .size_full()
        .child(
            v_flex()
                .w(px(460.))
                .h(px(520.))
                .gap_3()
                .p_4()
                .rounded_lg()
                .bg(cx.theme().muted.opacity(0.12))
                .child(Label::new("Public Lobbies").text_color(cx.theme().foreground))
                .child(Label::new(app.online_status.clone()).text_color(cx.theme().muted_foreground))
                .child(
                    div()
                        .relative()
                        .w_full()
                        .flex_1()
                        .rounded_md()
                        .bg(cx.theme().muted.opacity(0.10))
                        .overflow_hidden()
                        .child(if app.public_lobbies.is_empty() {
                            v_flex()
                                .size_full()
                                .items_center()
                                .justify_center()
                                .child(
                                    Label::new("No public lobbies available right now.")
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .into_any_element()
                        } else {
                            uniform_list(
                                "public-lobby-list",
                                app.public_lobbies.len(),
                                cx.processor(|this, range: Range<usize>, _window, cx| {
                                    range
                                        .map(|ix| render_public_lobby_row(this, ix, cx))
                                        .collect::<Vec<_>>()
                                }),
                            )
                            .track_scroll(&app.public_lobbies_scroll_handle)
                            .size_full()
                            .into_any_element()
                        })
                        .child(Scrollbar::vertical(&app.public_lobbies_scroll_handle)),
                )
                .child(Separator::horizontal().w_full())
                .child(Label::new("Private Lobby Code").text_color(cx.theme().muted_foreground))
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .items_center()
                        .child(Input::new(&app.lobby_code_input).flex_1())
                        .child({
                            let join_weak = weak.clone();
                            Button::new("join-private-lobby-btn")
                                .label("Join Private")
                                .w(px(128.))
                                .disabled(private_code_empty)
                                .on_click(move |_, _window, cx| {
                                    let _ = join_weak.update(cx, |this, cx| {
                                        let code =
                                            this.lobby_code_input.read(cx).value().trim().to_string();
                                        this.join_lobby_from_current_session(code, cx);
                                    });
                                })
                        }),
                )
                .child(
                    Button::new("cancel-lobby-browser-btn")
                        .w_full()
                        .label("Back")
                        .on_click(move |_, _window, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.cancel_matchmaking(cx);
                            });
                        }),
                ),
        )
        .into_any_element()
}

fn render_public_lobby_row(
    app: &mut ChessApp,
    ix: usize,
    cx: &mut Context<ChessApp>,
) -> gpui::AnyElement {
    let Some(lobby) = app.public_lobbies.get(ix).cloned() else {
        return div().into_any_element();
    };

    let weak = cx.weak_entity();
    let code = lobby.lobby_code.clone();
    let lobby_code_label = lobby.lobby_code.clone();

    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap_2()
        .p_2()
        .rounded_md()
        .hover(|style| style.bg(cx.theme().muted.opacity(0.18)))
        .child(
            v_flex()
                .gap_1()
                .child(Label::new(lobby.host_name).text_color(cx.theme().foreground))
                .child(Label::new(lobby_code_label).text_color(cx.theme().muted_foreground)),
        )
        .child(
            Button::new(format!("join-public-{}", lobby.lobby_code))
                .label("Join")
                .on_click(move |_, _window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.join_lobby_from_current_session(code.clone(), cx);
                    });
                }),
        )
        .into_any_element()
}

// ===================== Matchmaking screen =====================

fn matchmaking_view(app: &mut ChessApp, cx: &mut Context<ChessApp>) -> gpui::AnyElement {
    let weak = cx.weak_entity();
    let title = match app.online_intent.as_ref() {
        Some(OnlineIntent::RandomMatchmaking) => "Random Matchmaking",
        Some(OnlineIntent::CreatePublicLobby) => "Public Lobby",
        Some(OnlineIntent::CreatePrivateLobby) => "Private Lobby",
        Some(OnlineIntent::BrowseLobbies) => "Public Lobby Browser",
        Some(OnlineIntent::JoinLobby(_)) => "Join Lobby",
        None => "Online",
    };
    let detail = app.online_status.clone();

    v_flex()
        .items_center()
        .justify_center()
        .size_full()
        .child(
            v_flex()
                .items_center()
                .w(px(320.))
                .gap_2()
                .text_center()
                .child(Label::new(title).text_color(cx.theme().foreground))
                .child(Label::new(detail).text_color(cx.theme().muted_foreground))
                .children(app.active_lobby_code.as_ref().map(|code| {
                    v_flex()
                        .w_full()
                        .gap_1()
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .child(Label::new(code.clone()).text_color(cx.theme().foreground))
                                .child(
                                    Clipboard::new("copy-lobby-code")
                                        .value(code.clone())
                                        .tooltip("Copy lobby code")
                                        .on_copied(|value, window, cx| {
                                            window.push_notification(
                                                format!("Lobby code copied: {}", value),
                                                cx,
                                            );
                                        }),
                                ),
                        )
                        .into_any_element()
                }))
                .child(Button::new("cancel-mm-btn").label("Cancel").on_click(
                    move |_, _window, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.cancel_matchmaking(cx);
                        });
                    },
                )),
        )
        .into_any_element()
}

fn volume_bar(slider: &Entity<SliderState>, vol: f32, cx: &Context<ChessApp>) -> gpui::AnyElement {
    let pct = (vol * 100.0).round() as u32;
    v_flex()
        .w_full()
        .items_center()
        .gap_2()
        .child(Label::new(format!("Volume: {pct}%")).text_color(cx.theme().muted_foreground))
        .child(Slider::new(slider))
        .into_any_element()
}

// ===================== Game screen =====================

fn game_view(
    app: &mut ChessApp,
    _window: &mut Window,
    cx: &mut Context<ChessApp>,
) -> gpui::AnyElement {
    if app.board.read(cx).leave_requested {
        app.board
            .update(cx, |board, _| board.leave_requested = false);
        if app.board.read(cx).game_mode == GameMode::Online {
            app.disconnect_online("leave_game");
        }
        app.back_to_menu(cx);
    }
    h_flex()
        .size_full()
        .child(app.board.clone())
        .into_any_element()
}

// ===================== Entry point =====================

fn main() {
    if let Some(original_path) = parse_delete_old_arg()
        && let Err(e) = delete_original_binary(&original_path)
    {
        eprintln!("Failed to delete old binary: {}", e);
    }

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let _guard = rt.enter();

    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx: &mut App| {
        gpui_component::init(cx);
        theme::init(cx);

        cx.activate(true);
        if let Err(error) = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("Antarian Chess")),
                    ..TitleBar::title_bar_options()
                }),
                app_id: Some("com.sn0w12.antarian_chess".to_string()),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    ChessApp::size(),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| ChessApp::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            eprintln!("Failed to open window: {error}");
        }
    });

    drop(_guard);
    rt.shutdown_timeout(Duration::from_secs(2));
    std::process::exit(0);
}

/// Parse `--delete-old="..."` from command line and return the path.
fn parse_delete_old_arg() -> Option<PathBuf> {
    for arg in std::env::args() {
        if let Some(stripped) = arg.strip_prefix("--delete-old=\"") {
            if let Some(path) = stripped.strip_suffix('"') {
                return Some(PathBuf::from(path));
            }
        } else if let Some(stripped) = arg.strip_prefix("--delete-old=") {
            // Without quotes (unlikely, but handle)
            return Some(PathBuf::from(stripped));
        }
    }
    None
}

/// Delete the original binary file.
/// On Windows, retry a few times because the parent process might still hold a handle.
/// On Unix, just remove it.
fn delete_original_binary(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    // On Windows, the original process may have just exited; give it a moment.
    #[cfg(windows)]
    {
        for attempt in 0..5 {
            if std::fs::remove_file(path).is_ok() {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100 * attempt));
        }
        anyhow::bail!("Failed to delete {} after 5 attempts", path.display());
    }
    #[cfg(not(windows))]
    {
        std::fs::remove_file(path)?;
        Ok(())
    }
}
