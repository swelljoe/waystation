//! A small stirred pseudo-random source.
//!
//! The game wants randomness for who walks down the road, what name they give,
//! what turns up under a mattress, and which passage the book falls open at. It
//! does not want a dependency that has to be taught how to find entropy inside a
//! WebAssembly sandbox, so this keeps its own state and stirs it with the one
//! genuinely unpredictable thing a frame loop has: how long the last frame took.

use bevy::prelude::*;

/// Any odd constant works to start; the frame timings are what make runs differ.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

#[derive(Resource, Clone, Copy, Debug)]
pub struct Chance {
    state: u64,
}

impl Default for Chance {
    fn default() -> Self {
        Self { state: SEED }
    }
}

impl Chance {
    /// Folds one frame's duration into the state. Called every frame, before
    /// anything draws from it.
    pub const fn stir(&mut self, delta_nanos: u128) {
        #[allow(clippy::cast_possible_truncation)]
        let entropy = delta_nanos as u64;
        self.state = self.state.rotate_left(17) ^ entropy.wrapping_mul(SEED);
    }

    const fn next(&mut self) -> u64 {
        // xorshift64*, which is short, has no bad seeds beyond zero, and is far
        // better than anything this game can tell the difference from.
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        if self.state == 0 {
            self.state = SEED;
        }
        self.state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..bound`. Zero for an empty range, so callers that forgot
    /// to check an empty list index nothing rather than panicking on a divide.
    pub const fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        #[allow(clippy::cast_possible_truncation)]
        let draw = (self.next() >> 11) as usize;
        draw % bound
    }

    /// True with the given probability, clamped to `0.0..=1.0`.
    pub fn odds(&mut self, probability: f32) -> bool {
        #[allow(clippy::cast_precision_loss)]
        let roll = (self.next() >> 40) as f32 / 16_777_216.0;
        roll < probability.clamp(0.0, 1.0)
    }

    /// A number somewhere in `low..=high`.
    pub fn between(&mut self, low: f32, high: f32) -> f32 {
        if high <= low {
            return low;
        }
        #[allow(clippy::cast_precision_loss)]
        let roll = (self.next() >> 40) as f32 / 16_777_216.0;
        roll.mul_add(high - low, low)
    }

    /// One of the slice, or `None` if there is nothing to pick from.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        items.get(self.below(items.len()))
    }
}

/// Keeps the source unpredictable. Registered early in the frame so anything
/// drawing this frame draws from state the previous frame's timing touched.
pub fn stir_chance(mut chance: ResMut<Chance>, time: Res<Time>) {
    chance.stir(time.delta().as_nanos());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_stay_inside_their_bound_and_do_not_all_come_up_the_same() {
        let mut chance = Chance::default();
        let mut seen = [0_u32; 7];
        for _ in 0..700 {
            let draw = chance.below(7);
            assert!(draw < 7);
            seen[draw] += 1;
        }
        assert!(
            seen.iter().all(|&count| count > 40),
            "one of seven outcomes barely appeared in 700 draws: {seen:?}"
        );
    }

    #[test]
    fn an_empty_range_yields_zero_rather_than_dividing_by_it() {
        let mut chance = Chance::default();
        assert_eq!(chance.below(0), 0);
        let nothing: [u8; 0] = [];
        assert!(chance.pick(&nothing).is_none());
    }

    #[test]
    fn certain_and_impossible_odds_are_honoured_exactly() {
        let mut chance = Chance::default();
        for _ in 0..200 {
            assert!(chance.odds(1.0));
            assert!(!chance.odds(0.0));
            assert!(!chance.odds(-3.0), "a negative probability is impossible");
        }
    }

    #[test]
    fn a_roughly_even_coin_comes_up_roughly_even() {
        let mut chance = Chance::default();
        let heads = (0..1000).filter(|_| chance.odds(0.5)).count();
        assert!(
            (400..600).contains(&heads),
            "1000 tosses gave {heads} heads"
        );
    }

    #[test]
    fn a_span_stays_within_its_ends_and_a_reversed_span_collapses() {
        let mut chance = Chance::default();
        for _ in 0..200 {
            let draw = chance.between(4.0, 9.0);
            assert!((4.0..=9.0).contains(&draw), "{draw} escaped its span");
        }
        // A span with no width, and a span given the wrong way round, both
        // collapse to their low end rather than producing nonsense.
        assert!((chance.between(5.0, 5.0) - 5.0).abs() < f32::EPSILON);
        assert!((chance.between(9.0, 4.0) - 9.0).abs() < f32::EPSILON);
    }

    #[test]
    fn stirring_changes_what_comes_next() {
        let mut left = Chance::default();
        let mut right = Chance::default();
        right.stir(16_742_318);
        assert_ne!(left.below(10_000), right.below(10_000));
    }
}
