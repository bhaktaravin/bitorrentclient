use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]

pub enum Player {
    Red,
    Blue
}


impl Player {
    pub fn opponent(self) -> Player {
        match self {
            Player::Red => Player::Blue, 
            Player::Blue => Player::Red, 
        }
    }
}



#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rank {
    Spy, 
    Scout, 
    Miner, 
    Sergeant, 
    Lieutenant, 
    Captain, 
    Major, 
    Colonel, 
    General, 
    Marshal, 
    Bomb,
    Flag
}


impl Rank {
    pub fn display_str(self) -> &'static str {
        match self {
            Rank::Spy        => "S",
            Rank::Scout      => "2",
            Rank::Miner      => "3",
            Rank::Sergeant   => "4",
            Rank::Lieutenant => "5",
            Rank::Captain    => "6",
            Rank::Major      => "7",
            Rank::Colonel    => "8",
            Rank::General    => "9",
            Rank::Marshal    => "10",
            Rank::Bomb       => "B",
            Rank::Flag       => "F",
        }
    }
 
    pub fn full_name(self) -> &'static str {
        match self {
            Rank::Spy        => "Spy",
            Rank::Scout      => "Scout",
            Rank::Miner      => "Miner",
            Rank::Sergeant   => "Sergeant",
            Rank::Lieutenant => "Lieutenant",
            Rank::Captain    => "Captain",
            Rank::Major      => "Major",
            Rank::Colonel    => "Colonel",
            Rank::General    => "General",
            Rank::Marshal    => "Marshal",
            Rank::Bomb       => "Bomb",
            Rank::Flag       => "Flag",
        }
    }
 
    pub fn is_movable(self) -> bool {
        !matches!(self, Rank::Bomb | Rank::Flag)
    }
 
    pub fn count_per_player(self) -> usize {
        match self {
            Rank::Flag       => 1,
            Rank::Bomb       => 6,
            Rank::Marshal    => 1,
            Rank::General    => 1,
            Rank::Colonel    => 2,
            Rank::Major      => 3,
            Rank::Captain    => 4,
            Rank::Lieutenant => 4,
            Rank::Sergeant   => 4,
            Rank::Miner      => 5,
            Rank::Scout      => 8,
            Rank::Spy        => 1,
        }
    }
}
 
/// All ranks in setup order
pub const ALL_RANKS: &[Rank] = &[
    Rank::Flag, Rank::Bomb, Rank::Marshal, Rank::General,
    Rank::Colonel, Rank::Major, Rank::Captain, Rank::Lieutenant,
    Rank::Sergeant, Rank::Miner, Rank::Scout, Rank::Spy,
];
 
// ─── Combat resolution ───────────────────────────────────────────────────────
 
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CombatResult {
    AttackerWins,
    DefenderWins,
    BothDie,
}
 
pub fn resolve_combat(attacker: Rank, defender: Rank) -> CombatResult {
    match (attacker, defender) {
        // Spy kills Marshal only when attacking
        (Rank::Spy, Rank::Marshal) => CombatResult::AttackerWins,
        // Miner defuses Bomb
        (Rank::Miner, Rank::Bomb)  => CombatResult::AttackerWins,
        // Flag can't fight
        (_, Rank::Flag)            => CombatResult::AttackerWins,
        // Bomb destroys attacker
        (_, Rank::Bomb)            => CombatResult::DefenderWins,
        // Equal ranks: both die
        (a, d) if a == d           => CombatResult::BothDie,
        // Higher rank wins
        (a, d) if a > d            => CombatResult::AttackerWins,
        _                          => CombatResult::DefenderWins,
    }
}
 
// ─── Board ───────────────────────────────────────────────────────────────────
 
pub const COLS: usize = 10;
pub const ROWS: usize = 10;
 
/// Water squares that cannot be occupied or traversed
pub const LAKES: &[(usize, usize)] = &[
    (2,4),(3,4),(2,5),(3,5),
    (6,4),(7,4),(6,5),(7,5),
];
 
#[derive(Clone, Debug)]
pub struct Piece {
    pub rank: Rank,
    pub player: Player,
    pub revealed: bool,  // becomes true after first combat
}
 
impl Piece {
    pub fn new(rank: Rank, player: Player) -> Self {
        Piece { rank, player, revealed: false }
    }
}
 
#[derive(Clone, Debug)]
pub struct Board {
    pub cells: [[Option<Piece>; COLS]; ROWS],
}
 
impl Board {
    pub fn new() -> Self {
        Board { cells: std::array::from_fn(|_| std::array::from_fn(|_| None)) }
    }
 
    pub fn is_lake(col: usize, row: usize) -> bool {
        LAKES.contains(&(col, row))
    }
 
    pub fn get(&self, col: usize, row: usize) -> Option<&Piece> {
        self.cells[row][col].as_ref()
    }
 
    pub fn get_mut(&mut self, col: usize, row: usize) -> Option<&mut Piece> {
        self.cells[row][col].as_mut()
    }
 
    pub fn set(&mut self, col: usize, row: usize, piece: Option<Piece>) {
        self.cells[row][col] = piece;
    }
 
    /// Legal moves for the piece at (col, row)
    pub fn legal_moves(&self, col: usize, row: usize) -> Vec<(usize, usize)> {
        let piece = match self.get(col, row) {
            Some(p) if p.rank.is_movable() => p,
            _ => return vec![],
        };
        let is_scout = piece.rank == Rank::Scout;
        let player = piece.player;
        let mut moves = Vec::new();
 
        let directions: &[(i32, i32)] = &[(0,1),(0,-1),(1,0),(-1,0)];
        for &(dc, dr) in directions {
            let mut c = col as i32 + dc;
            let mut r = row as i32 + dr;
            loop {
                if c < 0 || c >= COLS as i32 || r < 0 || r >= ROWS as i32 { break; }
                let (uc, ur) = (c as usize, r as usize);
                if Self::is_lake(uc, ur) { break; }
                match self.get(uc, ur) {
                    None => {
                        moves.push((uc, ur));
                        if !is_scout { break; }
                    }
                    Some(target) => {
                        if target.player != player {
                            moves.push((uc, ur)); // can attack
                        }
                        break; // blocked
                    }
                }
                c += dc; r += dr;
            }
        }
        moves
    }
}
 
// ─── Game mode ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    SoloVsAi,
    Hotseat,
}

// ─── Game state machine ──────────────────────────────────────────────────────
 
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Setup(Player),   // placement phase for given player
    Play,
    GameOver(Player), // winner
}
 
#[derive(Debug, Clone)]
pub struct LastMove {
    pub from: (usize, usize),
    pub to: (usize, usize),
    pub attacker_rank: Option<Rank>,
    pub defender_rank: Option<Rank>,
    pub result: Option<CombatResult>,
}
 
pub struct GameState {
    pub mode: GameMode,
    pub board: Board,
    pub phase: Phase,
    pub current_player: Player,
    pub selected: Option<(usize, usize)>,
    pub highlights: Vec<(usize, usize)>,
    pub message: String,
    pub last_move: Option<LastMove>,
    pub setup_inventory: HashMap<Rank, usize>,
    pub captured_red: Vec<Rank>,
    pub captured_blue: Vec<Rank>,
}

impl GameState {
    pub fn new(mode: GameMode) -> Self {
        GameState {
            mode,
            board: Board::new(),
            phase: Phase::Setup(Player::Red),
            current_player: Player::Red,
            selected: None,
            highlights: vec![],
            message: "Pick a piece on the left, then place it on rows 1–4 (bottom zone).".into(),
            last_move: None,
            setup_inventory: Self::full_inventory(),
            captured_red: vec![],
            captured_blue: vec![],
        }
    }

    pub fn is_ai_player(&self, player: Player) -> bool {
        self.mode == GameMode::SoloVsAi && player == Player::Blue
    }

    pub fn is_ai_turn(&self) -> bool {
        self.phase == Phase::Play && self.is_ai_player(self.current_player)
    }

    /// Whose pieces are fully visible on the board (fog-of-war perspective).
    pub fn perspective_player(&self) -> Player {
        match self.mode {
            GameMode::SoloVsAi => Player::Red,
            GameMode::Hotseat => self.current_player,
        }
    }

    pub fn can_see_rank(&self, piece: &Piece) -> bool {
        match self.phase {
            Phase::Setup(p) => piece.player == p,
            Phase::Play | Phase::GameOver(_) => {
                piece.revealed || piece.player == self.perspective_player()
            }
        }
    }
 
    fn full_inventory() -> HashMap<Rank, usize> {
        ALL_RANKS.iter().map(|&r| (r, r.count_per_player())).collect()
    }
 
    /// Auto-place Blue pieces using the default AI layout.
    pub fn place_blue_auto(&mut self) {
        for &(col, row, rank) in &crate::ai::blue_setup_layout() {
            self.board.set(col, row, Some(Piece::new(rank, Player::Blue)));
        }
    }

    fn finish_setup_for(&mut self, player: Player) {
        match player {
            Player::Red => match self.mode {
                GameMode::SoloVsAi => {
                    self.place_blue_auto();
                    self.phase = Phase::Play;
                    self.current_player = Player::Red;
                    self.message = "The battle begins — you move first!".into();
                }
                GameMode::Hotseat => {
                    self.setup_inventory = Self::full_inventory();
                    self.phase = Phase::Setup(Player::Blue);
                    self.message =
                        "Red is set. Blue — arrange your army on rows 7–10 (top). Pass the screen!"
                            .into();
                }
            },
            Player::Blue => {
                self.phase = Phase::Play;
                self.current_player = Player::Red;
                self.message = "The battle begins — Red moves first!".into();
            }
        }
    }

    pub fn setup_rows_for(player: Player) -> std::ops::Range<usize> {
        match player {
            Player::Red => 6..10,
            Player::Blue => 0..4,
        }
    }
 
    pub fn remaining_pieces(&self) -> usize {
        self.setup_inventory.values().sum()
    }
 
    pub fn total_pieces() -> usize {
        ALL_RANKS.iter().map(|r| r.count_per_player()).sum()
    }
 
    // ── Setup phase: place a piece ──────────────────────────────────────────
 
    pub fn try_place(&mut self, rank: Rank, col: usize, row: usize) -> bool {
        let Phase::Setup(player) = self.phase else { return false; };
        let valid_rows = Self::setup_rows_for(player);
        if !valid_rows.contains(&row) {
            self.message = format!(
                "Place pieces only on rows {} (your side).",
                setup_row_labels(player)
            );
            return false;
        }
        if Board::is_lake(col, row) {
            self.message = "Can't place pieces on the lakes.".into();
            return false;
        }
        let remaining = *self.setup_inventory.get(&rank).unwrap_or(&0);
        if remaining == 0 {
            self.message = format!("No more {} pieces to place.", rank.full_name());
            return false;
        }
        // Return piece if cell was occupied
        if let Some(old) = self.board.get(col, row) {
            let old_rank = old.rank;
            *self.setup_inventory.get_mut(&old_rank).unwrap() += 1;
        }
        *self.setup_inventory.get_mut(&rank).unwrap() -= 1;
        self.board.set(col, row, Some(Piece::new(rank, player)));
        if self.remaining_pieces() == 0 {
            self.finish_setup_for(player);
        } else {
            self.message = format!(
                "Placed {} at {} — {} remaining.",
                rank.full_name(),
                format_cell(col, row),
                self.remaining_pieces()
            );
        }
        true
    }

    pub fn make_ai_move(&mut self) {
        if !self.is_ai_turn() {
            return;
        }
        if let Some(((fc, fr), (tc, tr))) = crate::ai::choose_move(&self.board, self.current_player)
        {
            self.do_move(fc, fr, tc, tr);
            if self.phase == Phase::Play && self.message.is_empty() {
                self.message = "Blue moved.".into();
            }
        } else {
            self.message = "Blue has no legal moves.".into();
        }
    }
 
    // ── Play phase ──────────────────────────────────────────────────────────
 
    pub fn click_cell(&mut self, col: usize, row: usize) {
        if self.phase != Phase::Play || self.is_ai_turn() {
            return;
        }
 
        // If a piece is selected, try to move/attack
        if let Some((sc, sr)) = self.selected {
            if self.highlights.contains(&(col, row)) {
                self.do_move(sc, sr, col, row);
                return;
            }
        }
 
        // Select a friendly piece
        if let Some(piece) = self.board.get(col, row) {
            if piece.player == self.current_player && piece.rank.is_movable() {
                let moves = self.board.legal_moves(col, row);
                if moves.is_empty() {
                    self.message = format!(
                        "Your {} at {} has nowhere to go.",
                        piece.rank.full_name(),
                        format_cell(col, row)
                    );
                    self.selected = None;
                    self.highlights.clear();
                } else {
                    let hint = if piece.rank == Rank::Scout && moves.len() > 1 {
                        format!(" Scouts move in straight lines.")
                    } else {
                        String::new()
                    };
                    self.message = format!(
                        "{} at {} — {} destination{}.{}",
                        piece.rank.full_name(),
                        format_cell(col, row),
                        moves.len(),
                        if moves.len() == 1 { "" } else { "s" },
                        hint
                    );
                    self.selected = Some((col, row));
                    self.highlights = moves;
                }
            } else {
                self.selected = None;
                self.highlights.clear();
                self.message = "Choose one of your pieces to move.".into();
            }
        } else {
            self.selected = None;
            self.highlights.clear();
        }
    }
 
    fn do_move(&mut self, from_c: usize, from_r: usize, to_c: usize, to_r: usize) {
        let attacker = self.board.cells[from_r][from_c].take().unwrap();
 
        let mut last_move = LastMove {
            from: (from_c, from_r),
            to: (to_c, to_r),
            attacker_rank: Some(attacker.rank),
            defender_rank: None,
            result: None,
        };
 
        if let Some(defender) = self.board.cells[to_r][to_c].take() {
            last_move.defender_rank = Some(defender.rank);
            let result = resolve_combat(attacker.rank, defender.rank);
            last_move.result = Some(result.clone());
 
            match result {
                CombatResult::AttackerWins => {
                    if defender.rank == Rank::Flag {
                        self.board.set(to_c, to_r, Some(Piece { revealed: true, ..attacker }));
                        self.phase = Phase::GameOver(self.current_player);
                        self.message = format!(
                            "Flag captured at {} — {:?} wins!",
                            format_cell(to_c, to_r),
                            self.current_player
                        );
                    } else {
                        let mut winner = attacker;
                        winner.revealed = true;
                        self.board.set(to_c, to_r, Some(winner));
                        self.add_captured(defender.player, defender.rank);
                        self.message = format!(
                            "{} takes {} at {}!",
                            last_move.attacker_rank.unwrap().full_name(),
                            defender.rank.full_name(),
                            format_cell(to_c, to_r)
                        );
                    }
                }
                CombatResult::DefenderWins => {
                    let defender_name = defender.rank.full_name();
                    let mut winner = defender;
                    winner.revealed = true;
                    self.board.set(to_c, to_r, Some(winner));
                    self.add_captured(attacker.player, attacker.rank);
                    self.message = format!(
                        "{} holds at {} — your {} is lost.",
                        defender_name,
                        format_cell(to_c, to_r),
                        last_move.attacker_rank.unwrap().full_name()
                    );
                }
                CombatResult::BothDie => {
                    self.add_captured(attacker.player, attacker.rank);
                    self.add_captured(defender.player, defender.rank);
                    self.message = format!(
                        "Mutual destruction at {} — both pieces removed.",
                        format_cell(to_c, to_r)
                    );
                }
            }
        } else {
            let rank_name = attacker.rank.full_name();
            self.board.set(to_c, to_r, Some(attacker));
            self.message = format!("Moved {} to {}.", rank_name, format_cell(to_c, to_r));
        }
 
        self.last_move = Some(last_move);
        self.selected = None;
        self.highlights.clear();
 
        if self.phase == Phase::Play {
            self.current_player = self.current_player.opponent();
        }
    }
 
    fn add_captured(&mut self, player: Player, rank: Rank) {
        match player {
            Player::Red  => self.captured_red.push(rank),
            Player::Blue => self.captured_blue.push(rank),
        }
    }
 
    pub fn reset(&mut self) {
        let mode = self.mode;
        *self = GameState::new(mode);
    }
}

pub fn format_cell(col: usize, row: usize) -> String {
    format!("{}{}", (b'A' + col as u8) as char, ROWS - row)
}

fn setup_row_labels(player: Player) -> String {
    let range = GameState::setup_rows_for(player);
    format!("{}–{}", ROWS - (range.end - 1), ROWS - range.start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_can_place_on_bottom_rows() {
        let mut game = GameState::new(GameMode::SoloVsAi);
        assert!(game.try_place(Rank::Flag, 0, 9));
        assert!(game.board.get(0, 9).is_some());
        assert_eq!(game.remaining_pieces(), 39);
    }

    #[test]
    fn red_cannot_place_on_blue_rows() {
        let mut game = GameState::new(GameMode::SoloVsAi);
        assert!(!game.try_place(Rank::Flag, 0, 0));
        assert!(game.board.get(0, 0).is_none());
    }
}