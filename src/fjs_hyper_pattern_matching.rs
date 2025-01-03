use crate::automata::NFAH;
use crate::automata_runner::{AppendOnlySequence, NFAHRunner};
use crate::hyper_pattern_matching::{HyperPatternMatching, PatternMatchingAutomataRunner};
use crate::kmp_skip_values::KMPSkipValues;
use crate::naive_hyper_pattern_matching::StartPosition;
use crate::quick_search_skip_values::QuickSearchSkipValues;
use crate::result_notifier::{MatchingInterval, ResultNotifier};
use itertools::Itertools;
use log::debug;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

/// A struct to store the skipped starting positions
struct SkippedStartingPositions {
    /// Number of variables in this hyper pattern matching
    variable_size: usize,
    /// Number of words to be monitored
    sequence_size: usize,
    /// Variable -> Word -> Set(Positions)
    skipped_starting_positions: Vec<Vec<HashSet<usize>>>,
}

impl SkippedStartingPositions {
    fn new(variable_size: usize, sequence_size: usize) -> Self {
        let skipped_starting_positions = (0..variable_size)
            .map(|_| (0..sequence_size).map(|_| HashSet::new()).collect_vec())
            .collect_vec();
        Self {
            variable_size,
            sequence_size,
            skipped_starting_positions,
        }
    }

    fn insert(&mut self, var: usize, word: usize, position: usize) {
        if var >= self.variable_size || word >= self.sequence_size {
            panic!(
                "Out of range given (var: {}, word: {}) for (var_size: {}, word_size: {})",
                var, word, self.variable_size, self.sequence_size
            );
        }
        self.skipped_starting_positions[var][word].insert(position);
    }

    fn contains(&self, var: usize, word: usize, position: usize) -> bool {
        if var >= self.variable_size || word >= self.sequence_size {
            panic!(
                "Out of range given (var: {}, word: {}) for (var_size: {}, word_size: {})",
                var, word, self.variable_size, self.sequence_size
            );
        }
        self.skipped_starting_positions[var][word].contains(&position)
    }

    fn matchable(&self, start_position: &StartPosition, id: &[usize]) -> bool {
        assert_eq!(start_position.start_indices.len(), id.len());
        for i in 0..start_position.start_indices.len() {
            if self.contains(id[i], i, start_position.start_indices[i]) {
                return false;
            }
        }
        true
    }
}

pub struct FJSHyperPatternMatching<'a, Notifier: ResultNotifier> {
    automata_runner: PatternMatchingAutomataRunner<'a>,
    notifier: Notifier,
    sequences: Vec<AppendOnlySequence<String>>,
    read_size: Vec<usize>,
    waiting_queues: HashMap<Vec<usize>, Vec<Reverse<StartPosition>>>,
    /// The set of ignored starting positions by the skip values
    skipped_starting_positions: SkippedStartingPositions,
    quick_search_skip_value: QuickSearchSkipValues,
    kmp_skip_value: KMPSkipValues<'a>,
    /// Either we reached the end of the sequences
    eof: Vec<bool>,
}

impl<'a, Notifier: ResultNotifier> FJSHyperPatternMatching<'a, Notifier> {
    pub fn new(
        automaton: &'a NFAH<'a>,
        notifier: Notifier,
        sequences: Vec<AppendOnlySequence<String>>,
    ) -> Self {
        let mut automata_runner = PatternMatchingAutomataRunner::new(automaton);
        let read_size = vec![0; sequences.len()];
        let eof = vec![false; sequences.len()];
        let start_indices = vec![0; automaton.dimensions];
        let successors = StartPosition { start_indices }
            .immediate_successors()
            .map(Reverse)
            .collect_vec();
        let ranges = vec![0..sequences.len(); automaton.dimensions];
        let ids = ranges.into_iter().multi_cartesian_product().collect_vec();
        let mut waiting_queue = successors;
        waiting_queue.sort();
        waiting_queue.dedup();
        let mut waiting_queues = HashMap::with_capacity(ids.len());
        for id in ids {
            let input_sequence = id
                .iter()
                .map(|i| {
                    let mut view = sequences[*i].readable_view();
                    view.advance_readable(0);
                    view
                })
                .collect_vec();
            waiting_queues.insert(id.clone(), waiting_queue.clone());
            automata_runner.insert_from_initial_states(input_sequence, id);
        }

        let skipped_starting_positions =
            SkippedStartingPositions::new(automaton.dimensions, sequences.len());

        Self {
            automata_runner,
            notifier,
            sequences,
            read_size,
            waiting_queues,
            eof,
            skipped_starting_positions,
            quick_search_skip_value: QuickSearchSkipValues::new(automaton),
            kmp_skip_value: KMPSkipValues::new(automaton),
        }
    }

    pub fn in_range(&self, start_position: &StartPosition, ids: &[usize]) -> bool {
        assert_eq!(start_position.start_indices.len(), ids.len());
        for i in 0..start_position.start_indices.len() {
            if self.eof[ids[i]] && start_position.start_indices[i] >= self.read_size[ids[i]] {
                return false;
            }
        }
        true
    }
}

impl<Notifier: ResultNotifier> HyperPatternMatching for FJSHyperPatternMatching<'_, Notifier> {
    fn feed(&mut self, action: &str, track: usize) {
        self.sequences[track].append(action.to_string());
        self.read_size[track] += 1;
        self.automata_runner.consume();
        let final_configurations = self.automata_runner.get_final_configurations();
        let dimensions = self.dimensions();
        final_configurations.iter().for_each(|c| {
            let mut intervals = Vec::with_capacity(dimensions);
            for i in 0..dimensions {
                let begin = c.matching_begin[i];
                let end = c.input_sequence[i].start - 1;
                intervals.push(MatchingInterval::new(begin, end));
            }
            self.notifier.notify(&intervals, &c.ids);
        });
        // Apply KMP-style skip values
        self.automata_runner
            .current_configurations
            .iter()
            .for_each(|c| {
                for i in 0..c.ids.len() {
                    if let Some(&skip_value) =
                        self.kmp_skip_value.skip_values[i].get(c.current_state)
                    {
                        for j in 1..skip_value {
                            self.skipped_starting_positions.insert(
                                i,
                                c.ids[i],
                                c.matching_begin[i] + j,
                            );
                        }
                    }
                }
            });
        self.automata_runner.remove_non_waiting_configurations();
        let current_ids = self
            .automata_runner
            .current_configurations
            .iter()
            .map(|c| c.ids.clone())
            .collect::<HashSet<Vec<usize>>>();
        let keys = self.waiting_queues.keys().cloned().collect_vec();
        for id in keys {
            if !current_ids.contains(&id) {
                let new_position = {
                    let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                    fn find_new_position(
                        id: &Vec<usize>,
                        waiting_queue: &mut Vec<Reverse<StartPosition>>,
                        skipped_starting_positions: &mut SkippedStartingPositions,
                        quick_search_skip_values: &QuickSearchSkipValues,
                        sequences: &Vec<AppendOnlySequence<String>>,
                    ) -> Option<Reverse<StartPosition>> {
                        let new_position_candidate = waiting_queue.pop();
                        match new_position_candidate {
                            Some(Reverse(found_new_position)) => {
                                let start_indices = &found_new_position.start_indices;
                                assert!(id.len() == start_indices.len());
                                if !skipped_starting_positions.matchable(&found_new_position, id) {
                                    // When we already know that this starting position can be skipped
                                    find_new_position(
                                        id,
                                        waiting_queue,
                                        skipped_starting_positions,
                                        quick_search_skip_values,
                                        sequences,
                                    )
                                } else {
                                    for var in 0..id.len() {
                                        let w = id[var];
                                        let sequence = &sequences[w];
                                        let start_index = start_indices[var];
                                        let shortest_matching_length = quick_search_skip_values
                                            .shortest_accepted_word_length_map[var];
                                        if shortest_matching_length > 0 {
                                            let shortest_end_index =
                                                start_index + shortest_matching_length - 1;
                                            let next_index = start_index + shortest_matching_length;
                                            let last_accepted_words =
                                                &quick_search_skip_values.last_accepted_word[var];
                                            if sequence.len()
                                                < start_index + shortest_matching_length
                                                && !last_accepted_words.contains(
                                                    &sequence.get(shortest_end_index).unwrap(),
                                                )
                                            {
                                                // This start position is ignorable according to quick search
                                                let skipped_width = quick_search_skip_values
                                                    .skip_value(
                                                        &sequence.get(next_index).unwrap(),
                                                        var,
                                                    );
                                                (0..skipped_width).for_each(|i| {
                                                    skipped_starting_positions.insert(
                                                        var,
                                                        w,
                                                        start_index + i,
                                                    );
                                                });
                                                return find_new_position(
                                                    id,
                                                    waiting_queue,
                                                    skipped_starting_positions,
                                                    quick_search_skip_values,
                                                    sequences,
                                                );
                                            }
                                        }
                                    }
                                    Some(Reverse(found_new_position))
                                }
                            }
                            None => None,
                        }
                    }
                    find_new_position(
                        &id,
                        waiting_queue,
                        &mut self.skipped_starting_positions,
                        &self.quick_search_skip_value,
                        &self.sequences,
                    )
                };
                // Start new matching trial
                if let Some(new_position) = new_position {
                    let mut valid_successors = new_position
                        .0
                        .immediate_successors_filtered(|successor| {
                            self.in_range(successor, &id)
                                && self.skipped_starting_positions.matchable(successor, &id)
                        })
                        .map(Reverse)
                        .collect_vec();
                    // Put the successors to the waiting queue
                    let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                    waiting_queue.append(&mut valid_successors);
                    waiting_queue.sort();
                    waiting_queue.dedup();
                    debug!("[FJSHyperPatternMatching::feed] Start new matching trial from {:?} for {:?})", new_position, id);
                    let input_sequence = id
                        .iter()
                        .map(|&i| {
                            let mut view = self.sequences[i].readable_view();
                            view.advance_readable(new_position.0.start_indices[i]);
                            view
                        })
                        .collect_vec();
                    self.automata_runner
                        .insert_from_initial_states(input_sequence, id)
                }
            }
        }
    }

    fn dimensions(&self) -> usize {
        self.sequences.len()
    }

    fn consume_remaining(&mut self) {
        debug!("Call FJSHyperPatternMatching::consume_remaining");
        self.automata_runner.consume();
        let final_configurations = self.automata_runner.get_final_configurations();
        let dimensions = self.dimensions();
        final_configurations.iter().for_each(|c| {
            let mut intervals = Vec::with_capacity(dimensions);
            for i in 0..dimensions {
                let begin = c.matching_begin[i];
                let end = c.input_sequence[i].start - 1;
                intervals.push(MatchingInterval::new(begin, end));
            }
            self.notifier.notify(&intervals, &c.ids);
        });
        while self.waiting_queues.values().any(|f| !f.is_empty()) {
            self.automata_runner.current_configurations.clear();
            let keys = self.waiting_queues.keys().cloned().collect_vec();
            for id in keys {
                let new_position = {
                    let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                    waiting_queue.pop()
                };
                // Start new matching trial
                if let Some(new_position) = new_position {
                    let mut valid_successors = new_position
                        .0
                        .immediate_successors_filtered(|successor| {
                            self.in_range(successor, &id)
                                && self.skipped_starting_positions.matchable(successor, &id)
                        })
                        .map(Reverse)
                        .collect_vec();
                    // Put the successors to the waiting queue
                    let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                    waiting_queue.append(&mut valid_successors);
                    waiting_queue.sort();
                    waiting_queue.dedup();
                    let input_sequence = id
                        .iter()
                        .map(|&i| {
                            let mut view = self.sequences[i].readable_view();
                            view.advance_readable(new_position.0.start_indices[i]);
                            view
                        })
                        .collect_vec();
                    self.automata_runner
                        .insert_from_initial_states(input_sequence, id)
                }
            }
            self.automata_runner.consume();
            let final_configurations = self.automata_runner.get_final_configurations();
            let dimensions = self.dimensions();
            final_configurations.iter().for_each(|c| {
                let mut intervals = Vec::with_capacity(dimensions);
                for i in 0..dimensions {
                    let begin = c.matching_begin[i];
                    let end = c.input_sequence[i].start - 1;
                    intervals.push(MatchingInterval::new(begin, end));
                }
                self.notifier.notify(&intervals, &c.ids);
            });
        }
    }

    fn set_eof(&mut self, track: usize) {
        self.sequences[track].close();
        self.eof[track] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multi_stream_reader::{MultiStreamReader, StreamSource};
    use crate::reading_scheduler::ReadingScheduler;
    use crate::result_notifier::SharedBufferResultNotifier;
    use crate::shared_buffer::SharedBuffer;
    use typed_arena::Arena;

    #[test]
    fn test_run() {
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

        let input_buffers = vec![SharedBuffer::new(), SharedBuffer::new()];
        let reader = MultiStreamReader::new(
            input_buffers
                .clone()
                .into_iter()
                .map(|buf| Box::new(buf) as Box<dyn StreamSource>)
                .collect(),
        );

        let result_buffer = SharedBuffer::new();
        let notifier = SharedBufferResultNotifier::new(result_buffer.make_source());
        let mut result_sink = result_buffer.make_sink();

        let matching = FJSHyperPatternMatching::new(
            &automaton,
            notifier,
            vec![AppendOnlySequence::new(), AppendOnlySequence::new()],
        );

        let mut scheduler = ReadingScheduler::new(matching, reader);

        input_buffers[0].push("a");
        input_buffers[1].push("b");
        input_buffers[0].push("a");
        input_buffers[1].push("b");
        input_buffers[0].push("c");
        input_buffers[1].push("d");

        scheduler.run();
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(0, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(0, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(0, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(0, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(0, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(0, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(0, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(2, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(0, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(2, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(2, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        assert!(result_sink.pop().is_none());
    }
}
