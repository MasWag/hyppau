use itertools::Itertools;

use crate::{
    automata::NFAH,
    automata_runner::{NFAHRunner, ReadableView},
    hyper_pattern_matching::PatternMatchingAutomataRunner,
    result_notifier::{MatchingInterval, ResultNotifier},
    single_hyper_pattern_matching::SingleHyperPatternMatching,
};

pub struct OnlineSingleHyperPatternMatching<'a, Notifier: ResultNotifier> {
    automata_runner: PatternMatchingAutomataRunner<'a>,
    notifier: Notifier,
    input_streams: Vec<ReadableView<String>>,
    ids: Vec<usize>,
}

impl<Notifier: ResultNotifier> OnlineSingleHyperPatternMatching<'_, Notifier> {
    fn get_read_size(&self, variable: usize) -> usize {
        self.input_streams[variable].start
    }

    fn build_initial_positions(&self, inserted_var: usize) -> Vec<Vec<usize>> {
        let variable_size = self.dimensions();
        let mut all_dims = Vec::with_capacity(variable_size);

        // Calculate the total product size to estimate capacity
        let mut product_size = 1;

        for variable in 0..variable_size {
            if variable == inserted_var {
                all_dims.push(vec![self.get_read_size(variable) - 1]);
            } else {
                let dim_size = self.get_read_size(variable);
                product_size *= dim_size;
                let mut dim_values = Vec::with_capacity(dim_size);
                for i in 0..dim_size {
                    dim_values.push(i);
                }
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
                self.input_streams[variable].advance_readable(1);
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
                    self.automata_runner
                        .insert_from_initial_states(new_view, self.ids.clone());
                }
            }
        }
    }
}

impl<'a, Notifier: ResultNotifier> SingleHyperPatternMatching<'a, Notifier>
    for OnlineSingleHyperPatternMatching<'a, Notifier>
{
    fn new(
        automaton: &'a NFAH<'a>,
        notifier: Notifier,
        input_streams: Vec<ReadableView<String>>,
        ids: Vec<usize>,
    ) -> Self {
        let automata_runner = PatternMatchingAutomataRunner::new(automaton);

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
    }

    fn get_input_stream(&self, variable: usize) -> &ReadableView<String> {
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
    fn test_online_single_hyper_pattern_matching() {
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
        let mut matcher = OnlineSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            input_streams,
            ids.clone(),
        );

        let mut result_sink = result_buffer.make_sink();

        // Feed input incrementally
        sequences[0].append("a".to_string());
        matcher.consume_input();
        sequences[0].close();
        matcher.consume_input();
        sequences[1].append("b".to_string());
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
    fn test_single_hyper_pattern_matching() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

        let s1 = automaton.add_state(true, false);
        let s12 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s13 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_nfah_transition(s1, "a".to_string(), 0, s12);
        automaton.add_nfah_transition(s12, "b".to_string(), 1, s2);
        automaton.add_nfah_transition(s1, "a".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "b".to_string(), 1, s1);
        automaton.add_nfah_transition(s1, "c".to_string(), 0, s13);
        automaton.add_nfah_transition(s13, "d".to_string(), 1, s3);
        automaton.remove_unreachable_transitions();

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].append("c".to_string());
        sequences[1].append("d".to_string());
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].close();
        sequences[1].close();
        let input_streams = sequences.iter().map(|s| s.readable_view()).collect();
        let ids = vec![0, 1];

        let result_buffer = SharedBuffer::new();

        let mut matcher = OnlineSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            input_streams,
            ids.clone(),
        );

        let mut result_sink = result_buffer.make_sink();
        matcher.consume_input();

        // Test the results
        let mut expected_intervals = vec![
            vec![1, 1, 1, 1],
            vec![1, 1, 0, 1],
            vec![0, 1, 0, 1],
            vec![0, 1, 1, 1],
        ];
        // Since the order of the online pattern matching is nondeterministic, we sort the results
        expected_intervals.sort();

        let mut results = Vec::with_capacity(expected_intervals.len());

        while let Some(result) = result_sink.pop() {
            results.push(result);
        }
        results.sort();

        assert_eq!(results.len(), expected_intervals.len());

        verify_intervals(&results, &expected_intervals);
    }
}
