use crate::automata::{NFAHState, NFAHTransition, NFAH};
use crate::automata_runner::{AppendOnlySequence, NFAHConfiguration, NFAHRunner, ReadableView};
use crate::result_notifier::{MatchingInterval, ResultNotifier};
use itertools::Itertools;
use std::cell::Ref;
use std::collections::hash_set::Iter;
use std::collections::HashSet;
use std::hash::Hash;

// Trait of pattern matching algorithms
pub trait HyperPatternMatching {
    // Feed a string-valued action to the given track
    fn feed(&mut self, action: &str, track: usize);

    fn dimensions(&self) -> usize;

    fn consume_remaining(&mut self);

    fn set_eof(&mut self, track: usize);
}

pub struct PatternMatchingAutomataRunner<'a> {
    /// The current set of configurations of type `PatternMatchingAutomataConfiguration`.
    automaton: &'a NFAH<'a>,
    /// Each configuration is unique in the set (thanks to `Hash`/`Eq`).
    pub current_configurations: HashSet<PatternMatchingAutomataConfiguration<'a>>,
    /// The possible streams to be used
    views: Vec<ReadableView<String>>,
}

impl<'a> PatternMatchingAutomataRunner<'a> {
    /// Constructs a new `PatternMatchingAutomataRunner` by inserting configurations for
    /// each initial state of the given `automaton`.
    ///
    /// # Arguments
    ///
    /// * `automaton` - The automaton containing states and transitions.
    ///
    /// # Returns
    ///
    /// A new `PatternMatchingAutomataRunner` with initial configurations set up.
    pub fn new(automaton: &'a NFAH<'a>, views: Vec<ReadableView<String>>) -> Self {
        let current_configurations = HashSet::new();
        Self {
            automaton,
            current_configurations,
            views,
        }
    }

    /// Returns the final configurations in the current set.
    pub fn get_final_configurations(&self) -> Vec<&PatternMatchingAutomataConfiguration<'a>> {
        self.current_configurations
            .iter()
            .filter(|c| c.is_final())
            .collect()
    }

    /// Removes all configurations that are not in a waiting state.
    pub fn remove_non_waiting_configurations(&mut self) {
        self.current_configurations.retain(|c| c.is_waiting());
    }
}

impl<'a> NFAHRunner<'a, PatternMatchingAutomataConfiguration<'a>>
    for PatternMatchingAutomataRunner<'a>
{
    /// Inserts a new configuration into the `HashSet`. Duplicate configurations
    /// (i.e., those that are `Eq`) will be automatically skipped.
    fn insert(&mut self, configuration: PatternMatchingAutomataConfiguration<'a>) {
        self.current_configurations.insert(configuration);
    }

    /// Returns the number of unique configurations in the `HashSet`.
    fn len(&self) -> usize {
        self.current_configurations.len()
    }

    /// Returns an iterator over the current configurations in the `HashSet`.
    fn iter(&mut self) -> Iter<PatternMatchingAutomataConfiguration<'a>> {
        self.current_configurations.iter()
    }

    /// Inserts new configurations for each initial state of the given automaton,
    /// using the provided `input_sequence`.
    fn insert_from_initial_states(&mut self, input_sequence: Vec<ReadableView<String>>) {
        if self.automaton.dimensions != input_sequence.len() {
            panic!(
                "Input sequence dimensions do not match automaton dimensions: expected {}, got {}",
                self.automaton.dimensions,
                input_sequence.len()
            );
        }

        // Preallocate with capacity
        let mut ids = Vec::with_capacity(input_sequence.len());

        for sequence in &input_sequence {
            for i in 0..self.views.len() {
                if self.views[i].same_data(sequence) {
                    ids.push(i);
                    break;
                }
            }
        }

        // Reserve space in the HashSet for the new configurations
        self.current_configurations
            .reserve(self.automaton.initial_states.len());

        for initial_state in self.automaton.initial_states.iter() {
            let config = PatternMatchingAutomataConfiguration::new(
                initial_state,
                input_sequence.clone(),
                ids.clone(),
            );
            self.current_configurations.insert(config);
        }
    }
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct PatternMatchingAutomataConfiguration<'a> {
    /// The current state of the automaton.
    pub current_state: &'a NFAHState<'a>,

    /// A vector of readable views over the input(s) that the automaton consumes.
    /// Each `ReadableView<String>` tracks how far the automaton has read.
    /// For example, if this vector has length 2, we are dealing with a 2D input.
    pub input_sequence: Vec<ReadableView<String>>,

    pub matching_begin: Vec<usize>,

    /// The list of IDs of words we are handling in this configuration.
    pub ids: Vec<usize>,
}

impl<'a> PatternMatchingAutomataConfiguration<'a> {
    /// Creates a new `PatternMatchingAutomataConfiguration` from the given state and
    /// list of `ReadableView`s for each input dimension.
    ///
    /// # Arguments
    ///
    /// * `current_state` - The automaton state this configuration points to.
    /// * `input_sequence` - A vector of `ReadableView<String>` representing
    ///   the input stream for the automaton.
    pub fn new(
        current_state: &'a NFAHState<'a>,
        input_sequence: Vec<ReadableView<String>>,
        ids: Vec<usize>,
    ) -> Self {
        // Preallocate with the exact capacity needed
        let mut matching_begin = Vec::with_capacity(input_sequence.len());
        for s in &input_sequence {
            matching_begin.push(s.start);
        }

        Self {
            current_state,
            input_sequence,
            matching_begin,
            ids,
        }
    }

    pub fn is_final(&self) -> bool {
        self.current_state.is_final
    }

    pub fn is_waiting(&self) -> bool {
        self.input_sequence
            .iter()
            .any(|s| !s.is_closed() && s.is_empty())
    }
}

impl<'a> NFAHConfiguration<'a> for PatternMatchingAutomataConfiguration<'a> {
    fn dimensions(&self) -> usize {
        self.input_sequence.len()
    }

    fn transitions(&self) -> Ref<Vec<&NFAHTransition<'a>>> {
        self.current_state.transitions.borrow()
    }

    fn duplicate(&self, current_state: &'a NFAHState<'a>) -> Self {
        // Create new vectors with preallocated capacity
        let input_sequence_len = self.input_sequence.len();
        let matching_begin_len = self.matching_begin.len();
        let ids_len = self.ids.len();

        let mut input_sequence = Vec::with_capacity(input_sequence_len);
        let mut matching_begin = Vec::with_capacity(matching_begin_len);
        let mut ids = Vec::with_capacity(ids_len);

        // Copy elements
        input_sequence.extend_from_slice(&self.input_sequence);
        matching_begin.extend_from_slice(&self.matching_begin);
        ids.extend_from_slice(&self.ids);

        Self {
            current_state,
            input_sequence,
            matching_begin,
            ids,
        }
    }

    fn input_head(&self, i: usize) -> Option<String> {
        if i < self.input_sequence.len() {
            let head = self.input_sequence[i].readable_slice();
            if head.is_empty() {
                None
            } else {
                Some(head[0].clone())
            }
        } else {
            None
        }
    }

    fn input_advance(&mut self, i: usize, count: usize) {
        if i < self.input_sequence.len() {
            self.input_sequence[i].advance_readable(count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata_runner::AppendOnlySequence;
    use typed_arena::Arena;

    #[test]
    fn test_automata_configuration_successors() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = NFAH::new(&state_arena, &transition_arena, 2);

        let s1 = automata.add_state(true, false);
        let s12 = automata.add_state(false, false);
        let s2 = automata.add_state(false, false);
        let s13 = automata.add_state(false, false);
        let s3 = automata.add_state(false, true);

        automata.add_nfah_transition(s1, "a".to_string(), 0, s12);
        automata.add_nfah_transition(s12, "b".to_string(), 1, s2);
        automata.add_nfah_transition(s1, "a".to_string(), 0, s1);
        automata.add_nfah_transition(s1, "b".to_string(), 1, s1);
        automata.add_nfah_transition(s1, "c".to_string(), 0, s13);
        automata.add_nfah_transition(s13, "d".to_string(), 1, s3);

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].append("c".to_string());
        sequences[1].append("d".to_string());

        let config = PatternMatchingAutomataConfiguration::new(
            s1,
            sequences.iter().map(|s| s.readable_view()).collect(),
            vec![0, 1],
        );

        let successors = config.successors();
        println!("{:?}", successors);
        assert_eq!(successors.len(), 3);

        // Moves to s12 using (a, 0)
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut configuration =
                PatternMatchingAutomataConfiguration::new(s12, view, vec![0, 1]);
            configuration.input_advance(0, 1);
            assert!(successors.contains(&configuration));
        }
        // Moves to s1 using (a, 0)
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut configuration = PatternMatchingAutomataConfiguration::new(s1, view, vec![0, 1]);
            configuration.input_advance(0, 1);
            assert!(successors.contains(&configuration));
        }
        // Moves to s1 using (b, 1)
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut configuration = PatternMatchingAutomataConfiguration::new(s1, view, vec![0, 1]);
            configuration.input_advance(1, 1);
            assert!(successors.contains(&configuration));
        }
    }

    #[test]
    fn test_automata_runner() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = NFAH::new(&state_arena, &transition_arena, 2);

        let s1 = automata.add_state(true, false);
        let s12 = automata.add_state(false, false);
        let s2 = automata.add_state(false, false);
        let s13 = automata.add_state(false, false);
        let s3 = automata.add_state(false, true);

        automata.add_nfah_transition(s1, "a".to_string(), 0, s12);
        automata.add_nfah_transition(s12, "b".to_string(), 1, s2);
        automata.add_nfah_transition(s1, "a".to_string(), 0, s1);
        automata.add_nfah_transition(s1, "b".to_string(), 1, s1);
        automata.add_nfah_transition(s1, "c".to_string(), 0, s13);
        automata.add_nfah_transition(s13, "d".to_string(), 1, s3);

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].append("c".to_string());
        sequences[1].append("d".to_string());
        let as_readable_view = sequences.iter().map(|s| s.readable_view()).collect();

        let mut runner = PatternMatchingAutomataRunner::new(&automata, as_readable_view);
        runner.insert_from_initial_states(sequences.iter().map(|s| s.readable_view()).collect());
        runner.consume();

        let successors = runner.current_configurations;

        assert_eq!(successors.len(), 10);

        // No transition
        assert!(
            successors.contains(&PatternMatchingAutomataConfiguration::new(
                s1,
                sequences.iter().map(|s| s.readable_view()).collect(),
                vec![0, 1],
            ))
        );

        // Self loops
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut config = PatternMatchingAutomataConfiguration::new(s1, view, vec![0, 1]);
            config.input_advance(0, 1);
            assert!(successors.contains(&config));
        }
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut config = PatternMatchingAutomataConfiguration::new(s1, view, vec![0, 1]);
            config.input_advance(1, 1);
            assert!(successors.contains(&config));
        }
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut config = PatternMatchingAutomataConfiguration::new(s1, view, vec![0, 1]);
            config.input_advance(0, 1);
            config.input_advance(1, 1);
            assert!(successors.contains(&config));
        }

        // Moves to s12
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut config = PatternMatchingAutomataConfiguration::new(s12, view, vec![0, 1]);
            config.input_advance(0, 1);
            assert!(successors.contains(&config));
        }

        // Moves to s12 after consuming the first element of the second dimension with a self-loop
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut config = PatternMatchingAutomataConfiguration::new(s12, view, vec![0, 1]);
            config.input_advance(0, 1);
            config.input_advance(1, 1);
            assert!(successors.contains(&config));
        }

        // Moves to s2 via s12
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut config = PatternMatchingAutomataConfiguration::new(s2, view, vec![0, 1]);
            config.input_advance(0, 1);
            config.input_advance(1, 1);
            assert!(successors.contains(&config));
        }

        // Moves to s13 after consuming the first element of the first dimension with the self loops
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut config = PatternMatchingAutomataConfiguration::new(s13, view, vec![0, 1]);
            config.input_advance(0, 2);
            assert!(successors.contains(&config));
        }

        // Moves to s13 after consuming the first elements with the self loops
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut config = PatternMatchingAutomataConfiguration::new(s13, view, vec![0, 1]);
            config.input_advance(0, 2);
            config.input_advance(1, 1);
            assert!(successors.contains(&config));
        }

        // Moves to s3 via s13 after consuming the first elements with the self loops
        {
            let view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            let mut config = PatternMatchingAutomataConfiguration::new(s3, view, vec![0, 1]);
            config.input_advance(0, 2);
            config.input_advance(1, 2);
            assert!(successors.contains(&config));
        }
    }
}
