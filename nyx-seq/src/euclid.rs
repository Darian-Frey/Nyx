//! Euclidean rhythm generator.
//!
//! Distributes `hits` onsets as evenly as possible across `steps` slots,
//! using the Bjorklund algorithm. The result is a `Pattern<bool>`.

use crate::pattern::Pattern;

/// Generate a Euclidean rhythm pattern.
///
/// `hits` onsets are distributed as evenly as possible across `steps` slots.
///
/// ```ignore
/// Euclid::new(3, 8)           // [x . . x . . x .] — tresillo
/// Euclid::new(5, 8)           // [x . x x . x x .] — cinquillo
/// Euclid::new(3, 8).rotate(1) // [. x . . x . . x] — shifted tresillo
/// ```
pub struct Euclid;

impl Euclid {
    /// Generate a Euclidean rhythm as a `Pattern<bool>`.
    pub fn generate(hits: usize, steps: usize) -> Pattern<bool> {
        if steps == 0 {
            return Pattern::new(&[]);
        }
        if hits == 0 {
            return Pattern::from_vec(vec![false; steps]);
        }
        if hits >= steps {
            return Pattern::from_vec(vec![true; steps]);
        }

        // Bjorklund algorithm
        let mut pattern = bjorklund(hits, steps);
        pattern.truncate(steps);
        Pattern::from_vec(pattern)
    }
}

/// Bjorklund's algorithm for distributing `k` ones among `n` slots.
fn bjorklund(k: usize, n: usize) -> Vec<bool> {
    let mut groups: Vec<Vec<bool>> = Vec::new();

    // Initial groups: k groups of [true], (n-k) groups of [false]
    for _ in 0..k {
        groups.push(vec![true]);
    }
    for _ in 0..(n - k) {
        groups.push(vec![false]);
    }

    // Iteratively distribute the remainder groups
    loop {
        let ones = k.min(groups.len());
        let zeros = groups.len() - ones;
        if zeros <= 1 || ones == 0 {
            break;
        }

        let remainder_count = zeros.min(ones);
        let mut new_groups = Vec::new();

        // Append one remainder group to each of the first `remainder_count` groups
        for i in 0..remainder_count {
            let tail_idx = groups.len() - 1 - i;
            let mut merged = groups[i].clone();
            merged.extend_from_slice(&groups[tail_idx]);
            new_groups.push(merged);
        }

        // Keep the unmerged middle groups
        for group in groups
            .iter()
            .take(groups.len() - remainder_count)
            .skip(remainder_count)
        {
            new_groups.push(group.clone());
        }

        groups = new_groups;

        if groups.len() <= 2 {
            break;
        }
    }

    groups.into_iter().flatten().collect()
}
