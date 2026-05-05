use chess_bot::find_best_move;
use chess_engine::*;
use chess_server::protocol::ClientMessage;
use crate::audio;
use gpui::{
    AnyElement, AsyncApp, Context, Hsla, Image, ImageFormat, IntoElement, ObjectFit, ParentElement,
    Render, Styled, StyledImage, WeakEntity, Window, div, hsla, img, px,
};
use gpui_component::{
    ActiveTheme, button::Button, h_flex, label::Label, separator::Separator, v_flex,
};
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

    pub bot_score: Option<i32>,
    pub bot_depth: Option<u32>,
}

impl ChessBoard {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
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
            bot_score: None,
            bot_depth: None,
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
        self.online_tx = None;
        self.leave_requested = false;
        self.bot_score = None;
        self.bot_depth = None;
        self.refresh_status();
    }

    /// Start the per‑player countdown clock. Call once after `start_game`.
    pub fn start_timer(&mut self, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let bg = cx.background_executor().clone();
        cx.spawn(
            move |_: WeakEntity<ChessBoard>, async_app: &mut AsyncApp| {
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
                        let _ = app.update(|app| {
                            let _ = entity.update(app, |this, cx| {
                                this.tick(cx);
                            });
                        });
                    }
                }
            },
        )
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
        if was_capture
            && let Some((_, kind)) = self.game_state.get(mv.to as usize)
        {
            self.captured_opp.push(kind);
        }
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
            let time = think_time(&self.game_state, self.white_time, self.black_time, self.move_count);
            Self::schedule_bot(&self.game_state, cx, time);
        }
    }

    pub fn apply_opponent_move(&mut self, mv: Move, cx: &mut Context<Self>) {
        let was_capture = mv.capture;
        if was_capture
            && let Some((_, kind)) = self.game_state.get(mv.to as usize)
        {
            self.captured_us.push(kind);
        }
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
        if was_capture
            && let Some((_, kind)) = self.game_state.get(mv.to as usize)
        {
            self.captured_us.push(kind);
        }
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
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl Render for ChessBoard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let flip = self.player_color == Color::White;
        h_flex()
            .size_full()
            .gap_2()
            .child(board_view(self, flip, cx))
            .child(sidebar(self, cx))
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
                    let bg = if (i / 8 + i % 8) % 2 == 0 {
                        lsq()
                    } else {
                        dsq()
                    };
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

fn sidebar(this: &ChessBoard, cx: &Context<ChessBoard>) -> AnyElement {
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
        .child(Separator::horizontal().w_full())
        .children(
            (this.game_mode == GameMode::Bot).then(|| {
                let score_str = this.bot_score.map(fmt_score).unwrap_or_default();
                let depth_str = this
                    .bot_depth
                    .map(|d| format!("{d}"))
                    .unwrap_or_default();
                v_flex()
                    .w_full()
                    .px_2()
                    .gap_1()
                    .child(detail(t, "Bot Eval", &score_str))
                    .child(detail(t, "Search Depth", &depth_str))
            }),
        )
        .child(Separator::horizontal().w_full())
        .child(clock_row("White", &white_time, Color::White, this, t))
        .child(clock_row("Black", &black_time, Color::Black, this, t))
        .child(Separator::horizontal().w_full())
        .child(
            Button::new("leave-btn")
                .w_full()
                .label("Leave")
                .on_click({
                    let weak = cx.weak_entity();
                    move |_, _window, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.leave_requested = true;
                            cx.notify();
                        });
                    }
                }),
        )
        .child(Separator::horizontal().w_full())
        .child(Label::new("Yours:").text_color(t.muted_foreground))
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .children(this.captured_us.iter().map(|k| {
                    div()
                        .p_2()
                        .rounded_sm()
                        .bg(t.primary.opacity(0.15))
                        .child(abbr(*k))
                })),
        )
        .child(Label::new("Theirs:").text_color(t.muted_foreground))
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .children(this.captured_opp.iter().map(|k| {
                    div()
                        .p_2()
                        .rounded_sm()
                        .bg(t.danger.opacity(0.15))
                        .child(abbr(*k))
                })),
        )
        .into_any_element()
}

fn clock_row(label: &str, time: &str, color: Color, board: &ChessBoard, t: &gpui_component::ThemeColor) -> AnyElement {
    let active = board.game_state.turn == color && !board.is_game_over();
    h_flex()
        .w_full()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(if active { t.primary.opacity(0.2) } else { t.muted.opacity(0.15) })
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

fn abbr(k: PieceKind) -> &'static str {
    match k {
        PieceKind::Emperor => "K",
        PieceKind::Empress => "Q",
        PieceKind::Priest => "P",
        PieceKind::Paladin => "L",
        PieceKind::Dragon => "D",
        PieceKind::Knight => "N",
    }
}

fn detail(t: &gpui_component::ThemeColor, label: &str, value: &str) -> AnyElement {
    h_flex()
        .w_full()
        .justify_between()
        .child(Label::new(label).text_color(t.muted_foreground))
        .child(Label::new(value).text_color(t.foreground))
        .into_any_element()
}
