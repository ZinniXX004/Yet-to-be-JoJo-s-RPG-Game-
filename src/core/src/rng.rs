//! Deterministic pseudo-random number generation.
//!
//! SplitMix64. Chosen because it is a handful of integer operations with no
//! platform-dependent behavior, has a well understood period of 2^64, and its
//! entire state is a single `u64` that serializes trivially as part of
//! [`crate::state::BattleState`].
//!
//! Deliberately absent: any float API. Float rounding differs across
//! architectures and optimization levels, which would silently break replay
//! determinism.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Current internal state. Exposed so a battle can be snapshotted and
    /// resumed bit-for-bit.
    pub fn state(&self) -> u64 {
        self.state
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `0..n`. Returns 0 when `n == 0` rather than panicking,
    /// because an empty target list is a legal game state, not a bug.
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % u64::from(n)) as u32
    }

    /// Inclusive integer range. Tolerates an inverted range by clamping.
    pub fn range_i32(&mut self, low: i32, high: i32) -> i32 {
        if high <= low {
            return low;
        }
        let span = (high - low + 1) as u32;
        low + self.below(span) as i32
    }

    /// `percent` in 0..=100.
    pub fn chance(&mut self, percent: i32) -> bool {
        if percent <= 0 {
            return false;
        }
        if percent >= 100 {
            return true;
        }
        (self.below(100) as i32) < percent
    }

    pub fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        if items.is_empty() {
            return None;
        }
        let index = self.below(items.len() as u32) as usize;
        Some(items[index])
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(12345);
        let mut b = Rng::new(12345);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn range_is_inclusive_and_bounded() {
        let mut rng = Rng::new(7);
        for _ in 0..10_000 {
            let value = rng.range_i32(90, 110);
            assert!((90..=110).contains(&value));
        }
        assert_eq!(rng.range_i32(5, 5), 5);
        assert_eq!(rng.range_i32(5, 1), 5);
    }

    #[test]
    fn chance_bounds_are_absolute() {
        let mut rng = Rng::new(99);
        assert!(!rng.chance(0));
        assert!(!rng.chance(-40));
        assert!(rng.chance(100));
        assert!(rng.chance(250));
    }

    /// Known answers of the reference SplitMix64 (Vigna's `splitmix64.c`),
    /// computed outside this crate. `same_seed_same_sequence` compares two
    /// generators of the same build, so it cannot notice that the generator
    /// itself changed; every recorded seed would then replay a different
    /// battle. These values can.
    #[test]
    fn the_generator_is_reference_splitmix64() {
        let mut rng = Rng::new(0);
        assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
        assert_eq!(rng.next_u64(), 0xF88B_B8A8_724C_81EC);

        let mut rng = Rng::new(12345);
        assert_eq!(rng.next_u64(), 0x2211_8258_A9D1_11A0);
        assert_eq!(rng.next_u64(), 0x346E_DCE5_F713_F8ED);
        assert_eq!(rng.next_u64(), 0x1E9A_57BC_80E6_721D);
        assert_eq!(rng.next_u64(), 0x2D16_0E7E_5C3F_42CA);
    }

    /// `range_is_inclusive_and_bounded` only checks that values stay inside
    /// the range, which a span one short also satisfies. A twin generator
    /// pins the draw itself: one `below(span)` with span `high - low + 1`.
    #[test]
    fn a_range_is_one_draw_over_its_whole_inclusive_span() {
        let mut ranged = Rng::new(7);
        let mut twin = Rng::new(7);
        let mut seen_low = false;
        let mut seen_high = false;
        for _ in 0..2_000 {
            let value = ranged.range_i32(90, 110);
            assert_eq!(value, 90 + twin.below(21) as i32);
            seen_low |= value == 90;
            seen_high |= value == 110;
        }
        assert!(seen_low && seen_high, "both ends must be reachable");
    }

    /// `chance(p)` succeeds on exactly `p` of the hundred outcomes of
    /// `below(100)`, so every percentage in the content means what it says.
    #[test]
    fn chance_succeeds_on_exactly_percent_of_a_hundred_outcomes() {
        let mut rolled = Rng::new(99);
        let mut twin = Rng::new(99);
        for percent in 1..100 {
            for _ in 0..20 {
                let expected = (twin.below(100) as i32) < percent;
                assert_eq!(rolled.chance(percent), expected, "chance({percent})");
            }
        }
    }
}
