use crate::audio;
use chess_bot::find_best_move;
use chess_engine::*;
use chess_server::protocol::ClientMessage;
use gpui::{
    AnyElement, AppContext, AsyncApp, AvailableSpace, Context, Entity, Hsla, Image, ImageFormat,
    IntoElement, ObjectFit, ParentElement, Pixels, Render, ScrollStrategy, SharedString, Size,
    Styled, StyledImage, WeakEntity, Window, div, hsla, img, prelude::FluentBuilder, px, size,
};
use gpui_component::{
    ActiveTheme, VirtualListScrollHandle,
    button::Button,
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::Scrollbar,
    separator::Separator,
    v_flex, v_virtual_list,
};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::mpsc;

/// How much time the bot spends thinking per move.
fn think_time(board: &Board, white_time: f64, black_time: f64, move_count: u32) -> Duration {
    let remaining = if board.turn == Color::White {
        white_time
    } else {
        black_time
    };

    // Estimated moves remaining — more time per move early on, less later.
    let expected = (100u32.saturating_sub(move_count)).max(20) as f64;
    let per_move = remaining / expected;

    // Position complexity: more legal moves = more complex = more time needed.
    let pseudolegal = board.generate_all_moves(board.turn).len().max(1) as f64;
    let complexity = (pseudolegal / 30.0).clamp(0.5, 2.0);

    // Never use more than 33 % of remaining time on one move.
    let max_time = remaining * 0.33;
    // Spend at least 0.5 s.
    let mut base = (per_move * complexity).clamp(0.5, max_time.max(0.5));

    // Early-game bonus: invest more time in the opening and early middlegame
    // when the position is still complex and plans are being formed.
    if move_count < 20 {
        base = (base * 1.5).min(max_time.max(0.5));
    }

    Duration::from_secs_f64(base)
}
const INITIAL_TIME: f64 = 600.0; // 10 minutes per player
const CHAT_FONT_SIZE: Pixels = px(16.);
const CHAT_LINE_HEIGHT: Pixels = px(20.);
const ROOT_GAP: Pixels = px(8.);
const SIDEBAR_HORIZONTAL_PADDING: Pixels = px(16.);
const CHAT_MIN_WRAP_WIDTH: Pixels = px(80.);

// ---------------------------------------------------------------------------
// Piece image cache
// ---------------------------------------------------------------------------

fn piece_image(piece: PieceKind, color: Color) -> Arc<Image> {
    static PIECE_IMAGES: OnceLock<[[Arc<Image>; 2]; 6]> = OnceLock::new();
    let images = PIECE_IMAGES.get_or_init(|| {
        fn load(data: &[u8]) -> Arc<Image> {
            Arc::new(Image::from_bytes(ImageFormat::Png, data.to_vec()))
        }
        [
            [
                load(include_bytes!("../assets/pieces/dragon_white.png")),
                load(include_bytes!("../assets/pieces/dragon_black.png")),
            ],
            [
                load(include_bytes!("../assets/pieces/emperor_white.png")),
                load(include_bytes!("../assets/pieces/emperor_black.png")),
            ],
            [
                load(include_bytes!("../assets/pieces/empress_white.png")),
                load(include_bytes!("../assets/pieces/empress_black.png")),
            ],
            [
                load(include_bytes!("../assets/pieces/knight_white.png")),
                load(include_bytes!("../assets/pieces/knight_black.png")),
            ],
            [
                load(include_bytes!("../assets/pieces/paladin_white.png")),
                load(include_bytes!("../assets/pieces/paladin_black.png")),
            ],
            [
                load(include_bytes!("../assets/pieces/priest_white.png")),
                load(include_bytes!("../assets/pieces/priest_black.png")),
            ],
        ]
    });

    let idx = match piece {
        PieceKind::Dragon => 0,
        PieceKind::Emperor => 1,
        PieceKind::Empress => 2,
        PieceKind::Knight => 3,
        PieceKind::Paladin => 4,
        PieceKind::Priest => 5,
    };
    let ci = match color {
        Color::White => 0,
        Color::Black => 1,
    };
    Arc::clone(&images[idx][ci])
}

// ---------------------------------------------------------------------------
// Game mode
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum GameMode {
    Bot,
    Online,
    Local,
}

#[derive(Clone, Debug)]
struct ChatEntry {
    sender_name: String,
    message: String,
    is_ours: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RematchState {
    Hidden,
    Available,
    Requesting,
    RequestedByOpponent,
    Starting,
    Declined,
}

// ---------------------------------------------------------------------------
// ChessBoard entity
// ---------------------------------------------------------------------------

pub struct ChessBoard {
    pub game_state: Board,
    pub player_color: Color,
    selected_square: Option<usize>,
    legal_targets: Vec<u8>,

    pub game_mode: GameMode,
    pub is_our_turn: bool,
    pub status_message: String,
    move_count: u32,
    captured_us: Vec<PieceKind>,
    captured_opp: Vec<PieceKind>,

    pub white_time: f64,
    pub black_time: f64,
    time_forfeit: bool,

    pub game_id: Option<String>,
    pub opponent_name: String,
    pub online_tx: Option<mpsc::UnboundedSender<String>>,
    pub leave_requested: bool,
    rematch_state: RematchState,
    timer_generation: u64,

    pub bot_score: Option<i32>,
    pub bot_depth: Option<u32>,

    last_move: Option<Move>,
    chat_messages: Vec<ChatEntry>,
    chat_input: Entity<InputState>,
    chat_scroll_handle: VirtualListScrollHandle,
    chat_item_sizes: Rc<Vec<Size<Pixels>>>,
    chat_list_width: Pixels,
    pending_chat_scroll: bool,
}

impl ChessBoard {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_input = cx.new(|cx| InputState::new(window, cx).placeholder("Message opponent"));

        Self {
            game_state: Board::initial(),
            player_color: Color::White,
            selected_square: None,
            legal_targets: Vec::new(),
            game_mode: GameMode::Local,
            is_our_turn: true,
            status_message: "White to move".into(),
            move_count: 0,
            captured_us: Vec::new(),
            captured_opp: Vec::new(),
            white_time: INITIAL_TIME,
            black_time: INITIAL_TIME,
            time_forfeit: false,
            game_id: None,
            opponent_name: String::new(),
            online_tx: None,
            leave_requested: false,
            rematch_state: RematchState::Hidden,
            timer_generation: 0,
            bot_score: None,
            bot_depth: None,
            last_move: None,
            chat_messages: Vec::new(),
            chat_input,
            chat_scroll_handle: VirtualListScrollHandle::new(),
            chat_item_sizes: Rc::new(Vec::new()),
            chat_list_width: px(0.),
            pending_chat_scroll: false,
        }
    }

    pub fn start_game(&mut self, mode: GameMode, color: Color, opponent: &str) {
        audio::play_game_start();
        self.game_state = Board::initial();
        self.player_color = color;
        self.game_mode = mode;
        self.is_our_turn = color == Color::White;
        self.selected_square = None;
        self.legal_targets.clear();
        self.move_count = 0;
        self.captured_us.clear();
        self.captured_opp.clear();
        self.opponent_name = opponent.to_string();
        self.white_time = INITIAL_TIME;
        self.black_time = INITIAL_TIME;
        self.time_forfeit = false;
        self.timer_generation = self.timer_generation.wrapping_add(1);
        self.online_tx = None;
        self.leave_requested = false;
        self.rematch_state = RematchState::Hidden;
        self.bot_score = None;
        self.bot_depth = None;
        self.last_move = None;
        self.chat_messages.clear();
        self.chat_item_sizes = Rc::new(Vec::new());
        self.chat_list_width = px(0.);
        self.pending_chat_scroll = false;
        self.refresh_status();
    }

    /// Start the per‑player countdown clock. Call once after `start_game`.
    pub fn start_timer(&mut self, cx: &mut Context<Self>) {
        let generation = self.timer_generation;
        let weak = cx.weak_entity();
        let bg = cx.background_executor().clone();
        cx.spawn(move |_: WeakEntity<ChessBoard>, async_app: &mut AsyncApp| {
            let bg = bg.clone();
            let app = async_app.clone();
            async move {
                let tick = Duration::from_millis(100);
                loop {
                    bg.timer(tick).await;
                    let entity = match weak.upgrade() {
                        Some(e) => e,
                        None => break,
                    };
                    let keep_running = app.update(|app| {
                        entity
                            .update(app, |this, cx| {
                                if this.timer_generation != generation {
                                    return false;
                                }
                                this.tick(cx);
                                true
                            })
                    });
                    if !keep_running {
                        break;
                    }
                }
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.is_game_over() || self.time_forfeit {
            return;
        }
        let dt = 0.1;
        if self.game_state.turn == Color::White {
            self.white_time -= dt;
            if self.white_time <= 0.0 {
                self.white_time = 0.0;
                self.time_forfeit = true;
                self.status_message = if self.player_color == Color::White {
                    "Time forfeit — you lose".into()
                } else {
                    "Time forfeit — you win!".into()
                };
                self.is_our_turn = false;
            }
        } else {
            self.black_time -= dt;
            if self.black_time <= 0.0 {
                self.black_time = 0.0;
                self.time_forfeit = true;
                self.status_message = if self.player_color == Color::Black {
                    "Time forfeit — you lose".into()
                } else {
                    "Time forfeit — you win!".into()
                };
                self.is_our_turn = false;
            }
        }
        cx.notify();
    }

    fn refresh_status(&mut self) {
        match self.game_state.game_result() {
            GameResult::Checkmate { winner } => {
                self.status_message = if winner == self.player_color {
                    "Checkmate — you win!".into()
                } else {
                    "Checkmate — you lose".into()
                };
                self.is_our_turn = false;
                return;
            }
            GameResult::Stalemate => {
                self.status_message = "Stalemate — draw".into();
                self.is_our_turn = false;
                return;
            }
            GameResult::Ongoing => {}
        }

        let in_check = self.game_state.is_in_check(self.game_state.turn);
        let turn = if self.game_state.turn == Color::White {
            "White"
        } else {
            "Black"
        };
        let check_suffix = if in_check { " — check!" } else { "" };

        if self.game_state.turn == self.player_color {
            self.status_message = format!("Your turn ({turn}){check_suffix}");
            self.is_our_turn = true;
        } else {
            self.status_message = format!("{turn} to move{check_suffix}");
            self.is_our_turn = false;
        }
    }

    fn is_game_over(&self) -> bool {
        self.game_state.game_result() != GameResult::Ongoing
    }

    fn can_offer_rematch(&self) -> bool {
        self.game_mode == GameMode::Online
            && self.game_id.is_some()
            && (self.is_game_over() || self.time_forfeit)
    }

    pub fn enable_rematch_offer(&mut self) {
        if self.can_offer_rematch() {
            self.rematch_state = RematchState::Available;
        }
    }

    pub fn note_rematch_requested(&mut self) {
        if self.can_offer_rematch() {
            self.rematch_state = RematchState::RequestedByOpponent;
        }
    }

    pub fn note_rematch_declined(&mut self) {
        self.rematch_state = RematchState::Declined;
    }

    pub fn note_rematch_starting(&mut self) {
        self.rematch_state = RematchState::Starting;
    }

    fn send_online_message(&self, msg: &ClientMessage) {
        let Some(tx) = &self.online_tx else {
            return;
        };
        if let Ok(payload) = serde_json::to_string(msg) {
            let _ = tx.send(payload);
        }
    }

    fn request_rematch(&mut self, cx: &mut Context<Self>) {
        let Some(game_id) = self.game_id.clone() else {
            return;
        };
        self.send_online_message(&ClientMessage::RequestRematch { game_id });
        self.rematch_state = RematchState::Requesting;
        cx.notify();
    }

    fn accept_rematch(&mut self, cx: &mut Context<Self>) {
        let Some(game_id) = self.game_id.clone() else {
            return;
        };
        self.send_online_message(&ClientMessage::AcceptRematch { game_id });
        self.rematch_state = RematchState::Starting;
        cx.notify();
    }

    fn decline_rematch(&mut self, cx: &mut Context<Self>) {
        let Some(game_id) = self.game_id.clone() else {
            return;
        };
        self.send_online_message(&ClientMessage::DeclineRematch { game_id });
        self.rematch_state = RematchState::Declined;
        cx.notify();
    }

    pub fn handle_square_click(&mut self, square: usize, cx: &mut Context<Self>) {
        if !self.is_our_turn || self.is_game_over() {
            return;
        }

        if let Some(selected) = self.selected_square
            && self.legal_targets.contains(&(square as u8))
        {
            let capture = self.game_state.get(square).is_some();
            let mv = Move::new(selected as u8, square as u8, capture);
            self.exec_move(mv, cx);
            return;
        }

        if let Some((color, _)) = self.game_state.get(square)
            && color == self.player_color
        {
            self.selected_square = Some(square);
            self.legal_targets = self
                .game_state
                .generate_moves_for(square, false)
                .into_iter()
                .filter(|mv| {
                    let child = self.game_state.make_move(mv);
                    !child.is_in_check(self.player_color)
                })
                .map(|mv| mv.to)
                .collect();
            cx.notify();
            return;
        }

        self.selected_square = None;
        self.legal_targets.clear();
        cx.notify();
    }

    fn exec_move(&mut self, mv: Move, cx: &mut Context<Self>) {
        let was_capture = mv.capture;
        if was_capture && let Some((_, kind)) = self.game_state.get(mv.to as usize) {
            self.captured_opp.push(kind);
        }
        self.last_move = Some(mv);
        self.game_state = self.game_state.make_move(&mv);
        self.selected_square = None;
        self.legal_targets.clear();
        self.move_count += 1;
        self.refresh_status();
        self.play_sound(was_capture);

        if self.game_mode == GameMode::Online {
            if let Some(ref tx) = self.online_tx {
                let gid = self.game_id.clone().unwrap_or_default();
                let msg = ClientMessage::MakeMove {
                    game_id: gid,
                    from: mv.from,
                    to: mv.to,
                };
                if let Ok(payload) = serde_json::to_string(&msg) {
                    let _ = tx.send(payload);
                }
            }
        }

        cx.notify();

        if self.game_mode == GameMode::Bot && !self.is_game_over() {
            let time = think_time(
                &self.game_state,
                self.white_time,
                self.black_time,
                self.move_count,
            );
            Self::schedule_bot(&self.game_state, cx, time);
        }
    }

    pub fn apply_opponent_move(&mut self, mv: Move, cx: &mut Context<Self>) {
        let was_capture = mv.capture;
        if was_capture && let Some((_, kind)) = self.game_state.get(mv.to as usize) {
            self.captured_us.push(kind);
        }
        self.last_move = Some(mv);
        self.game_state = self.game_state.make_move(&mv);
        self.move_count += 1;
        self.refresh_status();
        self.play_sound(was_capture);
        cx.notify();
    }

    fn play_sound(&self, was_capture: bool) {
        match self.game_state.game_result() {
            GameResult::Ongoing => {
                if self.game_state.is_in_check(self.game_state.turn) {
                    audio::play_check();
                } else if was_capture {
                    audio::play_capture();
                } else {
                    audio::play_move();
                }
            }
            _ => audio::play_game_end(),
        }
    }

    fn schedule_bot(board: &Board, cx: &Context<Self>, think_duration: Duration) {
        let board = board.clone();
        let task = cx
            .background_executor()
            .spawn(async move { find_best_move(&board, think_duration) });

        cx.spawn(|weak: WeakEntity<ChessBoard>, async_app: &mut AsyncApp| {
            let app = async_app.clone();
            async move {
                if let Some(result) = task.await
                    && let Some(entity) = weak.upgrade()
                {
                    let _ = app.update(|app| {
                        let _ = entity.update(app, |this, cx| {
                            this.exec_move_async(result, cx);
                        });
                    });
                }
            }
        })
        .detach();
    }

    fn exec_move_async(&mut self, result: chess_bot::SearchResult, cx: &mut Context<Self>) {
        let mv = result.best_move;
        let was_capture = mv.capture;
        if was_capture && let Some((_, kind)) = self.game_state.get(mv.to as usize) {
            self.captured_us.push(kind);
        }
        self.last_move = Some(mv);
        self.bot_score = Some(result.score);
        self.bot_depth = Some(result.depth);
        self.game_state = self.game_state.make_move(&mv);
        self.selected_square = None;
        self.legal_targets.clear();
        self.move_count += 1;
        self.refresh_status();
        self.play_sound(was_capture);
        cx.notify();
    }

    pub fn push_chat_message(&mut self, sender_name: String, message: String) {
        let is_ours = sender_name == "You";
        self.chat_messages.push(ChatEntry {
            sender_name,
            message,
            is_ours,
        });
        self.pending_chat_scroll = true;
    }

    pub fn push_system_chat_message(&mut self, message: String) {
        self.chat_messages.push(ChatEntry {
            sender_name: "System".to_string(),
            message,
            is_ours: false,
        });
        self.pending_chat_scroll = true;
    }

    pub fn send_chat_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.game_mode != GameMode::Online {
            return;
        }

        let Some(tx) = &self.online_tx else {
            return;
        };
        let Some(game_id) = &self.game_id else {
            return;
        };

        let message = self.chat_input.read(cx).value().trim().to_string();
        if message.is_empty() {
            return;
        }

        let msg = ClientMessage::SendChat {
            game_id: game_id.clone(),
            message,
        };
        if let Ok(payload) = serde_json::to_string(&msg) {
            let _ = tx.send(payload);
            self.chat_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
        }
    }

    fn refresh_chat_item_sizes(
        &mut self,
        list_width: Pixels,
        font_family: SharedString,
        foreground: Hsla,
        primary: Hsla,
        muted: Hsla,
        muted_foreground: Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chat_list_width = list_width;
        self.chat_item_sizes = Rc::new(
            self.chat_messages
                .iter()
                .map(|entry| {
                    chat_item_size(
                        entry,
                        list_width,
                        &font_family,
                        foreground,
                        primary,
                        muted,
                        muted_foreground,
                        window,
                        cx,
                    )
                })
                .collect(),
        );
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl Render for ChessBoard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list_width = chat_list_width(window, &self.chat_scroll_handle);
        let font_family = cx.theme().font_family.clone();
        let t = cx.theme();
        if list_width != self.chat_list_width
            || self.chat_item_sizes.len() != self.chat_messages.len()
        {
            self.refresh_chat_item_sizes(
                list_width,
                font_family.clone(),
                t.foreground,
                t.primary,
                t.muted,
                t.muted_foreground,
                window,
                cx,
            );
        }
        if self.pending_chat_scroll {
            let last_ix = self.chat_messages.len().saturating_sub(1);
            self.chat_scroll_handle
                .scroll_to_item(last_ix, ScrollStrategy::Bottom);
            self.pending_chat_scroll = false;
        }

        let flip = self.player_color == Color::White;
        h_flex()
            .size_full()
            .gap_2()
            .child(board_view(self, flip, cx))
            .child(sidebar(self, window, cx))
    }
}

// ---- Square colours (always these; they're part of the board aesthetic) ----

fn lsq() -> Hsla {
    hsla(0.10, 0.12, 0.82, 1.0)
}
fn dsq() -> Hsla {
    hsla(0.12, 0.24, 0.62, 1.0)
}
fn sel() -> Hsla {
    hsla(0.12, 0.80, 0.50, 1.0)
}
fn dot() -> Hsla {
    hsla(0.30, 0.60, 0.55, 0.6)
}
fn leg() -> Hsla {
    hsla(0.30, 0.60, 0.55, 0.35)
}
fn lm() -> Hsla {
    hsla(0.12, 0.50, 0.70, 1.0)
}

fn board_view(this: &ChessBoard, flip: bool, cx: &Context<ChessBoard>) -> AnyElement {
    let sqs = this.game_state.squares();

    div()
        .h_full()
        .aspect_ratio(1.0)
        .child(
            div()
                .size_full()
                .grid()
                .grid_cols(8)
                .grid_rows(8)
                .gap_0()
                .children(sqs.iter().enumerate().map(|(i, _)| {
                    let bi = if flip { 63 - i } else { i };
                    let piece = sqs[bi].map(|(c, p)| piece_image(p, c));
                    let mut bg = if (i / 8 + i % 8) % 2 == 0 {
                        lsq()
                    } else {
                        dsq()
                    };
                    if let Some(last) = this.last_move {
                        if bi == last.from as usize || bi == last.to as usize {
                            bg = lm();
                        }
                    }
                    let selected = this.selected_square == Some(bi);
                    let target = this.legal_targets.contains(&(bi as u8));
                    let weak = cx.weak_entity();
                    let sq = bi;

                    Button::new(format!("s{sq}"))
                        .p_0()
                        .rounded_none()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if target && piece.is_none() { leg() } else { bg })
                        .border_2()
                        .border_color(if selected { sel() } else { bg })
                        .on_click(move |_, _window, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.handle_square_click(sq, cx);
                            });
                        })
                        .child(if let Some(im) = piece {
                            if target {
                                div()
                                    .size_full()
                                    .bg(leg())
                                    .border_2()
                                    .border_color(dot())
                                    .child(img(im).size_full().object_fit(ObjectFit::Contain))
                            } else {
                                div()
                                    .size_full()
                                    .child(img(im).size_full().object_fit(ObjectFit::Contain))
                            }
                        } else if target {
                            div().size(px(20.)).rounded_full().bg(dot())
                        } else {
                            div()
                        })
                })),
        )
        .into_any_element()
}

fn sidebar(this: &ChessBoard, _window: &mut Window, cx: &mut Context<ChessBoard>) -> AnyElement {
    let t = cx.theme();
    let mode = match this.game_mode {
        GameMode::Bot => "vs Bot",
        GameMode::Online => "Online",
        GameMode::Local => "Local",
    };
    let you = if this.player_color == Color::White {
        "White"
    } else {
        "Black"
    };
    let white_time = fmt_time(this.white_time);
    let black_time = fmt_time(this.black_time);

    v_flex()
        .flex_1()
        .h_full()
        .p_2()
        .gap_2()
        .child(detail(t, "Mode", mode))
        .child(detail(t, "You", you))
        .child(detail(t, "Opponent", &this.opponent_name))
        .child(detail(t, "Moves", &this.move_count.to_string()))
        .child(Separator::horizontal().w_full())
        .child(
            div()
                .p_2()
                .rounded_md()
                .bg(if this.is_our_turn {
                    t.primary.opacity(0.25)
                } else {
                    t.muted.opacity(0.4)
                })
                .text_color(t.foreground)
                .child(this.status_message.clone()),
        )
        .children(this.can_offer_rematch().then(|| rematch_panel(this, cx)))
        .child(Separator::horizontal().w_full())
        .children((this.game_mode == GameMode::Bot).then(|| {
            let score_str = this.bot_score.map(fmt_score).unwrap_or_default();
            let depth_str = this.bot_depth.map(|d| format!("{d}")).unwrap_or_default();
            v_flex()
                .w_full()
                .px_2()
                .gap_1()
                .child(detail(t, "Bot Eval", &score_str))
                .child(detail(t, "Search Depth", &depth_str))
        }))
        .child(Separator::horizontal().w_full())
        .child(clock_row("White", &white_time, Color::White, this, t))
        .child(clock_row("Black", &black_time, Color::Black, this, t))
        .when(this.game_mode == GameMode::Online, |e| {
            e.child(Separator::horizontal().w_full())
        })
        .children((this.game_mode == GameMode::Online).then(|| {
            v_flex()
                .w_full()
                .flex_1()
                .gap_2()
                .child(Label::new("Match Chat").text_color(t.muted_foreground))
                .child(if this.chat_messages.is_empty() {
                    div()
                        .flex_1()
                        .max_h(px(200.))
                        .p_2()
                        .rounded_md()
                        .bg(t.muted.opacity(0.15))
                        .child(Label::new("No messages yet.").text_color(t.muted_foreground))
                        .into_any_element()
                } else {
                    div()
                        .relative()
                        .flex_1()
                        .rounded_md()
                        .bg(t.muted.opacity(0.15))
                        .overflow_hidden()
                        .child(
                            v_virtual_list(
                                cx.entity().clone(),
                                "match-chat-list",
                                this.chat_item_sizes.clone(),
                                |board, visible_range, _, cx| {
                                    let font_family = cx.theme().font_family.clone();
                                    let foreground = cx.theme().foreground;
                                    let primary = cx.theme().primary;
                                    let muted = cx.theme().muted;
                                    let muted_foreground = cx.theme().muted_foreground;
                                    let list_width = board.chat_list_width;
                                    visible_range
                                        .map(|ix| {
                                            let entry = board.chat_messages[ix].clone();
                                            chat_row_element(
                                                &entry,
                                                list_width,
                                                &font_family,
                                                foreground,
                                                primary,
                                                muted,
                                                muted_foreground,
                                            )
                                            .into_any_element()
                                        })
                                        .collect()
                                },
                            )
                            .track_scroll(&this.chat_scroll_handle)
                            .flex_1(),
                        )
                        .child(Scrollbar::vertical(&this.chat_scroll_handle))
                        .into_any_element()
                })
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .items_center()
                        .child(Input::new(&this.chat_input).flex_1())
                        .child(Button::new("send-chat-btn").label("Send").on_click({
                            let weak = cx.weak_entity();
                            move |_, window, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.send_chat_message(window, cx);
                                });
                            }
                        })),
                )
        }))
        .child(Separator::horizontal().w_full())
        .child(Button::new("leave-btn").w_full().label("Leave").on_click({
            let weak = cx.weak_entity();
            move |_, _window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.leave_requested = true;
                    cx.notify();
                });
            }
        }))
        .child(Separator::horizontal().w_full())
        .child(Label::new("Yours:").text_color(t.muted_foreground))
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .children(this.captured_us.iter().map(|k| {
                    div()
                        .rounded_sm()
                        .bg(t.primary.opacity(0.15))
                        .child(captured_piece_icon(
                            *k,
                            this.player_color.opposite(),
                            t.primary.opacity(0.15),
                        ))
                })),
        )
        .child(Label::new("Theirs:").text_color(t.muted_foreground))
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .children(this.captured_opp.iter().map(|k| {
                    div()
                        .rounded_sm()
                        .bg(t.danger.opacity(0.15))
                        .child(captured_piece_icon(
                            *k,
                            this.player_color,
                            t.danger.opacity(0.15),
                        ))
                })),
        )
        .into_any_element()
}

fn rematch_panel(this: &ChessBoard, cx: &Context<ChessBoard>) -> AnyElement {
    let t = cx.theme();
    let weak = cx.weak_entity();

    let header = Label::new("Rematch").text_color(t.foreground);
    let body = match this.rematch_state {
        RematchState::Hidden => div().into_any_element(),
        RematchState::Available => v_flex()
            .w_full()
            .gap_2()
            .child(Label::new("Want another game with the same opponent?").text_color(t.muted_foreground))
            .child(
                Button::new("request-rematch")
                    .w_full()
                    .label("Request Rematch")
                    .on_click(move |_, _window, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.request_rematch(cx);
                        });
                    }),
            )
            .into_any_element(),
        RematchState::Requesting => v_flex()
            .w_full()
            .gap_2()
            .child(Label::new("Rematch request sent. Waiting for your opponent.").text_color(t.muted_foreground))
            .into_any_element(),
        RematchState::RequestedByOpponent => {
            let accept_weak = cx.weak_entity();
            let decline_weak = cx.weak_entity();
            v_flex()
                .w_full()
                .gap_2()
                .child(Label::new("Your opponent wants a rematch.").text_color(t.muted_foreground))
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            Button::new("accept-rematch")
                                .flex_1()
                                .label("Accept")
                                .on_click(move |_, _window, cx| {
                                    let _ = accept_weak.update(cx, |this, cx| {
                                        this.accept_rematch(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("decline-rematch")
                                .flex_1()
                                .label("Decline")
                                .on_click(move |_, _window, cx| {
                                    let _ = decline_weak.update(cx, |this, cx| {
                                        this.decline_rematch(cx);
                                    });
                                }),
                        ),
                )
                .into_any_element()
        }
        RematchState::Starting => v_flex()
            .w_full()
            .gap_2()
            .child(Label::new("Starting rematch...").text_color(t.muted_foreground))
            .into_any_element(),
        RematchState::Declined => v_flex()
            .w_full()
            .gap_2()
            .child(Label::new("Rematch declined.").text_color(t.muted_foreground))
            .into_any_element(),
    };

    div()
        .w_full()
        .p_2()
        .rounded_md()
        .bg(t.muted.opacity(0.15))
        .child(v_flex().w_full().gap_2().child(header).child(body))
        .into_any_element()
}

fn chat_item_size(
    entry: &ChatEntry,
    list_width: Pixels,
    font_family: &SharedString,
    foreground: Hsla,
    primary: Hsla,
    muted: Hsla,
    muted_foreground: Hsla,
    window: &mut Window,
    cx: &mut Context<ChessBoard>,
) -> Size<Pixels> {
    let mut element = chat_row_element(
        entry,
        list_width,
        font_family,
        foreground,
        primary,
        muted,
        muted_foreground,
    )
    .into_any_element();

    element.layout_as_root(
        size(
            AvailableSpace::Definite(list_width),
            AvailableSpace::MinContent,
        ),
        window,
        cx,
    )
}

fn chat_list_width(window: &Window, scroll_handle: &VirtualListScrollHandle) -> Pixels {
    let measured_width = scroll_handle.bounds().size.width;
    if measured_width > px(0.) {
        return measured_width;
    }

    let viewport = window.viewport_size();
    let sidebar_width = (viewport.width - viewport.height - ROOT_GAP).max(px(0.));
    (sidebar_width - SIDEBAR_HORIZONTAL_PADDING).max(CHAT_MIN_WRAP_WIDTH)
}

fn chat_row_element(
    entry: &ChatEntry,
    _list_width: Pixels,
    font_family: &SharedString,
    foreground: Hsla,
    primary: Hsla,
    muted: Hsla,
    muted_foreground: Hsla,
) -> impl IntoElement {
    let bubble_bg = if entry.is_ours {
        primary.opacity(0.18)
    } else {
        muted.opacity(0.25)
    };
    let sender_color = if entry.is_ours {
        primary
    } else {
        muted_foreground
    };

    let body = div().w_full().p_2().rounded_md().bg(bubble_bg).child(
        Label::new(entry.message.clone())
            .font_family(font_family.clone())
            .text_size(CHAT_FONT_SIZE)
            .line_height(CHAT_LINE_HEIGHT)
            .text_color(foreground),
    );

    let row = v_flex().w_full().p_2().gap_1().child(
        Label::new(entry.sender_name.clone())
            .font_family(font_family.clone())
            .text_size(CHAT_FONT_SIZE)
            .line_height(CHAT_LINE_HEIGHT)
            .text_color(sender_color),
    );

    if entry.is_ours {
        row.items_end().child(body)
    } else {
        row.child(body)
    }
}

fn clock_row(
    label: &str,
    time: &str,
    color: Color,
    board: &ChessBoard,
    t: &gpui_component::ThemeColor,
) -> AnyElement {
    let active = board.game_state.turn == color && !board.is_game_over();
    h_flex()
        .w_full()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(if active {
            t.primary.opacity(0.2)
        } else {
            t.muted.opacity(0.15)
        })
        .child(Label::new(label).w(px(56.)).text_color(t.muted_foreground))
        .child(Label::new(time).text_color(if active { t.primary } else { t.foreground }))
        .into_any_element()
}

fn fmt_score(score: i32) -> String {
    if score.abs() >= 90_000 {
        let mate_in = (100_000 - score.abs()) / 2;
        format!("Mate in {mate_in}")
    } else {
        let centipawns = score as f64 / 100.0;
        if centipawns > 0.0 {
            format!("+{:.2}", centipawns)
        } else {
            format!("{:.2}", centipawns)
        }
    }
}

fn fmt_time(seconds: f64) -> String {
    let total = seconds.max(0.0) as u32;
    let mins = total / 60;
    let secs = total % 60;
    format!("{:02}:{:02}", mins, secs)
}

fn captured_piece_icon(piece: PieceKind, color: Color, bg: Hsla) -> AnyElement {
    div()
        .size(px(28.))
        .rounded_sm()
        .bg(bg)
        .child(
            img(piece_image(piece, color))
                .size_full()
                .object_fit(ObjectFit::Contain),
        )
        .into_any_element()
}

fn detail(t: &gpui_component::ThemeColor, label: &str, value: &str) -> AnyElement {
    h_flex()
        .w_full()
        .justify_between()
        .child(Label::new(label).text_color(t.muted_foreground))
        .child(Label::new(value).text_color(t.foreground))
        .into_any_element()
}
