use std::{
    cmp::Reverse,
    collections::{BTreeSet, HashSet},
};

use itertools::Itertools;
use log::{debug, trace};

use crate::{
    automata::NFAH,
    automata_runner::ReadableView,
    filtered_pattern_matching_automata_runner::FilteredPatternMatchingAutomataRunner,
    filtered_single_hyper_pattern_matching::FilteredSingleHyperPatternMatching,
    kmp_skip_values::KMPSkipValues,
    naive_hyper_pattern_matching::StartPosition,
    quick_search_skip_values::QuickSearchSkipValues,
    result_notifier::{MatchingInterval, ResultNotifier},
};

pub struct FJSFilteredSingleHyperPatternMatching<'a, Notifier: ResultNotifier> {
    automata_runner: FilteredPatternMatchingAutomataRunner<'a>,
    notifier: Notifier,
    input_streams: Vec<ReadableView<Option<String>>>,
    ids: Vec<usize>,
    waiting_queue: BTreeSet<Reverse<StartPosition>>,
    /// The set of ignored starting positions by the skip values
    skipped_positions: Vec<HashSet<usize>>,
    quick_search_skip_value: QuickSearchSkipValues,
    kmp_skip_value: KMPSkipValues<'a>,
}

impl<'a, Notifier: ResultNotifier> FilteredSingleHyperPatternMatching<'a, Notifier>
    for FJSFilteredSingleHyperPatternMatching<'a, Notifier>
{
    fn new(
        automaton: &'a NFAH<'a>,
        notifier: Notifier,
        input_streams: Vec<ReadableView<Option<String>>>,
        ids: Vec<usize>,
    ) -> Self {
        let mut automata_runner =
            FilteredPatternMatchingAutomataRunner::new(automaton, ids.clone());
        let start_indices = vec![0; automaton.dimensions];
        let waiting_queue = StartPosition { start_indices }
            .immediate_successors()
            .into_iter()
            .map(Reverse)
            .collect();
        let skipped_positions = (0..automaton.dimensions)
            .map(|_| HashSet::new())
            .collect_vec();

        automata_runner.insert_from_initial_states(input_streams.clone());

        Self {
            automata_runner,
            notifier,
            input_streams,
            ids,
            waiting_queue,
            skipped_positions,
            quick_search_skip_value: QuickSearchSkipValues::new(automaton),
            kmp_skip_value: KMPSkipValues::new(automaton),
        }
    }

    fn dimensions(&self) -> usize {
        self.ids.len()
    }

    fn get_id(&self, variable: usize) -> Option<usize> {
        self.ids.get(variable).copied()
    }

    fn consume_input(&mut self) {
        trace!("Enter FJSFilteredSingleHyperPatternMatching::consume_input");
        self.automata_runner.consume();
        let final_configurations = self.automata_runner.get_final_configurations();
        let dimensions = self.dimensions();
        final_configurations.iter().cloned().for_each(|c| {
            let mut intervals = Vec::with_capacity(dimensions);
            for i in 0..dimensions {
                let begin = c.matching_begin[i];
                let end = c.input_sequence[i].start - 1;
                intervals.push(MatchingInterval::new(begin, end));
            }
            self.notifier.notify(&intervals, &c.ids);
        });

        // Apply KMP-style skip values
        for c in &self.automata_runner.current_configurations {
            for i in 0..c.ids.len() {
                if let Some(&skip_value) = self.kmp_skip_value.skip_values[i].get(c.current_state) {
                    for j in 1..skip_value {
                        if i < self.skipped_positions.len() {
                            self.skipped_positions[i].insert(c.matching_begin[i] + j);
                        }
                    }
                }
            }
        }

        self.automata_runner.remove_non_waiting_configurations();
        self.automata_runner.remove_masked_configurations();

        while self.automata_runner.current_configurations.is_empty() {
            // Find a new valid starting position
            if let Some(position) = self.waiting_queue.pop_last() {
                // We do not use if we are too early
                if self.in_range(&position.0)
                    && (0..self.dimensions())
                        .any(|v| position.0.start_indices[v] >= self.input_streams[v].len())
                {
                    debug!("The position {:?} is too early", position);
                    self.waiting_queue.insert(position);
                    // println!("{:?}", self.waiting_queue.last());
                    break;
                }
                // println!("new_position: {:?}", position);
                let valid_successors = self.compute_valid_successors(&position.0);

                // Put the successors to the waiting queue
                self.waiting_queue.extend(valid_successors);

                if self.is_valid_position(&position.0) && !self.is_skipped(&position.0) {
                    let mut input_streams = self.input_streams.clone();
                    for variable in 0..self.dimensions() {
                        input_streams[variable].start = position.0.start_indices[variable];
                    }

                    self.automata_runner
                        .insert_from_initial_states(input_streams);
                    self.automata_runner.consume();

                    let final_configurations = self.automata_runner.get_final_configurations();
                    final_configurations.iter().cloned().for_each(|c| {
                        let mut intervals = Vec::with_capacity(dimensions);
                        for i in 0..dimensions {
                            let begin = c.matching_begin[i];
                            let end = c.input_sequence[i].start - 1;
                            intervals.push(MatchingInterval::new(begin, end));
                        }
                        self.notifier.notify(&intervals, &c.ids);
                    });

                    // Apply KMP-style skip values
                    for c in &self.automata_runner.current_configurations {
                        for i in 0..c.ids.len() {
                            if let Some(&skip_value) =
                                self.kmp_skip_value.skip_values[i].get(c.current_state)
                            {
                                for j in 1..skip_value {
                                    if i < self.skipped_positions.len() {
                                        self.skipped_positions[i].insert(c.matching_begin[i] + j);
                                    }
                                }
                            }
                        }
                    }

                    self.automata_runner.remove_non_waiting_configurations();
                    self.automata_runner.remove_masked_configurations();
                }
            } else {
                // No more valid positions to try
                trace!("Exit FJSFilteredSingleHyperPatternMatching::consume_input");
                break;
            }
        }
        trace!("Exit FJSFilteredSingleHyperPatternMatching::consume_input");
    }

    fn get_input_stream(&self, variable: usize) -> &ReadableView<Option<String>> {
        if let Some(stream) = self.input_streams.get(variable) {
            stream
        } else {
            panic!("Variable {} is out of range", variable);
        }
    }
}

impl<Notifier: ResultNotifier> FJSFilteredSingleHyperPatternMatching<'_, Notifier> {
    /// Check if a position is valid (not skipped and within range)
    fn is_valid_position(&self, position: &StartPosition) -> bool {
        // Check if position is within range
        for i in 0..position.start_indices.len() {
            let stream = self.get_input_stream(i);
            if stream.is_closed() && position.start_indices[i] >= stream.len() {
                return false;
            }
        }

        // Check if position is skipped by KMP
        for i in 0..position.start_indices.len() {
            if i < self.skipped_positions.len()
                && self.skipped_positions[i].contains(&position.start_indices[i])
            {
                return false;
            }
        }

        true
    }

    /// Returns `true` if this position is not skippable with quick_search
    fn try_quick_search_skip(&mut self, position: &StartPosition) -> bool {
        // Check if we can apply Quick Search optimization
        let mut should_skip = false;
        let mut positions_to_skip = Vec::new();

        for var in 0..self.dimensions() {
            let start_index = position.start_indices[var];
            let shortest_matching_length = self
                .quick_search_skip_value
                .shortest_accepted_word_length_map[var];

            if shortest_matching_length > 0 {
                let stream = self.get_input_stream(var);
                let slice_shortest_end_idx =
                    start_index + shortest_matching_length - 1 - stream.start;
                let slice_next_idx = start_index + shortest_matching_length - stream.start;

                if slice_next_idx < stream.len() {
                    let readable_data = stream.readable_slice();

                    if slice_next_idx < readable_data.len() {
                        // Check if the data at the position is not None
                        if let Some(Some(data)) = readable_data.get(slice_shortest_end_idx) {
                            let last_accepted_words =
                                &self.quick_search_skip_value.last_accepted_word[var];

                            if !last_accepted_words.contains(data) {
                                if let Some(Some(next_char)) = readable_data.get(slice_next_idx) {
                                    let skipped_width =
                                        self.quick_search_skip_value.skip_value(next_char, var);

                                    for i in 0..skipped_width {
                                        positions_to_skip.push((var, start_index + i));
                                    }

                                    should_skip = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        if should_skip {
            // Apply the skips
            for (var, pos) in positions_to_skip {
                if var < self.skipped_positions.len() {
                    self.skipped_positions[var].insert(pos);
                }
            }
        }

        !should_skip
    }

    fn compute_skipped_indices(&self, start_position: &StartPosition) -> Vec<usize> {
        let mut result = Vec::with_capacity(self.dimensions());
        for i in 0..self.dimensions() {
            if self.skipped_positions[i].contains(&start_position.start_indices[i]) {
                result.push(i);
            }
        }

        return result;
    }

    fn compute_valid_successors(
        &mut self,
        start_position: &StartPosition,
    ) -> Vec<Reverse<StartPosition>> {
        let mut waiting_queue = Vec::new();
        waiting_queue.push(start_position.clone());
        let mut valid_successors = Vec::new();
        while let Some(examined_position) = waiting_queue.pop() {
            let skipped_indices = self.compute_skipped_indices(&examined_position);

            // The skipped variables skipped by the filter
            let skipped_streams = self.skipped_streams(&examined_position);

            let successor_candidates = examined_position
                .immediate_successors()
                .into_iter()
                .filter(|successor| self.in_range(successor))
                .collect_vec();

            for successor in successor_candidates.into_iter() {
                trace!("successor_candidate: {:?}", successor);
                if self.is_skipped(&successor) {
                    if skipped_streams.is_empty()
                        || skipped_streams.iter().any(|&i| {
                            successor.start_indices[i] != examined_position.start_indices[i]
                        })
                    {
                        // We examine only if `examined_position` is not skipped or one of the previously skipped positions was increased
                        waiting_queue.push(successor);
                    }
                } else if self.is_valid_position(&successor)
                    && self.try_quick_search_skip(&successor)
                {
                    // KMP/QS cannot skip `successor`
                    valid_successors.push(Reverse(successor));
                } else {
                    // Examine `successor` if `successor` is skipped by KMP/QS
                    // Check if one of the invalidated positions changed
                    if skipped_indices.is_empty()
                        || skipped_indices.iter().any(|&i| {
                            examined_position.start_indices[i] != successor.start_indices[i]
                        })
                    {
                        waiting_queue.push(successor);
                    }
                }
            }
        }

        valid_successors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        automata_runner::AppendOnlySequence,
        result_notifier::SharedBufferResultNotifier,
        shared_buffer::SharedBuffer,
        tests::utils::{create_small_automaton, verify_ids, verify_intervals},
    };
    use typed_arena::Arena;

    #[test]
    fn test_fjs_filtered_single_hyper_pattern_matching() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

        let s0 = automaton.add_state(true, false);
        let s1 = automaton.add_state(false, false);
        let s12 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s13 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_nfah_transition(s0, "a".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "a".to_string(), 0, s12);
        automaton.add_nfah_transition(s12, "b".to_string(), 1, s2);
        automaton.add_nfah_transition(s1, "a".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "b".to_string(), 1, s1);
        automaton.add_nfah_transition(s1, "c".to_string(), 0, s13);
        automaton.add_nfah_transition(s13, "d".to_string(), 1, s3);
        automaton.remove_unreachable_transitions();

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append(Some("e".to_string()));
        sequences[1].append(Some("e".to_string()));
        sequences[0].append(Some("e".to_string()));
        sequences[1].append(Some("e".to_string()));
        sequences[0].append(Some("a".to_string()));
        sequences[1].append(Some("b".to_string()));
        sequences[0].append(Some("c".to_string()));
        sequences[1].append(Some("d".to_string()));
        sequences[0].append(Some("a".to_string()));
        sequences[1].append(Some("b".to_string()));
        sequences[0].close();
        sequences[1].close();
        let input_streams = sequences.iter().map(|s| s.readable_view()).collect();
        let ids = vec![0, 1];

        let result_buffer = SharedBuffer::new();

        let mut matcher = FJSFilteredSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            input_streams,
            ids.clone(),
        );

        let mut result_sink = result_buffer.make_sink();
        matcher.consume_input();

        // Test the results - collect all results first
        let mut actual_results = Vec::new();
        while let Some(result) = result_sink.pop() {
            assert_eq!(ids.clone(), result.ids);
            actual_results.push(result.intervals);
        }

        // Check that we have the expected number of results
        assert_eq!(2, actual_results.len());

        // Check that all expected patterns are present
        let expected_patterns = vec![
            vec![MatchingInterval::new(2, 3), MatchingInterval::new(2, 3)],
            vec![MatchingInterval::new(2, 3), MatchingInterval::new(3, 3)],
        ];

        for expected in expected_patterns {
            assert!(
                actual_results.contains(&expected),
                "Expected result {:?} not found in actual results",
                expected
            );
        }
        assert!(result_sink.pop().is_none());
    }

    /// Test using the small automaton with inputs from small1.txt and small2.txt
    #[test]
    fn test_small() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let automaton = create_small_automaton(&state_arena, &transition_arena);
        automaton.remove_unreachable_transitions();

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        let views = sequences
            .iter()
            .map(|sequence| sequence.readable_view())
            .collect();

        let ids = vec![0, 1];

        let result_buffer = SharedBuffer::new();

        let mut matcher = FJSFilteredSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            views,
            ids.clone(),
        );

        let small1 = ["a", "a", "c"];
        let small2 = ["a", "d", "d"];

        // Feed all the inputs
        for i in 0..small1.len() {
            sequences[0].append(Some(small1[i].to_string()));
            matcher.consume_input();
            if i == 0 {
                sequences[1].append(None);
            } else {
                sequences[1].append(Some(small2[i].to_string()));
            }
            matcher.consume_input();
        }

        // Set EOF for the input streams
        sequences[0].close();
        matcher.consume_input();
        sequences[1].close();
        matcher.consume_input();

        let mut result_sink = result_buffer.make_sink();

        // Collect all results
        let mut results = Vec::new();
        while let Some(result) = result_sink.pop() {
            results.push(result);
        }
        results.sort();
        results.dedup();

        // The expected results as (start1, end1, start2, end2) for each match
        let expected_intervals = [
            vec![0, 2, 1, 1],
            vec![0, 2, 2, 2],
            vec![1, 2, 1, 1],
            vec![1, 2, 2, 2],
            vec![2, 2, 1, 1],
            vec![2, 2, 2, 2],
        ];

        let expected_ids = vec![
            vec![0, 1],
            vec![0, 1],
            vec![0, 1],
            vec![0, 1],
            vec![0, 1],
            vec![0, 1],
        ];

        verify_intervals(&results, &expected_intervals);

        verify_ids(&results, &expected_ids);
    }

    /// Test using the small automaton with input from abcd.log
    #[test]
    fn test_small_with_abcd_10() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let automaton = create_small_automaton(&state_arena, &transition_arena);
        automaton.remove_unreachable_transitions();

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        let views = sequences
            .iter()
            .map(|sequence| sequence.readable_view())
            .collect();

        let ids = vec![0, 0];

        let result_buffer = SharedBuffer::new();

        let mut matcher = FJSFilteredSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            views,
            ids.clone(),
        );

        // Inputs generated by `seq 10 | gen_abcd.awk`
        let inputs = ["d", "b", "d", "d", "d", "a", "b", "d", "b", "c"];

        // Feed all the inputs to stream 0
        let masked_output = [5, 8, 9];
        for (i, input) in inputs.into_iter().enumerate() {
            if i < 9 {
                sequences[0].append(None);
            } else {
                sequences[0].append(Some(input.to_string()));
            }
            if masked_output.contains(&i) {
                sequences[1].append(None);
            } else {
                sequences[1].append(Some(input.to_string()));
            }
            matcher.consume_input();
        }

        // Set EOF for the input streams
        sequences[0].close();
        matcher.consume_input();
        sequences[1].close();
        matcher.consume_input();

        let mut result_sink = result_buffer.make_sink();

        // Collect all results
        let mut results = Vec::new();
        while let Some(result) = result_sink.pop() {
            results.push(result);
        }

        // The expected results as (start1, end1, start2, end2) for each match
        let expected_intervals = [
            vec![9, 9, 0, 0],
            vec![9, 9, 0, 0],
            vec![9, 9, 1, 2],
            vec![9, 9, 2, 2],
            vec![9, 9, 3, 3],
            vec![9, 9, 4, 4],
            vec![9, 9, 6, 7],
            vec![9, 9, 7, 7],
        ];

        let expected_ids = vec![
            vec![0, 0],
            vec![0, 0],
            vec![0, 0],
            vec![0, 0],
            vec![0, 0],
            vec![0, 0],
            vec![0, 0],
            vec![0, 0],
        ];

        verify_intervals(&results, &expected_intervals);

        verify_ids(&results, &expected_ids);
    }

    #[test]
    fn test_stuttering_robustness() {
        use crate::result_notifier::MatchingInterval;
        use crate::{
            automata::NFAH, automata_runner::AppendOnlySequence,
            result_notifier::SharedBufferResultNotifier, shared_buffer::SharedBuffer,
        };
        use typed_arena::Arena;

        // Create arenas and automaton with 2 dimensions.
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

        // Create states
        let s0 = automaton.add_state(true, false);
        let s1 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, false);
        let s4 = automaton.add_state(false, false);
        let s5 = automaton.add_state(false, true);

        // Add transitions
        automaton.add_nfah_transition(s0, "a_0".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "a_0".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "a_0".to_string(), 1, s1);
        automaton.add_nfah_transition(s1, "a_0".to_string(), 1, s0);
        automaton.add_nfah_transition(s1, "a_1".to_string(), 1, s5);

        automaton.add_nfah_transition(s0, "a_1".to_string(), 0, s2);
        automaton.add_nfah_transition(s2, "a_1".to_string(), 0, s2);
        automaton.add_nfah_transition(s2, "a_1".to_string(), 1, s2);
        automaton.add_nfah_transition(s2, "a_1".to_string(), 1, s0);
        automaton.add_nfah_transition(s2, "a_0".to_string(), 1, s5);

        automaton.add_nfah_transition(s0, "b_0".to_string(), 0, s3);
        automaton.add_nfah_transition(s3, "b_0".to_string(), 0, s3);
        automaton.add_nfah_transition(s3, "b_0".to_string(), 1, s3);
        automaton.add_nfah_transition(s3, "b_0".to_string(), 1, s0);
        automaton.add_nfah_transition(s3, "b_1".to_string(), 1, s5);

        automaton.add_nfah_transition(s0, "b_1".to_string(), 0, s4);
        automaton.add_nfah_transition(s4, "b_1".to_string(), 0, s4);
        automaton.add_nfah_transition(s4, "b_1".to_string(), 1, s4);
        automaton.add_nfah_transition(s4, "b_1".to_string(), 1, s0);
        automaton.add_nfah_transition(s4, "b_0".to_string(), 1, s5);

        automaton.remove_unreachable_transitions();

        // Create two input views from one input sequence
        let mut sequence = AppendOnlySequence::new();
        let input_views = vec![sequence.readable_view(), sequence.readable_view()];

        // Use ids [0, 0]
        let ids = vec![0, 0];
        let result_buffer = SharedBuffer::new();

        // Create the matcher using our automaton.
        let mut matcher = FJSFilteredSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            input_views,
            ids.clone(),
        );

        sequence.append(Some("b_0".to_string()));
        matcher.consume_input();
        sequence.append(Some("b_1".to_string()));
        matcher.consume_input();
        sequence.close();
        matcher.consume_input();

        // Collect and verify the match results.
        let mut result_sink = result_buffer.make_sink();
        let mut results = Vec::new();
        while let Some(result) = result_sink.pop() {
            results.push(result);
        }
        results.sort();
        results.dedup();

        let expected_intervals = [vec![0, 0, 0, 1], vec![0, 0, 1, 1], vec![1, 1, 0, 0]];

        let expected_ids = vec![ids.clone(), ids.clone(), ids.clone()];

        verify_intervals(&results, &expected_intervals);

        verify_ids(&results, &expected_ids);
    }
}
