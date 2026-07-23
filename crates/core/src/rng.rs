//! Grouped seeded-RNG configuration (issue #49) — one place to configure every
//! independently-seeded RNG source the game uses.
//!
//! [`Seeds`] holds four `u64` seeds (never a live RNG) and materializes a fresh
//! RNG per source on demand, mirroring the existing
//! `GenParams::rng()`/`generation_rng()` pattern (`crates/gen/src/lib.rs`). This
//! is the shared home for all four sources — car collision (`gp-core`
//! `sim::collision`), track generation (`gp-gen`), and the two AI sources
//! (`gp-ai`, a stub today) — because `gp-core` is the one crate every consumer
//! already depends on.
//!
//! Each source materializes a **purpose-fit** engine (issue #139):
//! [`Xoshiro256PlusPlus`] for track generation, collision, and AI inference —
//! fast and statistically sufficient for those uses — and [`ChaCha8Rng`] for AI
//! learning, whose training benefits from `ChaCha8`'s ideal statistics and long
//! period. Both engines are seedable and deterministic, so replay determinism
//! (`docs/design.md` §Ф1, §N4) holds on every source.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rand_xoshiro::Xoshiro256PlusPlus;

/// Four independently-seeded RNG sources, grouped for one-place UI
/// configuration (AC7).
///
/// Each field is a plain `u64` seed, not a live RNG — a fresh RNG is
/// materialized per source on demand via the `*_rng()` accessors, so the same
/// `Seeds` value can be reused to replay any source's stream from the start.
/// `Default` is all-zero seeds, a valid (if unexciting) UI starting point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Seeds {
    /// Seeds car-collision resolution (`gp-core` `sim::collision::resolve_collisions`).
    pub collision: u64,
    /// Seeds the track-generation pipeline (`gp-gen`).
    pub generation: u64,
    /// Seeds AI learning/training. Defined but unconsumed — `gp-ai` is a stub
    /// (AC9).
    pub ai_learning: u64,
    /// Seeds AI inference (in-race decision making). Defined but unconsumed —
    /// `gp-ai` is a stub (AC9).
    pub ai_inference: u64,
}

impl Seeds {
    /// A fresh, replay-deterministic RNG for car-collision resolution, seeded
    /// from `self.collision`. Uses [`Xoshiro256PlusPlus`] — fast and
    /// statistically sufficient for game-logic tie-breaks (issue #139).
    #[inline]
    pub fn collision_rng(&self) -> Xoshiro256PlusPlus {
        Xoshiro256PlusPlus::seed_from_u64(self.collision)
    }

    /// A fresh, replay-deterministic RNG for the track-generation pipeline,
    /// seeded from `self.generation`. Uses [`Xoshiro256PlusPlus`] — fast and
    /// statistically sufficient for procedural track generation (issue #139).
    #[inline]
    pub fn generation_rng(&self) -> Xoshiro256PlusPlus {
        Xoshiro256PlusPlus::seed_from_u64(self.generation)
    }

    /// A fresh, replay-deterministic RNG for AI learning, seeded from
    /// `self.ai_learning`. Uses [`ChaCha8Rng`] — its ideal statistics and long
    /// period best fit FNN training (issue #139). Constructible/reachable today
    /// with no consumer (AC9) — `gp-ai` is a stub.
    #[inline]
    pub fn ai_learning_rng(&self) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(self.ai_learning)
    }

    /// A fresh, replay-deterministic RNG for AI inference, seeded from
    /// `self.ai_inference`. Uses [`Xoshiro256PlusPlus`] — fast and
    /// statistically sufficient for sampling over a handful of logits (issue
    /// #139). Constructible/reachable today with no consumer (AC9) — `gp-ai` is
    /// a stub.
    #[inline]
    pub fn ai_inference_rng(&self) -> Xoshiro256PlusPlus {
        Xoshiro256PlusPlus::seed_from_u64(self.ai_inference)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    /// A `Seeds` with all four fields set explicitly.
    const fn seeds(collision: u64, generation: u64, ai_learning: u64, ai_inference: u64) -> Seeds {
        Seeds {
            collision,
            generation,
            ai_learning,
            ai_inference,
        }
    }

    fn stream(mut rng: impl Rng) -> Vec<u64> {
        (0..8).map(|_| rng.next_u64()).collect()
    }

    #[test]
    fn same_seed_same_source_yields_identical_stream() {
        // AC7: same seed -> identical stream per source.
        let a = seeds(1, 2, 3, 4);
        let b = seeds(1, 20, 30, 40);
        assert_eq!(stream(a.collision_rng()), stream(b.collision_rng()));
    }

    #[test]
    fn distinct_seeds_yield_independent_streams() {
        // AC7: the four sources are independently seeded — a distinct seed per
        // field yields four distinct streams.
        let s = seeds(1, 2, 3, 4);
        let streams = [
            stream(s.collision_rng()),
            stream(s.generation_rng()),
            stream(s.ai_learning_rng()),
            stream(s.ai_inference_rng()),
        ];
        for i in 0..streams.len() {
            for j in (i + 1)..streams.len() {
                assert_ne!(streams[i], streams[j], "sources {i} and {j} collided");
            }
        }
    }

    #[test]
    fn shared_seed_value_across_two_fields_yields_identical_streams() {
        // AC7: a shared seed value in two fields yields identical streams,
        // confirming per-field seeding (not some cross-field mixing).
        let s = seeds(7, 7, 1, 2);
        assert_eq!(stream(s.collision_rng()), stream(s.generation_rng()));
    }

    #[test]
    fn ai_sources_are_constructible_and_reachable_without_a_consumer() {
        // AC9: both AI sources are callable on a Default value and on a
        // field-set value, and produce a stream, with no gp-ai consumer.
        let default_seeds = Seeds::default();
        assert_eq!(stream(default_seeds.ai_learning_rng()).len(), 8);
        assert_eq!(stream(default_seeds.ai_inference_rng()).len(), 8);

        let set_seeds = seeds(1, 2, 3, 4);
        assert_eq!(
            stream(set_seeds.ai_learning_rng()),
            stream(seeds(9, 9, 3, 9).ai_learning_rng())
        );
        assert_ne!(
            stream(set_seeds.ai_learning_rng()),
            stream(set_seeds.ai_inference_rng())
        );
    }

    #[test]
    fn default_is_all_zero_seeds() {
        assert_eq!(Seeds::default(), seeds(0, 0, 0, 0));
    }
}
