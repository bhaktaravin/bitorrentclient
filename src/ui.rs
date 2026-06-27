// src/ui.rs — Stratego board GUI using egui

use egui::*;
use crate::game::*;

// ─── Color palette ───────────────────────────────────────────────────────────

const COL_BG:             Color32 = Color32::from_rgb(16, 18, 24);
const COL_WOOD_DARK:      Color32 = Color32::from_rgb(52, 34, 22);
const COL_WOOD_MID:       Color32 = Color32::from_rgb(88, 58, 34);
const COL_WOOD_LIGHT:     Color32 = Color32::from_rgb(128, 86, 48);
const COL_WOOD_HIGHLIGHT: Color32 = Color32::from_rgb(168, 118, 62);

const COL_CELL_LIGHT:     Color32 = Color32::from_rgb(224, 202, 158);
const COL_CELL_DARK:      Color32 = Color32::from_rgb(188, 158, 108);
const COL_LAKE_DEEP:      Color32 = Color32::from_rgb(22, 58, 98);
const COL_LAKE:           Color32 = Color32::from_rgb(42, 98, 152);
const COL_LAKE_SHINE:     Color32 = Color32::from_rgb(130, 195, 235);

const COL_MOVE_DOT:       Color32 = Color32::from_rgb(255, 228, 72);
const COL_SELECT_GLOW:    Color32 = Color32::from_rgb(255, 245, 120);
const COL_SELECTED:       Color32 = Color32::from_rgb(255, 240, 140);
const COL_LAST_FROM:      Color32 = Color32::from_rgb(200, 140, 50);
const COL_LAST_TO:        Color32 = Color32::from_rgb(240, 180, 60);

const COL_RED_PIECE:      Color32 = Color32::from_rgb(210, 58, 48);
const COL_RED_DARK:       Color32 = Color32::from_rgb(130, 28, 22);
const COL_RED_LIGHT:      Color32 = Color32::from_rgb(240, 110, 95);
const COL_BLUE_PIECE:     Color32 = Color32::from_rgb(52, 112, 210);
const COL_BLUE_DARK:      Color32 = Color32::from_rgb(22, 58, 140);
const COL_BLUE_LIGHT:     Color32 = Color32::from_rgb(110, 160, 240);

const COL_PIECE_FACE:     Color32 = Color32::from_rgb(248, 244, 236);
const COL_PIECE_TEXT:     Color32 = Color32::from_rgb(38, 32, 28);
const COL_HIDDEN_FACE:    Color32 = Color32::from_rgb(72, 68, 78);
const COL_HIDDEN_TEXT:    Color32 = Color32::from_rgb(190, 185, 195);

const COL_PANEL_BG:       Color32 = Color32::from_rgb(26, 28, 36);
const COL_PANEL_BORDER:   Color32 = Color32::from_rgb(72, 58, 42);
const COL_TEXT_GOLD:      Color32 = Color32::from_rgb(232, 198, 98);
const COL_TEXT_DIM:       Color32 = Color32::from_rgb(150, 145, 135);

const CELL: f32 = 62.0;
const GAP:  f32 = 2.0;
const AI_THINK_DELAY: f32 = 0.75;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppScreen {
    Menu,
    Game,
}

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
        let mut visuals = Visuals::dark();
        visuals.window_fill = COL_BG;
        visuals.panel_fill = COL_BG;
        visuals.selection.bg_fill = COL_TEXT_GOLD.gamma_multiply(0.25);
        ctx.set_visuals(visuals);

        if self.screen == AppScreen::Menu {
            CentralPanel::default()
                .frame(egui::Frame::central_panel(&ctx.style()).fill(COL_BG))
                .show(ctx, |ui| self.draw_menu(ui));
            return;
        }

        self.tick_ai(ctx);

        let is_setup = matches!(self.game.phase, Phase::Setup(_));
        let is_over = matches!(self.game.phase, Phase::GameOver(_));
        let is_ai_thinking = self.game.is_ai_turn() && self.ai_cooldown > 0.0;

        SidePanel::left("left_panel")
            .min_width(178.0)
            .resizable(false)
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(COL_PANEL_BG)
                    .stroke(Stroke::new(1.5, COL_PANEL_BORDER))
                    .inner_margin(Margin::same(10)),
            )
            .show(ctx, |ui| self.draw_left_panel(ui, is_setup));

        SidePanel::right("right_panel")
            .min_width(178.0)
            .resizable(false)
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(COL_PANEL_BG)
                    .stroke(Stroke::new(1.5, COL_PANEL_BORDER))
                    .inner_margin(Margin::same(10)),
            )
            .show(ctx, |ui| self.draw_right_panel(ui));

        TopBottomPanel::bottom("status_bar")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(COL_PANEL_BG)
                    .stroke(Stroke::new(1.0, COL_PANEL_BORDER))
                    .inner_margin(Margin::symmetric(14, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(mode_badge(self.game.mode))
                            .size(11.0)
                            .color(COL_TEXT_DIM)
                            .background_color(Color32::from_rgba_unmultiplied(255, 255, 255, 12)),
                    );
                    ui.separator();
                    ui.colored_label(
                        COL_TEXT_GOLD,
                        RichText::new(phase_label(&self.game, is_ai_thinking))
                            .strong()
                            .size(14.0),
                    );
                    ui.separator();
                    ui.colored_label(
                        Color32::from_rgb(230, 228, 220),
                        RichText::new(&self.game.message).size(13.0),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(RichText::new("Menu").size(13.0))
                            .on_hover_text("Return to main menu")
                            .clicked()
                        {
                            self.return_to_menu();
                        }
                        if ui
                            .button(RichText::new("New Game").size(13.0))
                            .on_hover_text("Restart this mode")
                            .clicked()
                        {
                            let mode = self.game.mode;
                            self.start_game(mode);
                        }
                    });
                });
            });

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
    fn draw_board(&mut self, ui: &mut Ui, is_setup: bool) {
        let anim = ui.input(|i| i.time) as f32;
        let mut clicked_cell: Option<(usize, usize)> = None;
        let mut hover_cell: Option<(usize, usize)> = None;
        let mut cell_centers = [[Pos2::ZERO; COLS]; ROWS];

        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            Frame::new()
                .fill(COL_WOOD_DARK)
                .stroke(Stroke::new(3.0, COL_WOOD_LIGHT))
                .corner_radius(10)
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    // Inner wood inset line
                    let inset = ui.max_rect().shrink(4.0);
                    ui.painter().rect_stroke(
                        inset,
                        6,
                        Stroke::new(1.0, COL_WOOD_MID),
                        StrokeKind::Inside,
                    );

                    ui.spacing_mut().item_spacing = vec2(GAP, GAP);

                    ui.horizontal(|ui| {
                        ui.allocate_exact_size(vec2(20.0, 16.0), Sense::hover());
                        for col in 0..COLS {
                            let (r, _) = ui.allocate_exact_size(vec2(CELL, 16.0), Sense::hover());
                            ui.painter().text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                char::from(b'A' + col as u8).to_string(),
                                FontId::proportional(12.0),
                                COL_WOOD_HIGHLIGHT,
                            );
                        }
                    });

                    for row in 0..ROWS {
                        ui.horizontal(|ui| {
                            let (label_rect, _) =
                                ui.allocate_exact_size(vec2(20.0, CELL), Sense::hover());
                            ui.painter().text(
                                label_rect.center(),
                                Align2::CENTER_CENTER,
                                format!("{}", ROWS - row),
                                FontId::proportional(12.0),
                                COL_WOOD_HIGHLIGHT,
                            );

                            for col in 0..COLS {
                                let (cell_rect, response) =
                                    ui.allocate_exact_size(vec2(CELL, CELL), Sense::click());
                                cell_centers[row][col] = cell_rect.center();
                                self.paint_cell(ui, cell_rect, col, row, is_setup, anim);
                                if response.clicked() {
                                    clicked_cell = Some((col, row));
                                }
                                if response.hovered() {
                                    hover_cell = Some((col, row));
                                }
                                if is_setup && response.hovered() {
                                    if let Some(rank) = self.setup_selected_rank {
                                        ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                                        response.on_hover_text(format!(
                                            "Place {} at {}",
                                            rank.full_name(),
                                            format_cell(col, row)
                                        ));
                                    }
                                }
                            }
                        });
                    }
                });
        });

        if let Some((sc, sr)) = self.game.selected {
            if let Some((hc, hr)) = hover_cell {
                if self.game.highlights.contains(&(hc, hr)) {
                    draw_hover_arrow(
                        ui,
                        cell_centers[sr][sc],
                        cell_centers[hr][hc],
                    );
                }
            }
        }

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

    fn paint_cell(
        &self,
        ui: &Ui,
        rect: Rect,
        col: usize,
        row: usize,
        is_setup: bool,
        anim: f32,
    ) {
        let painter = ui.painter();
        let pulse = (anim * 3.5).sin() * 0.5 + 0.5;

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

        if is_lake {
            paint_lake(painter, rect, anim);
        } else {
            let bg = if is_sel {
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
            painter.rect_filled(rect, 4.0, bg);
            painter.rect_stroke(
                rect.shrink(1.0),
                3.0,
                Stroke::new(1.0, Color32::from_white_alpha(30)),
                StrokeKind::Inside,
            );
        }

        if is_setup {
            if let Phase::Setup(player) = self.game.phase {
                let valid_rows = GameState::setup_rows_for(player);
                if valid_rows.contains(&row) && !is_lake {
                    let tint = match player {
                        Player::Red => Color32::from_rgba_unmultiplied(220, 60, 45, 35),
                        Player::Blue => Color32::from_rgba_unmultiplied(55, 110, 220, 35),
                    };
                    painter.rect_filled(rect, 4.0, tint);
                }
            }
        }

        if is_hi && !is_lake {
            let dot_r = 5.0 + pulse * 2.0;
            let alpha = (120.0 + pulse * 100.0) as u8;
            painter.circle_filled(
                rect.center(),
                dot_r,
                COL_MOVE_DOT.gamma_multiply(alpha as f32 / 255.0),
            );
            painter.rect_stroke(
                rect.shrink(2.0),
                4.0,
                Stroke::new(2.0 + pulse, COL_MOVE_DOT.gamma_multiply(0.85)),
                StrokeKind::Outside,
            );
        }

        if is_sel {
            painter.rect_stroke(
                rect.expand(2.0),
                5.0,
                Stroke::new(2.5 + pulse * 1.5, COL_SELECT_GLOW),
                StrokeKind::Outside,
            );
        }

        if let Some(piece) = self.game.board.get(col, row) {
            draw_piece_tile(painter, rect, piece, self.game.can_see_rank(piece), is_sel);
        }
    }

    fn draw_left_panel(&mut self, ui: &mut Ui, is_setup: bool) {
        ui.add_space(4.0);

        if is_setup {
            if let Phase::Setup(player) = self.game.phase {
                ui.colored_label(
                    COL_TEXT_GOLD,
                    RichText::new(format!("{} Setup", player_label(player)))
                        .strong()
                        .size(16.0),
                );
            }
            ui.colored_label(
                COL_TEXT_DIM,
                format!("{} pieces remaining", self.game.remaining_pieces()),
            );
            if self.game.mode == GameMode::Hotseat {
                ui.colored_label(COL_TEXT_DIM, "Pass the screen between players");
            }
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ScrollArea::vertical().show(ui, |ui| {
                for &rank in ALL_RANKS {
                    let count = *self.game.setup_inventory.get(&rank).unwrap_or(&0);
                    let is_sel = self.setup_selected_rank == Some(rank);
                    let (accent, _) = rank_colors(rank);

                    if piece_picker_button(ui, rank, count, is_sel, accent).clicked() && count > 0 {
                        self.setup_selected_rank = Some(rank);
                        self.game.message = format!(
                            "Selected {} — click a cell on your side to place it.",
                            rank.full_name()
                        );
                    }
                    ui.add_space(4.0);
                }
            });
        } else {
            ui.colored_label(
                COL_TEXT_GOLD,
                RichText::new("Enemy Losses").strong().size(14.0),
            );
            ui.colored_label(COL_TEXT_DIM, "Blue pieces captured");
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);
            let mut sorted = self.game.captured_blue.clone();
            sorted.sort();
            for rank in &sorted {
                ui.horizontal(|ui| {
                    draw_mini_tile(ui, *rank, Player::Blue, true);
                    ui.colored_label(COL_TEXT_DIM, rank.full_name());
                });
            }
            if sorted.is_empty() {
                ui.colored_label(COL_TEXT_DIM, "  None yet");
            }
        }
    }

    fn draw_right_panel(&self, ui: &mut Ui) {
        let is_setup = matches!(self.game.phase, Phase::Setup(_));
        ui.add_space(4.0);

        ui.colored_label(
            COL_TEXT_GOLD,
            RichText::new("Field Manual").strong().size(16.0),
        );
        ui.colored_label(COL_TEXT_DIM, "Ranks & counts");
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        ScrollArea::vertical().id_salt("legend").show(ui, |ui| {
            for &rank in ALL_RANKS {
                let (accent, _) = rank_colors(rank);
                ui.horizontal(|ui| {
                    draw_mini_tile(ui, rank, Player::Red, true);
                    ui.colored_label(accent, RichText::new(rank.full_name()).strong());
                    ui.colored_label(COL_TEXT_DIM, format!("×{}", rank.count_per_player()));
                })
                .response
                .on_hover_text(rank_tooltip(rank));
                ui.add_space(2.0);
            }
        });

        if !is_setup {
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            ui.colored_label(
                COL_TEXT_GOLD,
                RichText::new("Your Losses").strong().size(14.0),
            );
            ui.colored_label(COL_TEXT_DIM, "Red pieces captured");
            ui.add_space(4.0);
            let mut sorted = self.game.captured_red.clone();
            sorted.sort();
            for rank in &sorted {
                ui.horizontal(|ui| {
                    draw_mini_tile(ui, *rank, Player::Red, true);
                    ui.colored_label(COL_TEXT_DIM, rank.full_name());
                });
            }
            if sorted.is_empty() {
                ui.colored_label(COL_TEXT_DIM, "  None yet");
            }
        }
    }

    fn draw_victory_overlay(&mut self, ui: &mut Ui, winner: Player) {
        let avail = ui.available_rect_before_wrap();
        let card_w = 380.0;
        let card_h = 240.0;
        let card_rect = Rect::from_center_size(avail.center(), vec2(card_w, card_h));
        let btn_rect = Rect::from_center_size(
            card_rect.center_bottom() - vec2(0.0, 28.0),
            vec2(160.0, 38.0),
        );

        let (winner_col, winner_name) = match winner {
            Player::Red => (COL_RED_PIECE, "RED"),
            Player::Blue => (COL_BLUE_PIECE, "BLUE"),
        };

        {
            let painter = ui.painter();
            painter.rect_filled(avail, 0.0, Color32::from_black_alpha(190));
            painter.rect_filled(card_rect, 14.0, COL_PANEL_BG);
            painter.rect_stroke(
                card_rect,
                14.0,
                Stroke::new(3.0, COL_TEXT_GOLD),
                StrokeKind::Outside,
            );
            painter.text(
                card_rect.center_top() + vec2(0.0, 44.0),
                Align2::CENTER_CENTER,
                "FLAG CAPTURED",
                FontId::proportional(18.0),
                COL_TEXT_DIM,
            );
            painter.text(
                card_rect.center(),
                Align2::CENTER_CENTER,
                winner_name,
                FontId::proportional(54.0),
                winner_col,
            );
            painter.text(
                card_rect.center_bottom() - vec2(0.0, 62.0),
                Align2::CENTER_CENTER,
                "Victory!",
                FontId::proportional(20.0),
                COL_TEXT_GOLD,
            );
            painter.rect_filled(btn_rect, 8.0, COL_TEXT_GOLD.gamma_multiply(0.18));
            painter.rect_stroke(
                btn_rect,
                8.0,
                Stroke::new(1.5, COL_TEXT_GOLD),
                StrokeKind::Outside,
            );
            painter.text(
                btn_rect.center(),
                Align2::CENTER_CENTER,
                "Play Again",
                FontId::proportional(15.0),
                COL_TEXT_GOLD,
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
        let painter = ui.painter();

        // Background vignette
        painter.rect_filled(avail, 0.0, COL_BG);
        painter.circle_filled(
            avail.center_top() + vec2(0.0, 80.0),
            280.0,
            Color32::from_rgba_unmultiplied(90, 60, 30, 18),
        );

        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.colored_label(
                COL_TEXT_GOLD,
                RichText::new("STRATEGO").size(48.0).strong(),
            );
            ui.colored_label(
                COL_TEXT_DIM,
                RichText::new("Outrank. Outthink. Capture the Flag.").size(15.0),
            );
            ui.add_space(36.0);
            ui.set_width(340.0);

            if mode_card(
                ui,
                "Play vs Computer",
                "You command Red against a Blue AI opponent.",
                COL_RED_PIECE,
            ) {
                self.start_game(GameMode::SoloVsAi);
            }
            ui.add_space(12.0);
            if mode_card(
                ui,
                "Hotseat — 2 Players",
                "Share one screen. Pass it when it's your turn.",
                COL_BLUE_PIECE,
            ) {
                self.start_game(GameMode::Hotseat);
            }
        });
    }

    fn draw_ai_thinking_overlay(&self, ui: &mut Ui) {
        let avail = ui.available_rect_before_wrap();
        let painter = ui.painter();
        painter.rect_filled(avail, 0.0, Color32::from_black_alpha(50));
        painter.text(
            avail.center(),
            Align2::CENTER_CENTER,
            "Blue is planning its move…",
            FontId::proportional(22.0),
            COL_BLUE_LIGHT,
        );
    }
}

fn draw_hover_arrow(ui: &Ui, from: Pos2, to: Pos2) {
    let painter = ui.ctx().layer_painter(LayerId::new(
        Order::Foreground,
        Id::new("hover_arrow"),
    ));
    painter.line_segment(
        [from, to],
        Stroke::new(2.5, Color32::from_rgba_unmultiplied(255, 240, 100, 180)),
    );
    painter.circle_filled(to, 5.0, COL_MOVE_DOT);
}

// ─── Piece rendering ─────────────────────────────────────────────────────────

fn draw_piece_tile(painter: &Painter, rect: Rect, piece: &Piece, show_rank: bool, selected: bool) {
    let (main, dark, light) = match piece.player {
        Player::Red => (COL_RED_PIECE, COL_RED_DARK, COL_RED_LIGHT),
        Player::Blue => (COL_BLUE_PIECE, COL_BLUE_DARK, COL_BLUE_LIGHT),
    };

    let body = rect.shrink(5.0);
    painter.rect_filled(body.translate(vec2(2.0, 3.0)), 8.0, Color32::from_black_alpha(70));
    painter.rect_filled(body, 8.0, dark);

    if selected {
        painter.rect_stroke(
            body.expand(1.0),
            8.0,
            Stroke::new(2.0, COL_SELECT_GLOW),
            StrokeKind::Outside,
        );
    }

    let face = body.shrink(3.0);
    if show_rank {
        match piece.rank {
            Rank::Flag => draw_flag_tile(painter, face, main, light),
            Rank::Bomb => draw_bomb_tile(painter, face),
            _ => draw_rank_tile(painter, face, piece.rank, main, light),
        }
    } else {
        draw_hidden_tile(painter, face, main, dark);
    }
}

fn draw_rank_tile(
    painter: &Painter,
    rect: Rect,
    rank: Rank,
    main: Color32,
    light: Color32,
) {
    painter.rect_filled(rect, 6.0, COL_PIECE_FACE);
    painter.rect_stroke(rect, 6.0, Stroke::new(2.0, main), StrokeKind::Outside);
    painter.rect_filled(
        Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + 6.0)),
        4.0,
        light.gamma_multiply(0.45),
    );

    let label = rank.display_str();
    let font_size = if label.len() > 1 { 15.0 } else { 19.0 };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(font_size),
        COL_PIECE_TEXT,
    );
}

fn draw_flag_tile(painter: &Painter, rect: Rect, main: Color32, light: Color32) {
    painter.rect_filled(rect, 6.0, Color32::from_rgb(255, 220, 60));
    painter.rect_stroke(rect, 6.0, Stroke::new(2.5, light), StrokeKind::Outside);
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, main),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        "F",
        FontId::proportional(20.0),
        Color32::from_rgb(120, 80, 10),
    );
}

fn draw_bomb_tile(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 6.0, Color32::from_rgb(58, 58, 62));
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(2.0, Color32::from_rgb(30, 30, 34)),
        StrokeKind::Outside,
    );
    painter.circle_filled(rect.center(), rect.width() * 0.22, Color32::from_rgb(90, 90, 96));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        "B",
        FontId::proportional(17.0),
        Color32::from_rgb(200, 200, 205),
    );
}

fn draw_hidden_tile(painter: &Painter, rect: Rect, main: Color32, dark: Color32) {
    painter.rect_filled(rect, 6.0, COL_HIDDEN_FACE);
    painter.rect_stroke(rect, 6.0, Stroke::new(2.0, dark), StrokeKind::Outside);

    // Diagonal stripe pattern
    let stripe = Stroke::new(1.0, Color32::from_white_alpha(18));
    for i in 0..4 {
        let t = i as f32 * 8.0;
        painter.line_segment(
            [rect.left_bottom() + vec2(t, 0.0), rect.left_top() + vec2(t + 20.0, 0.0)],
            stripe,
        );
    }

    painter.circle_filled(rect.center(), rect.width() * 0.28, main.gamma_multiply(0.85));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        "?",
        FontId::proportional(18.0),
        COL_HIDDEN_TEXT,
    );
}

fn draw_mini_tile(ui: &mut Ui, rank: Rank, player: Player, show_rank: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
    let piece = Piece::new(rank, player);
    draw_piece_tile(ui.painter(), rect, &piece, show_rank, false);
}

fn paint_lake(painter: &Painter, rect: Rect, anim: f32) {
    painter.rect_filled(rect, 4.0, COL_LAKE_DEEP);
    painter.rect_filled(rect.shrink(2.0), 3.0, COL_LAKE);

    let wave = (anim * 2.0).sin() * 2.0;
    painter.rect_filled(
        Rect::from_min_size(rect.min + vec2(4.0, 6.0 + wave), vec2(rect.width() - 8.0, 5.0)),
        2.0,
        COL_LAKE_SHINE.gamma_multiply(0.35),
    );
    painter.rect_filled(
        Rect::from_min_size(rect.min + vec2(8.0, 18.0 - wave), vec2(rect.width() * 0.4, 3.0)),
        2.0,
        COL_LAKE_SHINE.gamma_multiply(0.25),
    );
}

// ─── Menu & widgets ──────────────────────────────────────────────────────────

fn mode_card(ui: &mut Ui, title: &str, desc: &str, accent: Color32) -> bool {
    let resp = Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            accent.r(),
            accent.g(),
            accent.b(),
            22,
        ))
        .stroke(Stroke::new(1.5, accent.gamma_multiply(0.55)))
        .corner_radius(10)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(300.0);
            ui.colored_label(accent, RichText::new(title).size(18.0).strong());
            ui.add_space(4.0);
            ui.colored_label(COL_TEXT_DIM, RichText::new(desc).size(13.0));
        })
        .response;

    let hovered = resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        let painter = ui.ctx().layer_painter(LayerId::new(
            Order::Background,
            resp.id.with("hover"),
        ));
        painter.rect_filled(
            resp.rect.expand(1.0),
            10.0,
            accent.gamma_multiply(0.12),
        );
    }

    resp.interact(Sense::click()).clicked()
}

fn piece_picker_button(
    ui: &mut Ui,
    rank: Rank,
    count: usize,
    selected: bool,
    accent: Color32,
) -> Response {
    let label = format!("{}  ×{}", rank.full_name(), count);
    ui.add_enabled(
        count > 0,
        Button::new(RichText::new(label).size(13.0).color(if count > 0 {
            Color32::WHITE
        } else {
            COL_TEXT_DIM
        }))
        .fill(if selected {
            accent.gamma_multiply(0.38)
        } else {
            Color32::from_rgba_unmultiplied(36, 38, 48, 220)
        })
        .stroke(Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected {
                accent
            } else {
                Color32::from_white_alpha(20)
            },
        ))
        .min_size(vec2(152.0, 32.0)),
    )
}

// ─── Labels & helpers ────────────────────────────────────────────────────────

fn mode_badge(mode: GameMode) -> &'static str {
    match mode {
        GameMode::SoloVsAi => " SOLO ",
        GameMode::Hotseat => " HOTSEAT ",
    }
}

fn player_label(player: Player) -> &'static str {
    match player {
        Player::Red => "Red",
        Player::Blue => "Blue",
    }
}

fn phase_label(game: &GameState, ai_thinking: bool) -> String {
    match &game.phase {
        Phase::Setup(p) => format!("{} — Arrange Army", player_label(*p)),
        Phase::Play if ai_thinking => "Opponent Thinking…".into(),
        Phase::Play if game.mode == GameMode::SoloVsAi && game.current_player == Player::Red => {
            "Your Turn".into()
        }
        Phase::Play => format!("{}'s Turn", player_label(game.current_player)),
        Phase::GameOver(w) => format!("{} Wins!", player_label(*w)),
    }
}

fn rank_tooltip(rank: Rank) -> &'static str {
    match rank {
        Rank::Spy => "Beats Marshal when attacking. Loses to almost everything else.",
        Rank::Scout => "Moves any distance in a straight line.",
        Rank::Miner => "Defuses Bombs.",
        Rank::Bomb => "Immovable. Destroys attackers except Miners.",
        Rank::Flag => "Immovable. Capture it to win.",
        _ => "Higher rank wins in combat. Equal ranks both fall.",
    }
}

fn rank_colors(rank: Rank) -> (Color32, Color32) {
    match rank {
        Rank::Flag => (
            Color32::from_rgb(255, 215, 0),
            Color32::from_rgb(180, 150, 0),
        ),
        Rank::Bomb => (
            Color32::from_rgb(110, 110, 118),
            Color32::from_rgb(50, 50, 55),
        ),
        Rank::Marshal => (
            Color32::from_rgb(230, 70, 50),
            Color32::from_rgb(160, 30, 20),
        ),
        Rank::General => (
            Color32::from_rgb(200, 100, 60),
            Color32::from_rgb(140, 60, 30),
        ),
        Rank::Colonel => (
            Color32::from_rgb(170, 130, 80),
            Color32::from_rgb(120, 90, 50),
        ),
        Rank::Spy => (
            Color32::from_rgb(160, 80, 180),
            Color32::from_rgb(100, 40, 120),
        ),
        _ => (
            Color32::from_rgb(180, 180, 180),
            Color32::from_rgb(120, 120, 120),
        ),
    }
}
