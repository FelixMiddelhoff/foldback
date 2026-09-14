//! Bisection engine — Phase 0 implements Level 1 (per-tick) only, per
//! foldback-plan.md §6 roadmap. Level 2 (per-entity) and Level 3
//! (per-field) narrow further *within* the tick this level finds, using
//! `EntityHash`/`FieldHash` records the same way — deferred to Phase 2,
//! not designed here, but the tick-first structure below is what they'll
//! build on rather than something they'll need to replace.
//!
//! This is `git bisect`'s algorithm applied to a hash tree instead of
//! commits: the tick range needing no search (peers report every tick, so
//! the first mismatch is found by a single pass, not a binary search) —
//! the actual search, once Level 2/3 exist, is over *what part of the
//! state* diverged within that already-known tick.

use std::collections::BTreeMap;

/// The first tick at which not all reporting peers agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DivergenceTick(pub u64);

/// One peer's hash for one tick — the Level 1 unit of comparison.
#[derive(Debug, Clone, Copy)]
pub struct TickHashRecord {
    pub tick: u64,
    pub peer_id: u16,
    pub hash: u64,
}

/// Scans records tick-ascending and returns the first tick where two
/// peers' hashes for that tick disagree. A tick with only one peer's hash
/// recorded so far can't be compared yet and is skipped, not flagged.
/// Returns `None` if every tick with 2+ reporting peers agrees — "no
/// divergence found," not an error.
pub fn find_first_divergence(records: &[TickHashRecord]) -> Option<DivergenceTick> {
    let mut by_tick: BTreeMap<u64, Vec<(u16, u64)>> = BTreeMap::new();
    for r in records {
        by_tick.entry(r.tick).or_default().push((r.peer_id, r.hash));
    }

    for (tick, peer_hashes) in by_tick {
        if peer_hashes.len() < 2 {
            continue;
        }
        let first_hash = peer_hashes[0].1;
        if peer_hashes.iter().any(|(_, h)| *h != first_hash) {
            return Some(DivergenceTick(tick));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(tick: u64, peer_id: u16, hash: u64) -> TickHashRecord {
        TickHashRecord {
            tick,
            peer_id,
            hash,
        }
    }

    #[test]
    fn no_divergence_when_all_agree() {
        let records = vec![
            rec(0, 0, 100),
            rec(0, 1, 100),
            rec(1, 0, 200),
            rec(1, 1, 200),
            rec(2, 0, 300),
            rec(2, 1, 300),
        ];
        assert_eq!(find_first_divergence(&records), None);
    }

    #[test]
    fn finds_exact_injected_divergence_tick() {
        let records = vec![
            rec(0, 0, 100),
            rec(0, 1, 100),
            rec(1, 0, 200),
            rec(1, 1, 200),
            rec(2, 0, 300),
            rec(2, 1, 999), // injected divergence at tick 2
            rec(3, 0, 400),
            rec(3, 1, 999), // would also diverge, but 2 is first
        ];
        assert_eq!(find_first_divergence(&records), Some(DivergenceTick(2)));
    }

    #[test]
    fn ignores_ticks_with_only_one_reporting_peer() {
        let records = vec![
            rec(0, 0, 100), // only one peer has reported this tick so far
            rec(1, 0, 200),
            rec(1, 1, 200),
        ];
        assert_eq!(find_first_divergence(&records), None);
    }

    #[test]
    fn three_or_more_peers_any_odd_one_out_is_caught() {
        let records = vec![
            rec(5, 0, 42),
            rec(5, 1, 42),
            rec(5, 2, 7), // peer 2 is the odd one out
        ];
        assert_eq!(find_first_divergence(&records), Some(DivergenceTick(5)));
    }

    proptest::proptest! {
        #[test]
        fn single_injected_divergence_is_always_found_and_never_a_different_tick(
            clean_ticks in 1u64..30,
            divergence_at in 0u64..30,
        ) {
            let mut records = Vec::new();
            for t in 0..clean_ticks {
                records.push(rec(t, 0, t * 7 + 1));
                records.push(rec(t, 1, t * 7 + 1));
            }
            if divergence_at < clean_ticks {
                // Corrupt peer 1's hash at exactly one tick.
                for r in records.iter_mut() {
                    if r.tick == divergence_at && r.peer_id == 1 {
                        r.hash = r.hash.wrapping_add(1);
                    }
                }
                let result = find_first_divergence(&records);
                proptest::prop_assert_eq!(result, Some(DivergenceTick(divergence_at)));
            } else {
                // divergence_at out of range: no corruption applied, must stay clean.
                proptest::prop_assert_eq!(find_first_divergence(&records), None);
            }
        }
    }
}
