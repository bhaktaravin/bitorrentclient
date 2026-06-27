// src/ui.rs — Stratego board GUI using egui

use egui::*;
use crate::game::*;

// ─── Color palette ───────────────────────────────────────────────────────────

const COL_BG:           Color32 = Color32::from_rgb(24,  20,  16 );
const COL_BOARD_BG:     Color32 = Color32::from_rgb(38,  30,  22 );
const COL_CELL_LIGHT:   Color32 = Color32::from_rgb(210, 185, 140);
const COL_CELL_DARK:    Color32 = Color32::from_rgb(180, 150, 100);
const COL_LAKE:         Color32 = Color32::from_rgb(40,  90,  160);
const COL_LAKE_SHINE:   Color32 = Color32::from_rgb(60,  120, 200);
const COL_HIGHLIGHT:    Color32 = Color32::from_rgb(230, 220, 60 );
const COL_SELECTED:     Color32 = Color32::from_rgb(255, 255, 80 );
const COL_LAST_FROM:    Color32 = Color32::from_rgb(180, 120, 30 );
const COL_LAST_TO:      Color32 = Color32::from_rgb(220, 160, 40 );

const COL_RED_PIECE:    Color32 = Color32::from_rgb(200, 50,  40 );
const COL_RED_DARK:     Color32 = Color32::from_rgb(140, 25,  20 );
const COL_BLUE_PIECE:   Color32 = Color32::from_rgb(50,  100, 200);
const COL_BLUE_DARK:    Color32 = Color32::from_rgb(25,  60,  140);
const COL_PIECE_TEXT:   Color32 = Color32::WHITE;
const COL_HIDDEN_TEXT:  Color32 = Color32::from_rgb(200, 200, 200);

const COL_PANEL_BG:     Color32 = Color32::from_rgb(30, 24, 18);
const COL_PANEL_BORDER: Color32 = Color32::from_rgb(80, 60, 40);
const COL_TEXT_GOLD:    Color32 = Color32::from_rgb(220, 185, 90);
const COL_TEXT_DIM:     Color32 = Color32::from_rgb(160, 140, 110);

const CELL: f32 = 62.0;
const GAP:  f32 = 2.0;

const AI_THINK_DELAY: f32 = 0.75;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppScreen {
    Menu,
    Game,
}

// ─── App struct ──────────────────────────────────────────────────────────────

pub struct StrategoApp {
    screen: AppScreen,
    pub game: GameState,
    pub setup_selected_rank: Option<Rank>,
    ai_cooldown: f32,
    ai_acted: bool,
    was_ai_turn: bool,
}

impl StrategoApp {
    pub fn new(_cc: &eframe::CreationContext) -> Self {
        StrategoApp {
            screen: AppScreen::Menu,
            game: GameState::new(GameMode::SoloVsAi),
            setup_selected_rank: None,
            ai_cooldown: 0.0,
            ai_acted: false,
            was_ai_turn: false,
        }
    }

    fn start_game(&mut self, mode: GameMode) {
        self.game = GameState::new(mode);
        self.setup_selected_rank = None;
        self.ai_cooldown = 0.0;
        self.ai_acted = false;
        self.was_ai_turn = false;
        self.screen = AppScreen::Game;
    }

    fn return_to_menu(&mut self) {
        self.screen = AppScreen::Menu;
        self.setup_selected_rank = None;
        self.ai_cooldown = 0.0;
        self.ai_acted = false;
        self.was_ai_turn = false;
    }

    fn tick_ai(&mut self, ctx: &Context) {
        let is_ai_turn = self.game.is_ai_turn();
        if is_ai_turn && !self.was_ai_turn {
            self.ai_cooldown = AI_THINK_DELAY;
            self.ai_acted = false;
        }
        self.was_ai_turn = is_ai_turn;

        if !is_ai_turn {
            self.ai_acted = false;
            return;
        }

        let dt = ctx.input(|i| i.unstable_dt);
        if self.ai_cooldown > 0.0 {
            self.ai_cooldown -= dt;
            ctx.request_repaint();
            return;
        }

        if !self.ai_acted {
            self.game.make_ai_move();
            self.ai_acted = true;
        }
    }
}

impl eframe::App for StrategoApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Style
        let mut visuals = Visuals::dark();
        visuals.window_fill = COL_BG;
        visuals.panel_fill  = COL_BG;
        ctx.set_visuals(visuals);

        if self.screen == AppScreen::Menu {
            CentralPanel::default()
                .frame(egui::Frame::central_panel(&ctx.style()).fill(COL_BG))
                .show(ctx, |ui| self.draw_menu(ui));
            return;
        }

        self.tick_ai(ctx);

        let is_setup = matches!(self.game.phase, Phase::Setup(_));
        let is_over  = matches!(self.game.phase, Phase::GameOver(_));
        let is_ai_thinking = self.game.is_ai_turn() && self.ai_cooldown > 0.0;

        // Left panel — setup inventory or captured pieces
        SidePanel::left("left_panel")
            .min_width(170.0)
            .resizable(false)
            .frame(egui::Frame::side_top_panel(&ctx.style())
                .fill(COL_PANEL_BG)
                .stroke(Stroke::new(1.0, COL_PANEL_BORDER)))
            .show(ctx, |ui| {
                self.draw_left_panel(ui, is_setup);
            });

        // Right panel — info + captured
        SidePanel::right("right_panel")
            .min_width(170.0)
            .resizable(false)
            .frame(egui::Frame::side_top_panel(&ctx.style())
                .fill(COL_PANEL_BG)
                .stroke(Stroke::new(1.0, COL_PANEL_BORDER)))
            .show(ctx, |ui| {
                self.draw_right_panel(ui);
            });

        // Bottom bar — status message
        TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::side_top_panel(&ctx.style())
                .fill(COL_PANEL_BG)
                .inner_margin(Margin::symmetric(12, 8)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let mode_label = match self.game.mode {
                        GameMode::SoloVsAi => "vs Computer",
                        GameMode::Hotseat => "Hotseat",
                    };
                    ui.colored_label(
                        COL_TEXT_DIM,
                        RichText::new(mode_label).size(12.0),
                    );
                    ui.separator();
                    let phase_txt = match &self.game.phase {
                        Phase::Setup(p) => format!("{:?} Setup", p),
                        Phase::Play if is_ai_thinking => "Blue is thinking…".into(),
                        Phase::Play => format!("{:?}'s Turn", self.game.current_player),
                        Phase::GameOver(w) => format!("{:?} WINS!", w),
                    };
                    ui.colored_label(COL_TEXT_GOLD,
                        RichText::new(&phase_txt).strong().size(14.0));
                    ui.separator();
                    ui.colored_label(Color32::WHITE,
                        RichText::new(&self.game.message).size(13.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("⌂  Menu").size(13.0)).clicked() {
                            self.return_to_menu();
                        }
                        if ui.button(RichText::new("⟳  New Game").size(13.0)).clicked() {
                            let mode = self.game.mode;
                            self.start_game(mode);
                        }
                    });
                });
            });

        // Central panel — the board
        CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).fill(COL_BG))
            .show(ctx, |ui| {
                if is_over {
                    if let Phase::GameOver(winner) = self.game.phase {
                        self.draw_victory_overlay(ui, winner);
                    }
                } else {
                    self.draw_board(ui, is_setup);
                    if is_ai_thinking {
                        self.draw_ai_thinking_overlay(ui);
                    }
                }
            });
    }
}

impl StrategoApp {
    // ── Board rendering ────────────────────────────────────────────────────

    fn draw_board(&mut self, ui: &mut Ui, is_setup: bool) {
        let mut clicked_cell: Option<(usize, usize)> = None;

        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);

            // Column labels (A–J)
            ui.horizontal(|ui| {
                ui.allocate_exact_size(vec2(18.0, 14.0), Sense::hover());
                for col in 0..COLS {
                    let (r, _) = ui.allocate_exact_size(vec2(CELL, 14.0), Sense::hover());
                    ui.painter().text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        char::from(b'A' + col as u8).to_string(),
                        FontId::proportional(11.0),
                        COL_TEXT_DIM,
                    );
                }
            });

            // Grid rows
            for row in 0..ROWS {
                ui.horizontal(|ui| {
                    let (label_rect, _) = ui.allocate_exact_size(vec2(18.0, CELL), Sense::hover());
                    ui.painter().text(
                        label_rect.center(),
                        Align2::CENTER_CENTER,
                        format!("{}", ROWS - row),
                        FontId::proportional(11.0),
                        COL_TEXT_DIM,
                    );

                    for col in 0..COLS {
                        let (cell_rect, response) =
                            ui.allocate_exact_size(vec2(CELL, CELL), Sense::click());
                        self.paint_cell(ui, cell_rect, col, row, is_setup);
                        if response.clicked() {
                            clicked_cell = Some((col, row));
                        }
                        if is_setup && response.hovered() {
                            if let Some(rank) = self.setup_selected_rank {
                                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                                response.on_hover_text(format!("Place {}", rank.full_name()));
                            }
                        }
                    }
                });
            }
        });

        if let Some((col, row)) = clicked_cell {
            if self.game.is_ai_turn() {
                return;
            }
            if is_setup {
                if let Some(rank) = self.setup_selected_rank {
                    if self.game.try_place(rank, col, row) {
                        if !matches!(self.game.phase, Phase::Setup(_))
                            || self.game.setup_inventory.get(&rank).copied().unwrap_or(0) == 0
                        {
                            self.setup_selected_rank = None;
                        }
                    }
                } else {
                    self.game.message =
                        "Select a piece type from the left panel first.".into();
                }
            } else {
                self.game.click_cell(col, row);
            }
        }
    }

    fn paint_cell(&self, ui: &Ui, rect: Rect, col: usize, row: usize, is_setup: bool) {
        let painter = ui.painter();

        let is_lake = Board::is_lake(col, row);
        let is_sel = self.game.selected == Some((col, row));
        let is_hi = self.game.highlights.contains(&(col, row));
        let is_last_from = self
            .game
            .last_move
            .as_ref()
            .map(|m| m.from == (col, row))
            .unwrap_or(false);
        let is_last_to = self
            .game
            .last_move
            .as_ref()
            .map(|m| m.to == (col, row))
            .unwrap_or(false);

        let bg = if is_lake {
            COL_LAKE
        } else if is_sel {
            COL_SELECTED
        } else if is_last_to {
            COL_LAST_TO
        } else if is_last_from {
            COL_LAST_FROM
        } else if (col + row) % 2 == 0 {
            COL_CELL_LIGHT
        } else {
            COL_CELL_DARK
        };
        painter.rect_filled(rect, 3.0, bg);

        if is_hi && !is_lake {
            painter.rect_stroke(
                rect.shrink(1.5),
                3.0,
                Stroke::new(3.0, COL_HIGHLIGHT),
                StrokeKind::Outside,
            );
        }

        if is_lake {
            let shine = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(10.0, 4.0));
            painter.rect_filled(shine, 2.0, COL_LAKE_SHINE.gamma_multiply(0.5));
        }

        if is_setup {
            if let Phase::Setup(player) = self.game.phase {
                let valid_rows = GameState::setup_rows_for(player);
                if valid_rows.contains(&row) && !is_lake {
                    let tint = match player {
                        Player::Red => Color32::from_rgba_unmultiplied(200, 50, 40, 30),
                        Player::Blue => Color32::from_rgba_unmultiplied(50, 100, 200, 30),
                    };
                    painter.rect_filled(rect, 3.0, tint);
                }
            }
        }

        if let Some(piece) = self.game.board.get(col, row) {
            self.draw_piece(&painter, rect, piece, is_sel);
        }
    }

    fn draw_piece(
        &self,
        painter: &Painter,
        rect: Rect,
        piece: &Piece,
        _selected: bool,
    ) {
        let (main_col, shadow_col) = match piece.player {
            Player::Red  => (COL_RED_PIECE,  COL_RED_DARK),
            Player::Blue => (COL_BLUE_PIECE, COL_BLUE_DARK),
        };

        // Show rank based on fog-of-war / setup rules
        let show_rank = self.game.can_see_rank(piece);

        // Piece body (rounded rect)
        let inner = rect.shrink(6.0);
        // Shadow
        painter.rect_filled(inner.translate(vec2(2.0, 2.0)), 5.0,
            Color32::from_black_alpha(80));
        painter.rect_filled(inner, 5.0, main_col);
        // Highlight edge
        painter.rect_stroke(inner, 5.0, Stroke::new(1.5, shadow_col), StrokeKind::Outside);

        // Rank label
        if show_rank {
            let rank_str = piece.rank.display_str();
            let font_size = if rank_str.len() > 1 { 16.0 } else { 20.0 };
            painter.text(
                inner.center(), Align2::CENTER_CENTER,
                rank_str, FontId::monospace(font_size), COL_PIECE_TEXT
            );
        } else {
            // Hidden piece — show "?"
            painter.text(
                inner.center(), Align2::CENTER_CENTER,
                "?", FontId::monospace(18.0), COL_HIDDEN_TEXT
            );
        }
    }

    // ── Left panel ─────────────────────────────────────────────────────────

    fn draw_left_panel(&mut self, ui: &mut Ui, is_setup: bool) {
        ui.add_space(12.0);

        if is_setup {
            if let Phase::Setup(player) = self.game.phase {
                ui.colored_label(COL_TEXT_GOLD,
                    RichText::new(format!("{:?} SETUP", player)).strong().size(15.0));
            }
            ui.colored_label(COL_TEXT_DIM,
                format!("  {} left to place", self.game.remaining_pieces()));
            if self.game.mode == GameMode::Hotseat {
                ui.colored_label(COL_TEXT_DIM, "  Pass screen to the active player");
            }
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ScrollArea::vertical().show(ui, |ui| {
                for &rank in ALL_RANKS {
                    let count = *self.game.setup_inventory.get(&rank).unwrap_or(&0);
                    let is_sel = self.setup_selected_rank == Some(rank);
                    let (col, _) = rank_colors(rank);
                    let label = format!(
                        "[{}]  {}  ×{}",
                        rank.display_str(),
                        rank.full_name(),
                        count
                    );
                    let btn = Button::new(RichText::new(label).monospace().size(14.0).color(
                        if count > 0 { col } else { COL_TEXT_DIM },
                    ))
                    .fill(if is_sel {
                        col.gamma_multiply(0.35)
                    } else {
                        Color32::from_rgba_unmultiplied(40, 35, 30, 180)
                    })
                    .stroke(Stroke::new(if is_sel { 2.0 } else { 0.0 }, col))
                    .min_size(vec2(148.0, 28.0));

                    if ui.add_enabled(count > 0, btn).clicked() {
                        self.setup_selected_rank = Some(rank);
                        self.game.message = format!(
                            "Selected {}. Click a red-tinted cell to place it.",
                            rank.full_name()
                        );
                    }
                    ui.add_space(3.0);
                }
            });
        } else {
            // During play: show captured pieces from Blue (Blue lost these)
            ui.colored_label(COL_TEXT_GOLD,
                RichText::new("🔵 BLUE LOST").strong().size(13.0));
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            let mut sorted = self.game.captured_blue.clone();
            sorted.sort();
            for rank in &sorted {
                ui.colored_label(COL_BLUE_PIECE,
                    format!("  {} {}", rank.display_str(), rank.full_name()));
            }
            if sorted.is_empty() {
                ui.colored_label(COL_TEXT_DIM, "  (none yet)");
            }
        }
    }

    // ── Right panel ────────────────────────────────────────────────────────

    fn draw_right_panel(&self, ui: &mut Ui) {
        let is_setup = matches!(self.game.phase, Phase::Setup(_));
        ui.add_space(12.0);

        // Legend / rules quick ref
        ui.colored_label(COL_TEXT_GOLD,
            RichText::new("PIECE RANKS").strong().size(15.0));
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);
        ScrollArea::vertical().id_salt("legend").show(ui, |ui| {
            for &rank in ALL_RANKS {
                let (col, _) = rank_colors(rank);
                ui.horizontal(|ui| {
                    ui.colored_label(col,
                        RichText::new(rank.display_str()).monospace().strong().size(14.0));
                    ui.colored_label(COL_TEXT_DIM,
                        format!("  {}  ×{}", rank.full_name(), rank.count_per_player()));
                });
            }
        });

        if !is_setup {
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            ui.colored_label(COL_TEXT_GOLD,
                RichText::new("🔴 RED LOST").strong().size(13.0));
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            let mut sorted = self.game.captured_red.clone();
            sorted.sort();
            for rank in &sorted {
                ui.colored_label(COL_RED_PIECE,
                    format!("  {} {}", rank.display_str(), rank.full_name()));
            }
            if sorted.is_empty() {
                ui.colored_label(COL_TEXT_DIM, "  (none yet)");
            }
        }
    }

    // ── Victory overlay ────────────────────────────────────────────────────

    fn draw_victory_overlay(&mut self, ui: &mut Ui, winner: Player) {
        let avail = ui.available_rect_before_wrap();

        let card_w = 360.0;
        let card_h = 220.0;
        let card_rect = Rect::from_center_size(avail.center(), vec2(card_w, card_h));
        let btn_rect = Rect::from_center_size(
            card_rect.center_bottom() - vec2(0.0, 22.0), vec2(140.0, 36.0)
        );

        let (winner_col, winner_name) = match winner {
            Player::Red  => (COL_RED_PIECE,  "RED"),
            Player::Blue => (COL_BLUE_PIECE, "BLUE"),
        };

        {
            let painter = ui.painter();

            painter.rect_filled(avail, 0.0, Color32::from_black_alpha(170));
            painter.rect_filled(card_rect, 12.0, COL_PANEL_BG);
            painter.rect_stroke(card_rect, 12.0,
                Stroke::new(3.0, COL_TEXT_GOLD), StrokeKind::Outside);
            painter.text(
                card_rect.center_top() + vec2(0.0, 40.0),
                Align2::CENTER_CENTER,
                "⚑  FLAG CAPTURED",
                FontId::proportional(18.0), COL_TEXT_DIM
            );
            painter.text(
                card_rect.center(),
                Align2::CENTER_CENTER,
                winner_name,
                FontId::proportional(52.0), winner_col
            );
            painter.text(
                card_rect.center_bottom() - vec2(0.0, 55.0),
                Align2::CENTER_CENTER,
                "WINS THE BATTLE!",
                FontId::proportional(20.0), COL_TEXT_GOLD
            );
            painter.rect_filled(btn_rect, 6.0, COL_TEXT_GOLD.gamma_multiply(0.2));
            painter.rect_stroke(btn_rect, 6.0, Stroke::new(1.5, COL_TEXT_GOLD), StrokeKind::Outside);
            painter.text(
                btn_rect.center(), Align2::CENTER_CENTER,
                "Play Again",
                FontId::proportional(15.0), COL_TEXT_GOLD
            );
        }

        ui.allocate_rect(card_rect, Sense::hover());
        if ui.allocate_rect(btn_rect, Sense::click()).clicked() {
            let mode = self.game.mode;
            self.start_game(mode);
        }
    }

    fn draw_menu(&mut self, ui: &mut Ui) {
        let avail = ui.available_rect_before_wrap();
        let card_w = 420.0;
        let card_h = 320.0;
        let card_rect = Rect::from_center_size(avail.center(), vec2(card_w, card_h));

        {
            let painter = ui.painter();
            painter.rect_filled(card_rect, 12.0, COL_PANEL_BG);
            painter.rect_stroke(card_rect, 12.0,
                Stroke::new(2.0, COL_TEXT_GOLD), StrokeKind::Outside);
            painter.text(
                card_rect.center_top() + vec2(0.0, 36.0),
                Align2::CENTER_CENTER,
                "STRATEGO",
                FontId::proportional(36.0), COL_TEXT_GOLD
            );
            painter.text(
                card_rect.center_top() + vec2(0.0, 72.0),
                Align2::CENTER_CENTER,
                "Capture the enemy Flag",
                FontId::proportional(14.0), COL_TEXT_DIM
            );
        }

        ui.allocate_ui_at_rect(card_rect, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.set_width(280.0);

                if ui.button(RichText::new("▶  Play vs Computer").size(16.0)).clicked() {
                    self.start_game(GameMode::SoloVsAi);
                }
                ui.add_space(8.0);
                if ui.button(RichText::new("👥  Hotseat (2 Players)").size(16.0)).clicked() {
                    self.start_game(GameMode::Hotseat);
                }
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);
                ui.colored_label(COL_TEXT_DIM, "Solo: you play Red vs Blue AI");
                ui.colored_label(COL_TEXT_DIM, "Hotseat: pass the screen each turn");
            });
        });
    }

    fn draw_ai_thinking_overlay(&self, ui: &mut Ui) {
        let avail = ui.available_rect_before_wrap();
        let painter = ui.painter();
        painter.rect_filled(avail, 0.0, Color32::from_black_alpha(40));
        painter.text(
            avail.center(),
            Align2::CENTER_CENTER,
            "Blue is thinking…",
            FontId::proportional(22.0),
            COL_BLUE_PIECE,
        );
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn rank_colors(rank: Rank) -> (Color32, Color32) {
    match rank {
        Rank::Flag       => (Color32::from_rgb(255, 215, 0),   Color32::from_rgb(180,150,0)),
        Rank::Bomb       => (Color32::from_rgb(80,  80,  80),  Color32::from_rgb(40, 40, 40)),
        Rank::Marshal    => (Color32::from_rgb(230, 70,  50),  Color32::from_rgb(160,30,20)),
        Rank::General    => (Color32::from_rgb(200, 100, 60),  Color32::from_rgb(140,60,30)),
        Rank::Colonel    => (Color32::from_rgb(170, 130, 80),  Color32::from_rgb(120,90,50)),
        Rank::Spy        => (Color32::from_rgb(160, 80,  180), Color32::from_rgb(100,40,120)),
        _                => (Color32::from_rgb(180, 180, 180), Color32::from_rgb(120,120,120)),
    }
}