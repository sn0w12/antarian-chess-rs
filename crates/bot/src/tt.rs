use chess_engine::{Board, Move};
use std::sync::atomic::{AtomicU64, Ordering};

/// Zobrist key for the board, using the cached incremental value.
#[inline]
pub fn zobrist_key(board: &Board) -> u64 {
    board.zobrist
}

#[derive(Debug, Clone)]
pub struct TTEntry {
    pub score: i32,
    pub depth: u32,
    pub flag: TTFlag,
    pub best_move: Option<Move>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TTFlag {
    Exact = 0,
    LowerBound = 1,
    UpperBound = 2,
}

const SLOT_BITS: usize = 23;
const SLOT_COUNT: usize = 1 << SLOT_BITS;
const SCORE_OFFSET: i32 = 1 << 17;
const SIGNATURE_SHIFT: usize = 21;
const MAX_DEPTH: u32 = 63;

/// Direct-mapped table of packed entries. One atomic per slot, so a probe is a
/// single load and writers never block each other. Slot collisions evict; the
/// signature rejects the entries that belong to a different position.
pub struct TranspositionTable {
    slots: Vec<AtomicU64>,
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new()
    }
}

impl TranspositionTable {
    pub fn new() -> Self {
        Self {
            slots: (0..SLOT_COUNT).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    fn slot(&self, key: u64) -> usize {
        key as usize & (SLOT_COUNT - 1)
    }

    fn signature(key: u64) -> u64 {
        (key >> SIGNATURE_SHIFT) & 0x00FF_FFFF
    }

    pub fn get(&self, key: u64) -> Option<TTEntry> {
        let packed = self.slots[self.slot(key)].load(Ordering::Relaxed);
        if packed == 0 || (packed >> 40) != Self::signature(key) {
            return None;
        }
        Some(unpack(packed))
    }

    pub fn store(&self, key: u64, entry: TTEntry) {
        let slot = &self.slots[self.slot(key)];
        let existing = slot.load(Ordering::Relaxed);
        if existing != 0
            && (existing >> 40) == Self::signature(key)
            && depth_of(existing) > entry.depth
        {
            return;
        }
        slot.store(pack(key, &entry), Ordering::Relaxed);
    }

    pub fn clear(&self) {
        for slot in &self.slots {
            slot.store(0, Ordering::Relaxed);
        }
    }
}

fn depth_of(packed: u64) -> u32 {
    ((packed >> 18) & 0x3F) as u32
}

fn pack(key: u64, entry: &TTEntry) -> u64 {
    let score = (entry.score + SCORE_OFFSET).clamp(0, (1 << 18) - 1) as u64;
    let depth = entry.depth.min(MAX_DEPTH) as u64;
    let flag = entry.flag as u64;
    let mv = match entry.best_move {
        Some(mv) => {
            1 | ((mv.from as u64) << 1) | ((mv.to as u64) << 7) | ((mv.capture as u64) << 13)
        }
        None => 0,
    };
    let signature = TranspositionTable::signature(key);

    score | (depth << 18) | (flag << 24) | (mv << 26) | (signature << 40)
}

fn unpack(packed: u64) -> TTEntry {
    let score = (packed & 0x3_FFFF) as i32 - SCORE_OFFSET;
    let depth = ((packed >> 18) & 0x3F) as u32;
    let flag = match (packed >> 24) & 0x3 {
        1 => TTFlag::LowerBound,
        2 => TTFlag::UpperBound,
        _ => TTFlag::Exact,
    };
    let mv = (packed >> 26) & 0x3FFF;
    let best_move = if mv & 1 != 0 {
        Some(Move::new(
            ((mv >> 1) & 0x3F) as u8,
            ((mv >> 7) & 0x3F) as u8,
            (mv >> 13) & 1 != 0,
        ))
    } else {
        None
    };

    TTEntry {
        score,
        depth,
        flag,
        best_move,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_engine::PieceKind;

    #[test]
    fn stores_and_reads_entries() {
        let tt = TranspositionTable::new();
        let mv = Move::new(12, 28, true);
        tt.store(
            0x1234_5678_9ABC_DEF0,
            TTEntry {
                score: -5040,
                depth: 9,
                flag: TTFlag::LowerBound,
                best_move: Some(mv),
            },
        );

        let entry = tt.get(0x1234_5678_9ABC_DEF0).expect("entry");
        assert_eq!(entry.score, -5040);
        assert_eq!(entry.depth, 9);
        assert_eq!(entry.flag, TTFlag::LowerBound);
        assert_eq!(entry.best_move, Some(mv));

        assert!(tt.get(0x1234_5678_9ABC_DEF1).is_none());
    }

    #[test]
    fn keeps_deeper_entries() {
        let tt = TranspositionTable::new();
        let key = 0xDEAD_BEEF_1234_5678;
        tt.store(
            key,
            TTEntry {
                score: 100,
                depth: 7,
                flag: TTFlag::Exact,
                best_move: None,
            },
        );
        tt.store(
            key,
            TTEntry {
                score: 200,
                depth: 3,
                flag: TTFlag::Exact,
                best_move: None,
            },
        );
        assert_eq!(tt.get(key).expect("entry").score, 100);
    }

    #[test]
    fn mate_scores_survive_packing() {
        let tt = TranspositionTable::new();
        for score in [-90_000 + 3, -12_345, 0, 55, 90_000 - 7] {
            let key = 0x0F0F_0F0F_0F0F_0F0F ^ score as u64;
            tt.store(
                key,
                TTEntry {
                    score,
                    depth: 1,
                    flag: TTFlag::Exact,
                    best_move: None,
                },
            );
            assert_eq!(tt.get(key).expect("entry").score, score);
        }
    }

    #[test]
    fn keeps_boards_with_rare_keys_apart() {
        let tt = TranspositionTable::new();
        let mut board = Board::initial();
        board.set(31, Some((chess_engine::Color::White, PieceKind::Empress)));
        let entry = TTEntry {
            score: 42,
            depth: 2,
            flag: TTFlag::Exact,
            best_move: Some(Move::new(1, 2, false)),
        };
        tt.store(board.zobrist, entry);
        let got = tt.get(board.zobrist).expect("entry");
        assert_eq!(got.score, 42);
        assert_eq!(got.best_move, Some(Move::new(1, 2, false)));
    }
}
