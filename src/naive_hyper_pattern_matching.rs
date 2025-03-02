use crate::automata::NFAH;
use crate::automata_runner::{AppendOnlySequence, NFAHRunner};
use crate::hyper_pattern_matching::{HyperPatternMatching, PatternMatchingAutomataRunner};
use crate::result_notifier::{MatchingInterval, ResultNotifier};
use itertools::Itertools;
use log::trace;
use std::collections::{HashMap, HashSet};

/// the element in the waiting queue of hyper pattern matching algorithms based on priority-queue.
#[derive(Debug, Clone, Ord, Eq, PartialEq)]
pub struct StartPosition {
    /// The starting indices of the word in the pattern.
    pub start_indices: Vec<usize>,
}

impl StartPosition {
    pub fn immediate_successors(&self) -> Vec<StartPosition> {
        let mut result = Vec::with_capacity(self.start_indices.len());
        for i in 0..self.start_indices.len() {
            let mut new_start_indices = self.start_indices.clone();
            new_start_indices[i] += 1;
            result.push(StartPosition {
                start_indices: new_start_indices,
            });
        }

        result
    }
}

impl PartialOrd for StartPosition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let self_sum: usize = self.start_indices.iter().sum();
        let other_sum = other.start_indices.iter().sum();
        if self_sum < other_sum {
            Some(std::cmp::Ordering::Less)
        } else if self_sum > other_sum {
            return Some(std::cmp::Ordering::Greater);
        } else {
            return Some(self.start_indices.cmp(&other.start_indices));
        }
    }
}

pub struct NaiveHyperPatternMatching<'a, Notifier: ResultNotifier> {
    automata_runner: PatternMatchingAutomataRunner<'a>,
    notifier: Notifier,
    sequences: Vec<AppendOnlySequence<String>>,
    read_size: Vec<usize>,
    waiting_queues: HashMap<Vec<usize>, Vec<StartPosition>>,
    /// Either we reached the end of the sequences
    eof: Vec<bool>,
}

impl<'a, Notifier: ResultNotifier> NaiveHyperPatternMatching<'a, Notifier> {
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
            .into_iter()
            .collect_vec();
        let ranges = vec![0..sequences.len(); automaton.dimensions];
        let ids = ranges.into_iter().multi_cartesian_product().collect_vec();
        let mut waiting_queue = successors;
        waiting_queue.sort_by(|a, b| a.cmp(b).reverse());
        waiting_queue.dedup();
        let mut waiting_queues = HashMap::with_capacity(ids.len());
        for id in ids {
            let input_sequence = id
                .iter()
                .map(|&i| sequences[i].readable_view())
                .collect_vec();
            waiting_queues.insert(id.clone(), waiting_queue.clone());
            automata_runner.insert_from_initial_states(input_sequence, id);
        }

        Self {
            automata_runner,
            notifier,
            sequences,
            read_size,
            waiting_queues,
            eof,
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

impl<Notifier: ResultNotifier> HyperPatternMatching for NaiveHyperPatternMatching<'_, Notifier> {
    fn feed(&mut self, action: &str, track: usize) {
        trace!(
            "Call of NaiveHyperPatternMatching::feed({}, {})",
            action,
            track
        );
        self.sequences[track].append(action.to_string());
        self.read_size[track] += 1;
        self.automata_runner.consume();
        let final_configurations = self.automata_runner.get_final_configurations();
        let dimensions = self.dimensions();
        trace!(
            "{:?} matching are found in NaiveHyperPatternMatching::feed.",
            final_configurations.len()
        );
        final_configurations.iter().for_each(|c| {
            let mut intervals = Vec::with_capacity(dimensions);
            for i in 0..dimensions {
                let begin = c.matching_begin[i];
                let end = c.input_sequence[i].start - 1;
                intervals.push(MatchingInterval::new(begin, end));
            }
            self.notifier.notify(&intervals, &c.ids);
        });
        trace!(
            "Number of configurations before reduction: {:?}.",
            self.automata_runner.current_configurations.len()
        );
        self.automata_runner.remove_non_waiting_configurations();
        trace!(
            "Number of configurations after reduction: {:?}.",
            self.automata_runner.current_configurations.len()
        );
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
                    waiting_queue.pop()
                };
                // Start new matching trial
                if let Some(new_position) = new_position {
                    let mut valid_successors = new_position
                        .immediate_successors()
                        .into_iter()
                        .filter(|successor| self.in_range(successor, &id))
                        .collect_vec();
                    // Put the successors to the waiting queue
                    let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                    waiting_queue.append(&mut valid_successors);
                    waiting_queue.sort_by(|a, b| a.cmp(b).reverse());
                    waiting_queue.dedup();

                    trace!("[NaiveHyperPatternMatching::feed] Start new matching trial from {:?} for {:?})", new_position, id);
                    let input_sequence = id
                        .iter()
                        .map(|&i| {
                            let mut view = self.sequences[i].readable_view();
                            view.advance_readable(new_position.start_indices[i]);
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
                        .immediate_successors()
                        .into_iter()
                        .filter(|successor| self.in_range(successor, &id))
                        .collect_vec();
                    // Put the successors to the waiting queue
                    let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                    waiting_queue.append(&mut valid_successors);
                    waiting_queue.sort_by(|a, b| a.cmp(b).reverse());
                    waiting_queue.dedup();
                    let input_sequence = id
                        .iter()
                        .map(|&i| {
                            let mut view = self.sequences[i].readable_view();
                            view.advance_readable(new_position.start_indices[i]);
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

        let matching = NaiveHyperPatternMatching::new(
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
            assert_eq!(result.intervals[0], MatchingInterval::new(0, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
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
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
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
            assert_eq!(result.intervals[1], MatchingInterval::new(0, 2));
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

    #[test]
    fn test_start_position_order() {
        let start_positions = [
            StartPosition {
                start_indices: vec![0, 3],
            },
            StartPosition {
                start_indices: vec![2, 2],
            },
            StartPosition {
                start_indices: vec![3, 1],
            },
        ];

        for i in 0..start_positions.len() {
            for j in 0..start_positions.len() {
                if i < j {
                    assert!(start_positions[i] < start_positions[j]);
                } else if i > j {
                    assert!(start_positions[i] > start_positions[j]);
                } else {
                    assert!(start_positions[i] == start_positions[j]);
                }
            }
        }
    }
}
