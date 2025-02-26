use crate::dfa::DFA;
use std::{collections::HashMap, hash::Hash};

/// A pattern matcher that tracks the earliest starting position of a match
/// using a deterministic finite automaton (DFA).
///
/// This matcher maintains the number of input actions processed (stored in `len`)
/// and a configuration that maps DFA states to the optional starting position
/// (i.e. the input index) when that state was activated. It is designed to efficiently
/// determine the earliest occurrence of a pattern as input actions are fed.
pub struct DFAEarliestPatternMatcher<'a, S, A> {
    dfa: &'a DFA<S, A>,
    /// The number of input actions processed so far.
    pub len: usize,
    /// A mapping from DFA states to the starting input index (if any) when that state became active.
    current_configuration: HashMap<&'a S, Option<usize>>,
}

impl<'a, S, A> DFAEarliestPatternMatcher<'a, S, A>
where
    S: Eq + Hash + Clone,
    A: Eq + Hash + Clone,
{
    /// Creates a new `DFAEarliestPatternMatcher` for the provided DFA.
    ///
    /// The matcher starts with an empty configuration and a length of zero.
    ///
    /// # Arguments
    ///
    /// * `dfa` - A reference to the DFA that defines the pattern.
    ///
    /// # Returns
    ///
    /// A new instance of `DFAEarliestPatternMatcher`.
    fn new(dfa: &'a DFA<S, A>) -> Self {
        Self {
            dfa,
            len: 0,
            current_configuration: HashMap::new(),
        }
    }

    /// Feeds an input action to the matcher and updates the internal DFA configuration.
    ///
    /// This method uses the current `len` (the count of previously processed actions) as
    /// the starting index when activating the DFA's initial state (if not already active).
    /// It then computes the new configuration by following the DFA's transitions for the
    /// given action. When a state is reached by multiple paths, the earliest starting index
    /// is retained. Finally, the length is incremented.
    ///
    /// # Arguments
    ///
    /// * `action` - A reference to the input action to process.
    fn feed(&mut self, action: &A) {
        // Activate the initial state with the current input index if it isn't already active.
        self.current_configuration
            .entry(&self.dfa.initial)
            .or_insert(Some(self.len));

        let mut next_configuration = HashMap::new();
        for (&state, &start_position_option) in &self.current_configuration {
            if let Some(start_position) = start_position_option {
                let successor = &self.dfa.transitions[&(state.clone(), action.clone())];
                next_configuration
                    .entry(successor)
                    .and_modify(|existing: &mut Option<usize>| {
                        if let Some(existing_pos) = existing {
                            if start_position < *existing_pos {
                                *existing = Some(start_position);
                            }
                        }
                    })
                    .or_insert(Some(start_position));
            }
        }
        self.current_configuration = next_configuration;
        self.len += 1;
    }

    /// Returns the earliest starting input index of any active match that reaches a final state.
    ///
    /// This method inspects all final states defined in the DFA and returns the minimum starting
    /// index among those that are currently active.
    ///
    /// # Returns
    ///
    /// * `Some(usize)` if a final state is active, representing the earliest starting index.
    /// * `None` if no final state is active.
    fn current_matching(&self) -> Option<usize> {
        self.dfa
            .finals
            .iter()
            .filter_map(|state| self.current_configuration.get(state).copied().flatten())
            .min()
    }

    /// Returns the earliest starting input index among all active states.
    ///
    /// This method scans the configuration for all active states (regardless of whether they are
    /// final or not) and returns the minimum starting index.
    ///
    /// # Returns
    ///
    /// * `Some(usize)` if there is at least one active state.
    /// * `None` if no states are active.
    fn earliest_starting_position(&self) -> Option<usize> {
        self.current_configuration.values().copied().flatten().min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    /// A simple enum representing the states in a dummy DFA.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum State {
        Q0,
        Q1,
        Q2,
        Q3,
        Qd,
    }

    /// A dummy DFA used for testing purposes.
    ///
    /// This DFA is configured to recognize the pattern "ab". It contains:
    /// - `Q0`: the initial state.
    /// - `Q2`: the only final state.
    /// - `Qd`: a dead state.
    #[derive(Debug)]
    struct DummyDFA {
        pub states: HashSet<State>,
        pub initial: State,
        pub finals: HashSet<State>,
        pub transitions: HashMap<(State, char), State>,
    }

    impl DummyDFA {
        /// Creates a new dummy DFA configured to recognize the pattern "ab".
        ///
        /// # Returns
        ///
        /// A new instance of `DummyDFA`.
        fn new() -> Self {
            let states = vec![State::Q0, State::Q1, State::Q2, State::Qd]
                .into_iter()
                .collect();
            let initial = State::Q0;
            let finals = vec![State::Q2].into_iter().collect();
            let mut transitions = HashMap::new();
            // Transitions for state Q0.
            transitions.insert((State::Q0, 'a'), State::Q1);
            transitions.insert((State::Q0, 'b'), State::Qd);
            // Transitions for state Q1.
            transitions.insert((State::Q1, 'a'), State::Qd);
            transitions.insert((State::Q1, 'b'), State::Q2);
            // Transitions for state Q2 (final state).
            transitions.insert((State::Q2, 'a'), State::Qd);
            transitions.insert((State::Q2, 'b'), State::Qd);
            // Transitions for the dead state Qd.
            transitions.insert((State::Qd, 'a'), State::Qd);
            transitions.insert((State::Qd, 'b'), State::Qd);

            DummyDFA {
                states,
                initial,
                finals,
                transitions,
            }
        }
    }

    impl DummyDFA {
        /// Converts the dummy DFA into the `DFA` type expected by the matcher.
        fn as_dfa(&self) -> DFA<State, char> {
            DFA {
                states: self.states.clone(),
                alphabet: vec!['a', 'b'].into_iter().collect(),
                initial: self.initial.clone(),
                finals: self.finals.clone(),
                transitions: self.transitions.clone(),
            }
        }
    }

    /// Tests that the matcher returns `None` for both matching methods when no input is fed.
    #[test]
    fn test_no_feed() {
        let dummy_dfa = DummyDFA::new();
        let dfa = dummy_dfa.as_dfa();
        let matcher = DFAEarliestPatternMatcher::new(&dfa);
        // With no feed calls, there is no active configuration.
        assert_eq!(matcher.earliest_starting_position(), None);
        assert_eq!(matcher.current_matching(), None);
    }

    /// Tests that the matcher correctly identifies a single pattern match.
    #[test]
    fn test_single_match() {
        let dummy_dfa = DummyDFA::new();
        let dfa = dummy_dfa.as_dfa();
        let mut matcher = DFAEarliestPatternMatcher::new(&dfa);

        // Feed 'a' then 'b', which should match the pattern "ab".
        matcher.feed(&'a');
        // After feeding 'a', no final state has been reached.
        // The initial state is activated at input index 0.
        assert_eq!(matcher.current_matching(), None);
        assert_eq!(matcher.earliest_starting_position(), Some(0));
        assert_eq!(matcher.len, 1);

        matcher.feed(&'b');
        // After feeding 'b', the DFA reaches the final state Q2 via Q1.
        // The starting index of the match remains as 0.
        assert_eq!(matcher.current_matching(), Some(0));
        assert_eq!(matcher.earliest_starting_position(), Some(0));
        assert_eq!(matcher.len, 2);
    }

    /// Tests that the matcher correctly identifies the earliest occurrence in overlapping matches.
    #[test]
    fn test_overlapping_matches() {
        let dummy_dfa = DummyDFA::new();
        let dfa = dummy_dfa.as_dfa();
        let mut matcher = DFAEarliestPatternMatcher::new(&dfa);

        // Input: "abab"
        let inputs = vec!['a', 'b', 'a', 'b'];
        for ch in inputs {
            matcher.feed(&ch);
        }
        // After processing "abab":
        // - The earliest starting index among active states is 0.
        // - A final state is reached with a starting index of 2 (from the second occurrence of "ab").
        assert_eq!(matcher.current_matching(), Some(2));
        assert_eq!(matcher.earliest_starting_position(), Some(0));
        assert_eq!(matcher.len, 4);
    }

    /// A more complex DFA that can match both "ab" and "abb".
    #[derive(Debug)]
    struct MultiPatternDFA {
        pub states: HashSet<State>,
        pub initial: State,
        pub finals: HashSet<State>,
        pub transitions: HashMap<(State, char), State>,
    }

    impl MultiPatternDFA {
        /// Creates a new DFA configured to recognize both "ab" and "abb".
        ///
        /// # Returns
        ///
        /// A new instance of `MultiPatternDFA`.
        fn new() -> Self {
            // Define states: Q0 (initial), Q1 (after 'a'), Q2 (final, after 'ab'),
            // Q3 (final, after 'abb'), Qd (dead state)
            let states = vec![State::Q0, State::Q1, State::Q2, State::Q3, State::Qd]
                .into_iter()
                .collect();
            let initial = State::Q0;
            let finals = vec![State::Q2, State::Q3].into_iter().collect();
            let mut transitions = HashMap::new();

            // Transitions for state Q0 (initial)
            transitions.insert((State::Q0, 'a'), State::Q1);
            transitions.insert((State::Q0, 'b'), State::Qd);

            // Transitions for state Q1 (after 'a')
            transitions.insert((State::Q1, 'a'), State::Qd);
            transitions.insert((State::Q1, 'b'), State::Q2);

            // Transitions for state Q2 (final, after 'ab')
            transitions.insert((State::Q2, 'a'), State::Q1); // Can start a new pattern
            transitions.insert((State::Q2, 'b'), State::Q3); // Can extend to "abb"

            // Transitions for state Q3 (final, after 'abb')
            transitions.insert((State::Q3, 'a'), State::Q1); // Can start a new pattern
            transitions.insert((State::Q3, 'b'), State::Qd);

            // Transitions for the dead state Qd
            transitions.insert((State::Qd, 'a'), State::Qd);
            transitions.insert((State::Qd, 'b'), State::Qd);

            MultiPatternDFA {
                states,
                initial,
                finals,
                transitions,
            }
        }

        /// Converts the multi-pattern DFA into the `DFA` type expected by the matcher.
        fn as_dfa(&self) -> DFA<State, char> {
            DFA {
                states: self.states.clone(),
                alphabet: vec!['a', 'b'].into_iter().collect(),
                initial: self.initial.clone(),
                finals: self.finals.clone(),
                transitions: self.transitions.clone(),
            }
        }
    }

    /// Tests that the matcher correctly identifies the earliest match when multiple matches
    /// are possible at different positions.
    #[test]
    fn test_earliest_match_critical() {
        let multi_dfa = MultiPatternDFA::new();
        let dfa = multi_dfa.as_dfa();
        let mut matcher = DFAEarliestPatternMatcher::new(&dfa);

        // Input: "ababb"
        // This test verifies that the matcher correctly identifies the earliest match
        // when multiple patterns could match at different positions.
        // The DFA can match both "ab" and "abb", and we'll feed each character individually
        // to check the matcher's state after each step.

        // Feed 'a' (position 0)
        matcher.feed(&'a');
        assert_eq!(matcher.current_matching(), None);
        assert_eq!(matcher.earliest_starting_position(), Some(0));

        // Feed 'b' (position 1) - Should match "ab" starting at position 0
        matcher.feed(&'b');
        assert_eq!(matcher.current_matching(), Some(0));
        assert_eq!(matcher.earliest_starting_position(), Some(0));

        // Feed 'a' (position 2)
        matcher.feed(&'a');
        // After feeding 'a', we're in state Q1 with starting position 2
        // and no final states are active, so current_matching() returns None
        assert_eq!(matcher.current_matching(), None);
        assert_eq!(matcher.earliest_starting_position(), Some(0));

        // Feed 'b' (position 3)
        matcher.feed(&'b');
        // After feeding 'b', we reach state Q2 (final) with starting position 2
        // But the DFA also maintains state Q1 with starting position 0
        // So current_matching() returns the earliest starting position (0)
        assert_eq!(matcher.current_matching(), Some(0));
        assert_eq!(matcher.earliest_starting_position(), Some(0));

        // Feed 'b' (position 4)
        matcher.feed(&'b');
        // After feeding another 'b', we reach state Q3 (final) with starting position 2
        // The DFA still maintains the earliest starting position for each state
        // So current_matching() still returns the earliest starting position (0)
        assert_eq!(matcher.current_matching(), Some(0));
        assert_eq!(matcher.earliest_starting_position(), Some(0));
        assert_eq!(matcher.len, 5);
    }
}
