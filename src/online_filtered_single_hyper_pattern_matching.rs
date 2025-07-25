use itertools::Itertools;
use log::trace;

use crate::{
    automata::NFAH,
    automata_runner::ReadableView,
    filtered_pattern_matching_automata_runner::FilteredPatternMatchingAutomataRunner,
    filtered_single_hyper_pattern_matching::FilteredSingleHyperPatternMatching,
    result_notifier::{MatchingInterval, ResultNotifier},
};

pub struct OnlineFilteredSingleHyperPatternMatching<'a, Notifier: ResultNotifier> {
    automata_runner: FilteredPatternMatchingAutomataRunner<'a>,
    notifier: Notifier,
    input_streams: Vec<ReadableView<Option<String>>>,
    ids: Vec<usize>,
}

impl<Notifier: ResultNotifier> OnlineFilteredSingleHyperPatternMatching<'_, Notifier> {
    /// Computes the positions that are not skipped by the filter
    fn compute_retained_positions(&self, variable: usize) -> Vec<usize> {
        let max_value = self.input_streams[variable].start;
        self.input_streams[variable]
            .full_slice()
            .iter()
            .enumerate()
            .filter(|(i, value)| value.is_some() && *i < max_value)
            .map(|(i, _)| i)
            .collect()
    }

    fn get_read_size(&self, variable: usize) -> usize {
        self.input_streams[variable].start
    }

    fn build_initial_positions(&self, inserted_var: usize) -> Vec<Vec<usize>> {
        // If the initial position of the inserted variable is skipped, return an empty vector
        if self.input_streams[inserted_var].readable_slice()[0].is_none() {
            return Vec::new();
        }
        let variable_size = self.dimensions();
        let mut all_dims = Vec::with_capacity(variable_size);

        // Calculate the total product size to estimate capacity
        let mut product_size = 1;

        for variable in 0..variable_size {
            if variable == inserted_var {
                all_dims.push(vec![self.get_read_size(variable)]);
            } else {
                let dim_size = self.get_read_size(variable);
                product_size *= dim_size;
                let dim_values = self.compute_retained_positions(variable);
                all_dims.push(dim_values);
            }
        }

        // Preallocate the result vector with the calculated capacity
        let mut result = Vec::with_capacity(product_size);

        // Process the cartesian product with less allocations
        all_dims
            .iter()
            .multi_cartesian_product()
            .for_each(|combo_of_refs| {
                // Preallocate each inner vector with exact size
                let mut inner = Vec::with_capacity(variable_size);
                for &val in combo_of_refs {
                    inner.push(val);
                }
                result.push(inner);
            });

        result
    }

    fn insert_initial_positions(&mut self) {
        let variable_size = self.dimensions();
        for variable in 0..variable_size {
            while !self.input_streams[variable].is_empty() {
                // Get initial positions with optimized memory usage
                let initial_positions = self.build_initial_positions(variable);
                // Process each initial position
                for initial_position in initial_positions {
                    // Create views for each dimension with preallocated capacity
                    let mut new_view = Vec::with_capacity(variable_size);

                    for j in 0..variable_size {
                        new_view.push(self.input_streams[j].clone());
                        new_view[j].start = initial_position[j];
                    }

                    // Insert new configurations
                    self.automata_runner.insert_from_initial_states(new_view);
                }
                self.input_streams[variable].advance_readable(1);
            }
        }
    }
}

impl<'a, Notifier: ResultNotifier> FilteredSingleHyperPatternMatching<'a, Notifier>
    for OnlineFilteredSingleHyperPatternMatching<'a, Notifier>
{
    fn new(
        automaton: &'a NFAH<'a>,
        notifier: Notifier,
        input_streams: Vec<ReadableView<Option<String>>>,
        ids: Vec<usize>,
    ) -> Self {
        let automata_runner = FilteredPatternMatchingAutomataRunner::new(automaton, ids.clone());

        Self {
            automata_runner,
            notifier,
            input_streams,
            ids,
        }
    }

    fn dimensions(&self) -> usize {
        self.ids.len()
    }

    fn get_id(&self, variable: usize) -> Option<usize> {
        self.ids.get(variable).copied()
    }

    fn consume_input(&mut self) {
        trace!("Enter OnlineFilteredSingleHyperPatternMatching::consume_input");
        // Insert the initial positions as much as possible
        self.insert_initial_positions();
        // Thus, all input streams should be empty
        assert!(self.input_streams.iter().all(|stream| stream.is_empty()));

        // Process configurations
        self.automata_runner.consume();

        // Get and process final configurations
        let final_configurations = self.automata_runner.get_final_configurations();

        // Process each final configuration
        for c in &final_configurations {
            // Build result intervals
            let dims = self.ids.len(); // Use the length of ids directly to avoid ambiguity
            let mut result = Vec::with_capacity(dims);

            for i in 0..dims {
                let begin = c.matching_begin[i];
                let end = c.input_sequence[i].start - 1;
                result.push(MatchingInterval::new(begin, end));
            }

            // Notify with the result
            self.notifier.notify(&result, &c.ids);
        }

        // Remove configurations that are not in a waiting state
        self.automata_runner.remove_non_waiting_configurations();
        // Remove masked configurations (specific to filtered version)
        self.automata_runner.remove_masked_configurations();

        trace!("Exit OnlineFilteredSingleHyperPatternMatching::consume_input");
    }

    fn get_input_stream(&self, variable: usize) -> &ReadableView<Option<String>> {
        &self.input_streams[variable]
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::automata_runner::AppendOnlySequence;
    use crate::result_notifier::SharedBufferResultNotifier;
    use crate::shared_buffer::SharedBuffer;
    use crate::tests::utils::verify_intervals;
    use typed_arena::Arena;

    #[test]
    fn test_online_filtered_single_hyper_pattern_matching() {
        // Create a simple automaton that matches "a" on track 0 followed by "b" on track 1
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

        // Create states
        let s0 = automaton.add_state(true, false); // Initial state
        let s1 = automaton.add_state(false, false); // After reading "a"
        let s2 = automaton.add_state(false, true); // Final state after reading "b"

        // Add transitions
        automaton.add_nfah_transition(s0, "a".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "b".to_string(), 1, s2);

        // Create sequences and matcher
        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        let input_streams = sequences.iter().map(|s| s.readable_view()).collect();
        let ids = vec![0, 1];

        let result_buffer = SharedBuffer::new();
        let mut matcher = OnlineFilteredSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            input_streams,
            ids.clone(),
        );

        let mut result_sink = result_buffer.make_sink();

        // Feed input incrementally
        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[0].close();
        matcher.consume_input();
        sequences[1].append(Some("b".to_string()));
        matcher.consume_input();
        sequences[1].close();
        matcher.consume_input();

        // Check for the pattern match
        while let Some(match_result) = result_sink.pop() {
            assert_eq!(ids, match_result.ids);
            assert_eq!(
                vec![MatchingInterval::new(0, 0), MatchingInterval::new(0, 0)],
                match_result.intervals
            );
        }

        // No more results should be available
        assert!(result_sink.pop().is_none());
    }

    #[test]
    fn test_online_filtered_single_hyper_pattern_matching_with_filtered_elements() {
        // Create a simple automaton that matches "a" on track 0 followed by "b" on track 1
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

        // Create states
        let s0 = automaton.add_state(true, false); // Initial state
        let s1 = automaton.add_state(false, false); // After reading "a"
        let s2 = automaton.add_state(false, true); // Final state after reading "b"

        // Add transitions
        automaton.add_nfah_transition(s0, "a".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "b".to_string(), 1, s2);

        // Create sequences and matcher
        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        let input_streams = sequences.iter().map(|s| s.readable_view()).collect();
        let ids = vec![0, 1];

        let result_buffer = SharedBuffer::new();
        let mut matcher = OnlineFilteredSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            input_streams,
            ids.clone(),
        );

        let mut result_sink = result_buffer.make_sink();

        // Feed input incrementally with some filtered elements
        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[0].append(None); // Filtered element
        matcher.consume_input();
        sequences[0].close();
        matcher.consume_input();
        sequences[1].append(None); // Filtered element
        matcher.consume_input();
        sequences[1].append(Some("b".to_string()));
        matcher.consume_input();
        sequences[1].close();
        matcher.consume_input();

        // Check for the pattern match
        while let Some(match_result) = result_sink.pop() {
            assert_eq!(ids, match_result.ids);
            assert_eq!(
                vec![MatchingInterval::new(0, 0), MatchingInterval::new(1, 1)],
                match_result.intervals
            );
        }

        // No more results should be available
        assert!(result_sink.pop().is_none());
    }

    #[test]
    fn test_small_double() {
        // Create the automaton in small.json
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

        // Create states based on small.json
        let s0 = automaton.add_state(true, false); // id: 0, is_initial: true, is_final: false
        let s1 = automaton.add_state(false, false); // id: 1, is_initial: false, is_final: false
        let s2 = automaton.add_state(false, false); // id: 2, is_initial: false, is_final: false
        let s3 = automaton.add_state(false, false); // id: 3, is_initial: false, is_final: false
        let s4 = automaton.add_state(false, true); // id: 4, is_initial: false, is_final: true

        // Add transitions based on small.json
        automaton.add_nfah_transition(s0, "a".to_string(), 0, s1); // from: 0, to: 1, label: ["a", 0]
        automaton.add_nfah_transition(s1, "b".to_string(), 1, s2); // from: 1, to: 2, label: ["b", 1]
        automaton.add_nfah_transition(s0, "a".to_string(), 0, s0); // from: 0, to: 0, label: ["a", 0]
        automaton.add_nfah_transition(s0, "b".to_string(), 1, s0); // from: 0, to: 0, label: ["b", 1]
        automaton.add_nfah_transition(s0, "c".to_string(), 0, s3); // from: 0, to: 3, label: ["c", 0]
        automaton.add_nfah_transition(s3, "d".to_string(), 1, s4); // from: 3, to: 4, label: ["d", 1]

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        let input_streams = sequences.iter().map(|s| s.readable_view()).collect();
        let ids = vec![0, 1];

        let result_buffer = SharedBuffer::new();

        let mut matcher = OnlineFilteredSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            input_streams,
            ids.clone(),
        );

        let mut result_sink = result_buffer.make_sink();

        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[1].append(None);
        matcher.consume_input();
        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[1].append(Some("d".to_string()));
        matcher.consume_input();
        sequences[0].append(Some("c".to_string()));
        matcher.consume_input();
        sequences[1].append(Some("d".to_string()));
        matcher.consume_input();
        sequences[1].close();
        matcher.consume_input();
        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[0].append(Some("c".to_string()));
        matcher.consume_input();
        sequences[0].close();
        matcher.consume_input();

        // The expected results
        let expected_intervals = vec![
            vec![0, 2, 1, 1],
            vec![0, 2, 2, 2],
            vec![1, 2, 1, 1],
            vec![1, 2, 2, 2],
            vec![2, 2, 1, 1],
            vec![2, 2, 2, 2],
            vec![3, 5, 1, 1],
            vec![3, 5, 2, 2],
            vec![4, 5, 1, 1],
            vec![4, 5, 2, 2],
            vec![5, 5, 1, 1],
            vec![5, 5, 2, 2],
        ];

        // Collect all results
        let mut results = Vec::new();
        while let Some(result) = result_sink.pop() {
            results.push(result);
        }
        results.sort();
        results.dedup();

        assert_eq!(results.len(), expected_intervals.len());

        verify_intervals(&results, &expected_intervals);
    }
}
