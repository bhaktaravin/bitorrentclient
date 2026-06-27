use crate::game::*;

/// Default Blue setup layout (Blue sits on rows 0–3, back row = row 0).
pub fn blue_setup_layout() -> Vec<(usize, usize, Rank)> {
    let rows: &[&[Rank]] = &[
        &[
            Rank::Flag, Rank::Bomb, Rank::Bomb, Rank::Bomb, Rank::Bomb,
            Rank::Bomb, Rank::Bomb, Rank::Marshal, Rank::General, Rank::Colonel,
        ],
        &[
            Rank::Colonel, Rank::Major, Rank::Major, Rank::Major,
            Rank::Captain, Rank::Captain, Rank::Captain, Rank::Captain,
            Rank::Lieutenant, Rank::Lieutenant,
        ],
        &[
            Rank::Lieutenant, Rank::Lieutenant, Rank::Sergeant, Rank::Sergeant,
            Rank::Sergeant, Rank::Sergeant, Rank::Miner, Rank::Miner,
            Rank::Miner, Rank::Miner,
        ],
        &[
            Rank::Miner, Rank::Scout, Rank::Scout, Rank::Scout,
            Rank::Scout, Rank::Scout, Rank::Scout, Rank::Scout,
            Rank::Scout, Rank::Spy,
        ],
    ];
    let mut placements = Vec::new();
    for (ri, row_ranks) in rows.iter().enumerate() {
        let row = ri;
        for (col, &rank) in row_ranks.iter().enumerate() {
            placements.push((col, row, rank));
        }
    }
    placements
}

fn rank_strength(rank: Rank) -> i32 {
    match rank {
        Rank::Flag => 0,
        Rank::Spy => 1,
        Rank::Scout => 2,
        Rank::Miner => 3,
        Rank::Sergeant => 4,
        Rank::Lieutenant => 5,
        Rank::Captain => 6,
        Rank::Major => 7,
        Rank::Colonel => 8,
        Rank::General => 9,
        Rank::Marshal => 10,
        Rank::Bomb => 11,
    }
}

fn score_move(board: &Board, player: Player, from: (usize, usize), to: (usize, usize), rank: Rank) -> i32 {
    let (fc, fr) = from;
    let (tc, tr) = to;
    let mut score = 0;

    if let Some(target) = board.get(tc, tr) {
        if target.revealed {
            match resolve_combat(rank, target.rank) {
                CombatResult::AttackerWins => {
                    score += rank_strength(target.rank) * 15 + 40;
                    if target.rank == Rank::Flag {
                        score += 10_000;
                    }
                }
                CombatResult::BothDie => score += rank_strength(target.rank) * 5 - rank_strength(rank) * 3,
                CombatResult::DefenderWins => score -= rank_strength(rank) * 20,
            }
        } else {
            // Probe unknown pieces with cheap units; miners may hit bombs, spies may hit marshal.
            score += 25 - rank_strength(rank) * 2;
            if rank == Rank::Miner {
                score += 18;
            }
            if rank == Rank::Spy {
                score += 12;
            }
            if rank == Rank::Scout {
                score += 8;
            }
            if rank == Rank::Marshal || rank == Rank::General {
                score -= 30;
            }
        }
    } else {
        // Advance toward enemy territory.
        let advance = match player {
            Player::Blue => tr as i32 - fr as i32,
            Player::Red => fr as i32 - tr as i32,
        };
        if advance > 0 {
            score += advance * 8;
        }

        if rank == Rank::Scout {
            let dist = (fc as i32 - tc as i32).unsigned_abs() as i32 + advance.max(0);
            score += dist * 3;
        }

        // Keep flag and bombs near the back row in early game.
        if rank == Rank::Flag || rank == Rank::Bomb {
            let back = match player {
                Player::Blue => fr as i32,
                Player::Red => 9 - fr as i32,
            };
            score -= advance.max(0) * 15;
            score += back * 2;
        }
    }

    // Small random tie-breaker so games aren't identical every time.
    score += ((fc * 7 + fr * 13 + tc * 3 + tr * 11) % 5) as i32;
    score
}

/// Pick the best legal move for `player`, if any.
pub fn choose_move(board: &Board, player: Player) -> Option<((usize, usize), (usize, usize))> {
    let mut best: Option<((usize, usize), (usize, usize), i32)> = None;

    for row in 0..ROWS {
        for col in 0..COLS {
            if board.get(col, row).is_some_and(|p| p.player == player && p.rank.is_movable()) {
                let rank = board.get(col, row).unwrap().rank;
                for moveto in board.legal_moves(col, row) {
                    let s = score_move(board, player, (col, row), moveto, rank);
                    if best.as_ref().is_none_or(|(_, _, bs)| s > *bs) {
                        best = Some(((col, row), moveto, s));
                    }
                }
            }
        }
    }

    best.map(|(from, to, _)| (from, to))
}
