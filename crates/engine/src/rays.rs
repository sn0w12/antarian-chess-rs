use std::sync::OnceLock;

const DIR_OFFSETS: [(i8, i8); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

/// Directions in the same order [`ray`] indexes them.
pub const DIRECTIONS: [(i8, i8); 8] = DIR_OFFSETS;

static RAYS: OnceLock<[[&[u8]; 8]; 64]> = OnceLock::new();

fn build_rays() -> [[&'static [u8]; 8]; 64] {
    let mut rays: [[Vec<u8>; 8]; 64] = std::array::from_fn(|_| std::array::from_fn(|_| Vec::new()));
    for sq in 0..64usize {
        let f = (sq % 8) as i8;
        let r = (sq / 8) as i8;
        for (dir, &(df, dr)) in DIR_OFFSETS.iter().enumerate() {
            let mut v = Vec::with_capacity(7);
            let mut cf = f + df;
            let mut cr = r + dr;
            while cf >= 0 && cf < 8 && cr >= 0 && cr < 8 {
                v.push((cr * 8 + cf) as u8);
                cf += df;
                cr += dr;
            }
            rays[sq][dir] = v;
        }
    }
    rays.map(|dirs| dirs.map(|v| &*Box::leak(v.into_boxed_slice())))
}

fn get_rays() -> &'static [[&'static [u8]; 8]; 64] {
    RAYS.get_or_init(build_rays)
}

/// Direction index used by [`ray`] and [`generate_ray_moves`].
pub fn direction_index(df: i8, dr: i8) -> Option<usize> {
    Some(match (df, dr) {
        (-1, -1) => 0,
        (-1, 0) => 1,
        (-1, 1) => 2,
        (0, -1) => 3,
        (0, 1) => 4,
        (1, -1) => 5,
        (1, 0) => 6,
        (1, 1) => 7,
        _ => return None,
    })
}

/// Squares from `sq` outward in `direction`, nearest first.
pub fn ray(sq: usize, direction: usize) -> &'static [u8] {
    get_rays()[sq][direction]
}

/// Walk a pre-computed ray to generate sliding moves.
/// Returns `true` if a blocker was encountered.
pub fn generate_ray_moves(
    sq: usize,
    direction: usize,
    board: &[Option<(crate::types::Color, crate::types::PieceKind)>; 64],
    color: crate::types::Color,
    captures_only: bool,
    can_jump: bool,
    moves: &mut Vec<crate::types::Move>,
) -> bool {
    let ray = get_rays()[sq][direction];
    for &target in ray {
        let target = target as usize;
        match board[target] {
            None => {
                if !captures_only {
                    moves.push(crate::types::Move::new(sq as u8, target as u8, false));
                }
            }
            Some((pc, _)) => {
                if pc != color {
                    moves.push(crate::types::Move::new(sq as u8, target as u8, true));
                }
                if !can_jump {
                    return true;
                }
            }
        }
    }
    false
}

/// A pre-defined set of leaper offsets for fast move generation.
pub struct LeaperSet {
    offsets: &'static [(i8, i8)],
}

impl LeaperSet {
    pub const fn new(offsets: &'static [(i8, i8)]) -> Self {
        Self { offsets }
    }

    pub fn generate(
        &self,
        sq: usize,
        board: &[Option<(crate::types::Color, crate::types::PieceKind)>; 64],
        color: crate::types::Color,
        captures_only: bool,
        can_jump: bool,
        moves: &mut Vec<crate::types::Move>,
    ) {
        let f = (sq % 8) as i8;
        let r = (sq / 8) as i8;
        for &(df, dr) in self.offsets {
            let tf = f + df;
            let tr = r + dr;
            if tf < 0 || tf >= 8 || tr < 0 || tr >= 8 {
                continue;
            }
            if !can_jump {
                let adf = df.abs();
                let adr = dr.abs();
                if adf > adr {
                    let inter = (r * 8 + (f + df.signum())) as usize;
                    if board[inter].is_some() {
                        continue;
                    }
                } else if adr > adf {
                    let inter = ((r + dr.signum()) * 8 + f) as usize;
                    if board[inter].is_some() {
                        continue;
                    }
                }
            }

            let to = (tr * 8 + tf) as usize;
            match board[to] {
                None => {
                    if !captures_only {
                        moves.push(crate::types::Move::new(sq as u8, to as u8, false));
                    }
                }
                Some((pc, _)) if pc != color => {
                    moves.push(crate::types::Move::new(sq as u8, to as u8, true));
                }
                _ => {}
            }
        }
    }
}
