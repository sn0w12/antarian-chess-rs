#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod components;
mod theme;

use chess_engine::Color;
use chess_server::protocol::{ClientMessage, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use gpui::{
    App, AppContext, AsyncApp, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, SharedString, Size, Styled, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Root, Sizable, TitleBar,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use gpui_component_assets::Assets;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::components::chess_board::{ChessBoard, GameMode};

const WS_URL: &str = "wss://chess.arathia.net/";

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
    Matchmaking,
    Playing,
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

    online_tx: Option<mpsc::UnboundedSender<String>>,

    name_input: Entity<InputState>,
}

impl ChessApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = load_settings();

        let focus_handle = cx.focus_handle();
        let board = cx.new(|cx| ChessBoard::new(window, cx));

        audio::set_volume(settings.volume);

        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Your name")
                .default_value(settings.player_name.as_str())
        });

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

        Self {
            focus_handle,
            view: View::Menu,
            board,
            volume_slider,
            _volume_subscription: volume_subscription,
            _name_subscription: name_subscription,
            online_tx: None,
            name_input,
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

    fn connect_online(&mut self, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().to_string();

        self.view = View::Matchmaking;
        cx.notify();

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
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
                                    this.view = View::Menu;
                                    cx.notify();
                                });
                            }
                        });
                        return;
                    }
                };

                let (mut write, mut read) = ws.split();

                let join = serde_json::to_string(&ClientMessage::Join { name }).unwrap();
                if write.send(Message::Text(join)).await.is_err() {
                    app.update(|app| {
                        if let Some(entity) = weak.upgrade() {
                            let _ = entity.update(app, |this, cx| {
                                this.online_tx = None;
                                this.view = View::Menu;
                                cx.notify();
                            });
                        }
                    });
                    return;
                }

                let write_task = tokio::spawn(async move {
                    while let Some(msg) = rx.recv().await {
                        if write.send(Message::Text(msg)).await.is_err() {
                            break;
                        }
                    }
                });

                while let Some(Ok(msg)) = read.next().await {
                    let text = match msg {
                        Message::Text(t) => t,
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
                            if this.view == View::Matchmaking {
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

    fn cancel_matchmaking(&mut self, cx: &mut Context<Self>) {
        if let Some(tx) = &self.online_tx {
            if let Ok(payload) = serde_json::to_string(&ClientMessage::LeaveMatchmaking) {
                let _ = tx.send(payload);
            }
        }
        self.online_tx = None;
        self.view = View::Menu;
        cx.notify();
    }

    fn handle_server_message(&mut self, msg: ServerMessage, cx: &mut Context<Self>) {
        match msg {
            ServerMessage::Joined { .. } => {
                if let Some(tx) = &self.online_tx {
                    if let Ok(payload) = serde_json::to_string(&ClientMessage::StartMatchmaking) {
                        let _ = tx.send(payload);
                    }
                }
            }
            ServerMessage::MatchmakingStarted => {}
            ServerMessage::MatchmakingLeft => {
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
                        _ => format!("Game over: {reason}"),
                    };
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
                self.board.update(cx, |board, cx| {
                    if board.game_mode == GameMode::Online {
                        board.push_system_chat_message(format!("Server error: {message}"));
                    } else {
                        board.status_message = format!("Server error: {message}");
                    }
                    cx.notify();
                });
            }
            _ => {}
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
            View::Matchmaking => matchmaking_view(self, cx),
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
                .items_center()
                .gap_2()
                .child(
                    Label::new("Antarian Chess")
                        .text_color(t.foreground)
                        .font_family(t.font_family.clone()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Input::new(&app.name_input).w(px(200.))),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child({
                            Button::new("bot-btn").large().label("vs Bot").on_click({
                                let weak = weak.clone();
                                move |_, _window, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.start_game(GameMode::Bot, Color::White, "Bot", cx);
                                    });
                                }
                            })
                        })
                        .child({
                            let weak = weak.clone();
                            Button::new("online-btn")
                                .large()
                                .label("Online")
                                .disabled(name_empty)
                                .on_click(move |_, _window, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.connect_online(cx);
                                    });
                                })
                        }),
                )
                .child(volume_bar(&app.volume_slider, vol, cx)),
        )
        .into_any_element()
}

// ===================== Matchmaking screen =====================

fn matchmaking_view(_app: &mut ChessApp, cx: &mut Context<ChessApp>) -> gpui::AnyElement {
    let weak = cx.weak_entity();

    v_flex()
        .items_center()
        .justify_center()
        .size_full()
        .child(
            v_flex()
                .items_center()
                .gap_2()
                .child(Label::new("Matchmaking...").text_color(cx.theme().foreground))
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
        .w_32()
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
        app.back_to_menu(cx);
    }
    h_flex()
        .size_full()
        .child(app.board.clone())
        .into_any_element()
}

// ===================== Entry point =====================

fn main() {
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
