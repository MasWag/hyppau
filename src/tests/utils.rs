use typed_arena::Arena;

use crate::{
    automata::{NFAHState, NFAHTransition, NFAH},
    result_notifier::{MatchingInterval, MatchingResult},
};

/// Helper function to create a standard test automaton with 2 dimensions
pub fn create_small_automaton<'a>(
    state_arena: &'a Arena<NFAHState<'a>>,
    transition_arena: &'a Arena<NFAHTransition<'a>>,
) -> NFAH<'a> {
    let mut automaton = NFAH::new(state_arena, transition_arena, 2);

    // Create states
    let s0 = automaton.add_state(true, false); // Initial state
    let s1 = automaton.add_state(false, false);
    let s2 = automaton.add_state(false, false);
    let s3 = automaton.add_state(false, false);
    let s4 = automaton.add_state(false, true); // Final state

    // Add transitions
    automaton.add_nfah_transition(s0, "a".to_string(), 0, s1); // from: 0, to: 1, label: ["a", 0]
    automaton.add_nfah_transition(s1, "b".to_string(), 1, s2); // from: 1, to: 2, label: ["b", 1]
    automaton.add_nfah_transition(s0, "a".to_string(), 0, s0); // from: 0, to: 0, label: ["a", 0]
    automaton.add_nfah_transition(s0, "b".to_string(), 1, s0); // from: 0, to: 0, label: ["b", 1]
    automaton.add_nfah_transition(s0, "c".to_string(), 0, s3); // from: 0, to: 3, label: ["c", 0]
    automaton.add_nfah_transition(s3, "d".to_string(), 1, s4); // from: 3, to: 4, label: ["d", 1]

    automaton
}

/// Helper function to verify matching results against expected intervals
pub fn verify_intervals(results: &[MatchingResult], expected_intervals: &[Vec<usize>]) {
    assert_eq!(
        results.len(),
        expected_intervals.len(),
        "Number of results doesn't match expected"
    );

    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.intervals.len(), 2, "Result should have 2 intervals");
        assert_eq!(
            result.intervals[0],
            MatchingInterval::new(expected_intervals[i][0], expected_intervals[i][1]),
            "First interval mismatch at result {}",
            i
        );
        assert_eq!(
            result.intervals[1],
            MatchingInterval::new(expected_intervals[i][2], expected_intervals[i][3]),
            "Second interval mismatch at result {}",
            i
        );
    }
}

/// Helper function to verify matching results against expected ids
pub fn verify_ids(results: &[MatchingResult], expected_ids: &[Vec<usize>]) {
    assert_eq!(
        results.len(),
        expected_ids.len(),
        "Number of results doesn't match expected"
    );

    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.ids.len(), 2, "Result should have 2 IDs");
        assert_eq!(
            result.ids[0], expected_ids[i][0],
            "First ID mismatch at result {}",
            i
        );
        assert_eq!(
            result.ids[1], expected_ids[i][1],
            "Second ID mismatch at result {}",
            i
        );
    }
}
