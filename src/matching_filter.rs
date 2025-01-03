use log::debug;

use crate::{
    automata_runner::{AppendOnlySequence, ReadableView},
    dfa_earliest_pattern_matcher::DFAEarliestPatternMatcher,
};
use std::{collections::VecDeque, hash::Hash};

/// Given a stream, it masks the elements not appearing in any matching
/// pattern by replacing them with None. Elements that are part of a match
/// are preserved as Some(element).
pub struct MatchingFilter<S, L> {
    /// The pattern matcher used to identify matches in the input stream
    matcher: DFAEarliestPatternMatcher<S, L>,
    /// The input stream to filter
    input_stream: ReadableView<L>,
    /// Temporally queue
    temporally_queue: VecDeque<(L, bool)>,
    /// The output stream containing filtered elements (Some(L) for matched elements, None for unmatched)
    output_stream: AppendOnlySequence<Option<L>>,
}

impl<S, L> MatchingFilter<S, L>
where
    S: Eq + Hash + Clone,
    L: Eq + Hash + Clone,
{
    /// Creates a new MatchingFilter from a DFA and an input stream
    ///
    /// # Arguments
    ///
    /// * `matcher` - A DFAEarliestPatternMatcher that will be used to identify patterns
    /// * `input_stream` - The input stream to filter
    ///
    /// # Returns
    ///
    /// A new MatchingFilter instance
    pub fn new(matcher: DFAEarliestPatternMatcher<S, L>, input_stream: ReadableView<L>) -> Self {
        // Estimate initial capacity for the queue based on input stream size
        let estimated_capacity = input_stream.len().max(16);

        Self {
            matcher,
            input_stream,
            temporally_queue: VecDeque::with_capacity(estimated_capacity),
            output_stream: AppendOnlySequence::new(),
        }
    }

    /// Returns a ReadableView of the output stream
    ///
    /// # Returns
    ///
    /// A ReadableView of the filtered output stream
    pub fn readable_view(&self) -> ReadableView<Option<L>> {
        let readable_view = self.output_stream.readable_view();
        // Since we do not move the start position of the output_stream, the start position of the readable_view should be 0
        assert!(readable_view.start == 0);
        readable_view
    }

    /// Consumes elements from the input stream, processes them through the matcher,
    /// and updates the output stream accordingly.
    ///
    /// Elements that are part of a match are preserved as Some(element),
    /// while elements not part of any match are represented as None.
    pub fn consume_input(&mut self) {
        // The following is the invariant of the MatchingFilter:
        assert!(self.input_stream.start == self.temporally_queue.len() + self.output_stream.len());
        // Get the input length and clone elements to process
        let input_len;
        let elements_to_process: Vec<L>;
        {
            let input_slice = self.input_stream.readable_slice();
            input_len = input_slice.len();

            // Clone the elements we need to process
            elements_to_process = input_slice.iter().cloned().collect();
        }

        // Pre-allocate capacity for new elements in the queue
        if self.temporally_queue.capacity() < self.temporally_queue.len() + input_len {
            self.temporally_queue.reserve(input_len);
        }

        // Process each element
        for element in elements_to_process {
            // Feed the element to the matcher
            self.matcher.feed(&element);

            // Add to the temporary queue
            self.temporally_queue.push_back((element, false));

            // Process based on matcher state
            if let Some(matching_bound) = self.matcher.earliest_starting_position() {
                // Calculate how many elements to move
                let elements_to_move = matching_bound - self.output_stream.len();

                if elements_to_move > 0 {
                    // Prepare a batch of elements to append
                    let mut batch = Vec::with_capacity(elements_to_move);

                    // Move elements from the temporally_queue to the batch
                    for _ in 0..elements_to_move {
                        match self.temporally_queue.pop_front() {
                            Some((label, true)) => batch.push(Some(label)),
                            Some((_, false)) => batch.push(None),
                            None => panic!("Something is wrong with the temporally_queue"),
                        }
                    }

                    // Append the batch to the output stream
                    for item in batch {
                        self.output_stream.append(item);
                    }
                }

                // Mark the elements in a match as matched
                if let Some(i) = self.matcher.current_matching() {
                    let pos_in_queue = i - matching_bound;
                    for j in pos_in_queue..self.temporally_queue.len() {
                        self.temporally_queue[j].1 = true;
                    }
                }
            } else {
                // Move all elements from the queue to the output stream
                let queue_len = self.temporally_queue.len();

                if queue_len > 0 {
                    // Prepare a batch of elements to append
                    let mut batch = Vec::with_capacity(queue_len);

                    // Move elements from the temporally_queue to the batch
                    for _ in 0..queue_len {
                        match self.temporally_queue.pop_front() {
                            Some((label, true)) => batch.push(Some(label)),
                            Some((_, false)) => batch.push(None),
                            None => panic!("Something is wrong with the temporally_queue"),
                        }
                    }

                    // Append the batch to the output stream
                    for item in batch {
                        self.output_stream.append(item);
                    }
                }
            }
        }

        // Advance the input stream
        self.input_stream.advance_readable(input_len);

        // All the elements in the input stream should be consumed
        assert_eq!(0, self.input_stream.len());

        // Check if the stream is closed
        self.check_closed();
    }

    /// Check if the input_stream is already closed. If it is closed, move the remaining elements from the temporally_queue to the output_stream
    pub fn check_closed(&mut self) {
        if self.input_stream.is_closed() && !self.output_stream.is_closed() {
            debug!("Close the filter");

            // Process all remaining elements in the queue in a batch
            let queue_len = self.temporally_queue.len();

            if queue_len > 0 {
                // Prepare a batch of elements to append
                let mut batch = Vec::with_capacity(queue_len);

                // Move elements from the temporally_queue to the batch
                while !self.temporally_queue.is_empty() {
                    match self.temporally_queue.pop_front() {
                        Some((label, true)) => batch.push(Some(label)),
                        Some((_, false)) => batch.push(None),
                        None => panic!("Something is wrong with the temporally_queue"),
                    }
                }

                // Append the batch to the output stream
                for item in batch {
                    self.output_stream.append(item);
                }
            }

            // Close the output stream
            self.output_stream.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use typed_arena::Arena;

    use super::*;
    use crate::{
        automata::{NFAHState, NFAHTransition, NFAH},
        dfa::DFA,
    };
    use std::collections::{HashMap, HashSet};

    // Define a simple enum for states in our test DFA
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum State {
        Initial,
        Middle,
        Final,
    }

    // Helper function to create a test DFA that matches the pattern "ab"
    fn create_test_dfa() -> DFA<State, char> {
        let states = vec![State::Initial, State::Middle, State::Final]
            .into_iter()
            .collect::<HashSet<_>>();

        let initial = State::Initial;
        let finals = vec![State::Final].into_iter().collect();

        let mut transitions = HashMap::new();
        transitions.insert((State::Initial, 'a'), State::Middle);
        transitions.insert((State::Middle, 'b'), State::Final);

        DFA {
            states,
            alphabet: vec!['a', 'b', 'c'].into_iter().collect(),
            initial,
            finals,
            transitions,
        }
    }

    // Define states for the small.json automaton projection (variable 0)
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum SmallState {
        S0, // Initial state
        S1, // After 'a'
        S3, // After 'c'
        S4, // Final state (not directly reachable in variable 0 projection)
    }

    // Helper function to create a DFA based on the variable 0 projection from test_small_double
    fn create_small_dfa() -> DFA<SmallState, String> {
        let states = vec![
            SmallState::S0,
            SmallState::S1,
            SmallState::S3,
            SmallState::S4,
        ]
        .into_iter()
        .collect::<HashSet<_>>();

        let initial = SmallState::S0;
        let finals = vec![SmallState::S3].into_iter().collect();

        let mut transitions = HashMap::new();
        // In the original NFAH, there are transitions from S0 to S1 and S0 to S0 with label "a",
        // but in a DFA we can only have one transition per state-label pair.
        // Since we're testing the filter's behavior with the "c" pattern, we'll prioritize that.
        transitions.insert((SmallState::S0, "a".to_string()), SmallState::S1);
        transitions.insert((SmallState::S0, "c".to_string()), SmallState::S3);

        DFA {
            states,
            alphabet: vec!["a".to_string(), "c".to_string()].into_iter().collect(),
            initial,
            finals,
            transitions,
        }
    }

    #[test]
    fn test_matching_filter_basic() {
        // Create a DFA that matches "ab"
        let dfa = create_test_dfa();

        // Create a matcher from the DFA
        let matcher = DFAEarliestPatternMatcher::new(dfa);

        // Create an input stream with "abc"
        let mut input_seq = AppendOnlySequence::new();
        input_seq.append('a');
        input_seq.append('b');
        input_seq.append('c');

        // Create a matching filter
        let mut filter = MatchingFilter::new(matcher, input_seq.readable_view());

        // Process the input
        filter.consume_input();

        // Check the output
        let output = filter.readable_view();
        let output_slice = output.readable_slice();

        // "a" and "b" should be preserved (Some), "c" should be filtered (None)
        assert_eq!(output_slice.len(), 3);
        assert_eq!(output_slice[0], Some('a'));
        assert_eq!(output_slice[1], Some('b'));
        assert_eq!(output_slice[2], None);
    }

    #[test]
    fn test_matching_filter_multiple_matches() {
        // Create a DFA that matches "ab"
        let dfa = create_test_dfa();

        // Create a matcher from the DFA
        let matcher = DFAEarliestPatternMatcher::new(dfa);

        // Create an input stream with "abcabc"
        let mut input_seq = AppendOnlySequence::new();
        input_seq.append('a');
        input_seq.append('b');
        input_seq.append('c');
        input_seq.append('a');
        input_seq.append('b');
        input_seq.append('c');

        // Create a matching filter
        let mut filter = MatchingFilter::new(matcher, input_seq.readable_view());

        // Process the input
        filter.consume_input();

        // Check the output
        let output = filter.readable_view();
        let output_slice = output.readable_slice();

        // "a" and "b" should be preserved (Some), "c" should be filtered (None)
        assert_eq!(output_slice.len(), 6);
        assert_eq!(output_slice[0], Some('a'));
        assert_eq!(output_slice[1], Some('b'));
        assert_eq!(output_slice[2], None);
        assert_eq!(output_slice[3], Some('a'));
        assert_eq!(output_slice[4], Some('b'));
        assert_eq!(output_slice[5], None);
    }

    #[test]
    fn test_matching_filter_no_matches() {
        // Create a DFA that matches "ab"
        let dfa = create_test_dfa();

        // Create a matcher from the DFA
        let matcher = DFAEarliestPatternMatcher::new(dfa);

        // Create an input stream with "ccc"
        let mut input_seq = AppendOnlySequence::new();
        input_seq.append('c');
        input_seq.append('c');
        input_seq.append('c');

        // Create a matching filter
        let mut filter = MatchingFilter::new(matcher, input_seq.readable_view());

        // Process the input
        filter.consume_input();

        // Check the output
        let output = filter.readable_view();
        let output_slice = output.readable_slice();

        // All elements should be filtered (None)
        assert_eq!(output_slice.len(), 3);
        assert_eq!(output_slice[0], None);
        assert_eq!(output_slice[1], None);
        assert_eq!(output_slice[2], None);
    }

    #[test]
    fn test_matching_filter_empty_input() {
        // Create a DFA that matches "ab"
        let dfa = create_test_dfa();

        // Create a matcher from the DFA
        let matcher = DFAEarliestPatternMatcher::new(dfa);

        // Create an empty input stream
        let input_seq = AppendOnlySequence::<char>::new();

        // Create a matching filter
        let mut filter = MatchingFilter::new(matcher, input_seq.readable_view());

        // Process the input
        filter.consume_input();

        // Check the output
        let output = filter.readable_view();
        let output_slice = output.readable_slice();

        // Output should be empty
        assert_eq!(output_slice.len(), 0);
    }

    /// Helper function to create a standard test automaton with 2 dimensions
    fn create_small_automaton<'a>(
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

    #[test]
    fn test_small_double() {
        // Create the automaton directly instead of loading from small.json
        let nfah_state_arena = Arena::new();
        let nfah_transition_arena = Arena::new();
        let enfa_state_arena = Arena::new();
        let enfa_transition_arena = Arena::new();
        let nfa_state_arena = Arena::new();
        let nfa_transition_arena = Arena::new();
        let automaton = create_small_automaton(&nfah_state_arena, &nfah_transition_arena);

        let mut dfas = Vec::with_capacity(2);
        dfas.push(
            automaton
                .project(&enfa_state_arena, &enfa_transition_arena, 0)
                .to_nfa_powerset(&nfa_state_arena, &nfa_transition_arena)
                .determinize(),
        );
        dfas.push(
            automaton
                .project(&enfa_state_arena, &enfa_transition_arena, 1)
                .to_nfa_powerset(&nfa_state_arena, &nfa_transition_arena)
                .determinize(),
        );

        // Create a matcher from the DFA
        let matchers = dfas
            .into_iter()
            .map(DFAEarliestPatternMatcher::new)
            .collect_vec();

        let mut input_seqs = [AppendOnlySequence::new(), AppendOnlySequence::new()];

        // Create a matching filter
        let mut filters = matchers
            .into_iter()
            .enumerate()
            .map(|(i, matcher)| MatchingFilter::new(matcher, input_seqs[i].readable_view()))
            .collect_vec();

        // Process the input
        input_seqs[0].append("a".to_string());
        filters[0].consume_input();
        input_seqs[0].append("a".to_string());
        filters[0].consume_input();
        input_seqs[0].append("c".to_string());
        filters[0].consume_input();
        input_seqs[0].append("a".to_string());
        filters[0].consume_input();
        input_seqs[0].append("a".to_string());
        filters[0].consume_input();
        input_seqs[0].append("c".to_string());
        filters[0].consume_input();
        input_seqs[0].close();
        filters[0].consume_input();

        // Process the input
        input_seqs[1].append("a".to_string());
        filters[1].consume_input();
        input_seqs[1].append("d".to_string());
        filters[1].consume_input();
        input_seqs[1].append("d".to_string());
        filters[1].consume_input();
        input_seqs[1].close();
        filters[1].consume_input();

        // Check the output
        let outputs = filters
            .iter()
            .map(|filter| filter.readable_view())
            .collect_vec();
        let output_slices = outputs
            .iter()
            .map(|output| output.readable_slice())
            .collect_vec();

        assert_eq!(output_slices[0].len(), 6);
        assert_eq!(output_slices[0][0], Some("a".to_string()));
        assert_eq!(output_slices[0][1], Some("a".to_string()));
        assert_eq!(output_slices[0][2], Some("c".to_string()));
        assert_eq!(output_slices[0][3], Some("a".to_string()));
        assert_eq!(output_slices[0][4], Some("a".to_string()));
        assert_eq!(output_slices[0][5], Some("c".to_string()));

        assert_eq!(output_slices[1].len(), 3);
        assert_eq!(output_slices[1][0], None);
        assert_eq!(output_slices[1][1], Some("d".to_string()));
        assert_eq!(output_slices[1][2], Some("d".to_string()));
    }

    #[test]
    fn test_small_with_abcd_10() {
        // Create the automaton directly instead of loading from small.json
        let nfah_state_arena = Arena::new();
        let nfah_transition_arena = Arena::new();
        let enfa_state_arena = Arena::new();
        let enfa_transition_arena = Arena::new();
        let nfa_state_arena = Arena::new();
        let nfa_transition_arena = Arena::new();
        let automaton = create_small_automaton(&nfah_state_arena, &nfah_transition_arena);

        let mut dfas = Vec::with_capacity(2);
        dfas.push(
            automaton
                .project(&enfa_state_arena, &enfa_transition_arena, 0)
                .to_nfa_powerset(&nfa_state_arena, &nfa_transition_arena)
                .determinize(),
        );
        dfas.push(
            automaton
                .project(&enfa_state_arena, &enfa_transition_arena, 1)
                .to_nfa_powerset(&nfa_state_arena, &nfa_transition_arena)
                .determinize(),
        );

        // Create a matcher from the DFA
        let matchers = dfas
            .into_iter()
            .map(DFAEarliestPatternMatcher::new)
            .collect_vec();

        let mut input_seq = AppendOnlySequence::new();

        // Create a matching filter
        let mut filters = matchers
            .into_iter()
            .map(|matcher| MatchingFilter::new(matcher, input_seq.readable_view()))
            .collect_vec();

        // Inputs generated by `seq 10 | gen_abcd.awk`
        let inputs = ["d", "b", "d", "d", "d", "a", "b", "d", "b", "c"];

        // Feed all the inputs to the input stream
        for input in inputs.iter() {
            input_seq.append(input.to_string());
            filters[0].consume_input();
            filters[1].consume_input();
        }

        // Set EOF for the input stream
        input_seq.close();
        filters[0].consume_input();
        filters[1].consume_input();

        // Check the output
        let outputs = filters
            .iter()
            .map(|filter| filter.readable_view())
            .collect_vec();
        let output_slices = outputs
            .iter()
            .map(|output| output.readable_slice())
            .collect_vec();

        assert_eq!(output_slices[0].len(), 10);
        for i in 0..9 {
            assert_eq!(output_slices[0][i], None);
        }
        assert_eq!(output_slices[0][9], Some("c".to_string()));

        assert_eq!(output_slices[1].len(), 10);
        let masked_output = [5, 8, 9];
        for i in 0..10 {
            if masked_output.contains(&i) {
                assert_eq!(output_slices[1][i], None);
            } else {
                assert_eq!(output_slices[1][i], Some(inputs[i].to_string()));
            }
        }
    }
}
