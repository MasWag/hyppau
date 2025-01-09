use crate::automata::{Automata, State, Transition};
use crate::automata_runner::{AutomataConfiguration, AutomataRunner, ReadableView};
use std::cell::Ref;
use std::collections::hash_set::Iter;
use std::collections::HashSet;

// Trait of pattern matching algorithms
trait HyperPatternMatching {
    // Feed a string-valued action to the given track
    fn feed(&mut self, action: &str, track: u32);
}

pub struct PatternMatchingAutomataRunner<'a> {
    /// The current set of configurations of type `PatternMatchingAutomataConfiguration`.
    /// Each configuration is unique in the set (thanks to `Hash`/`Eq`).
    pub current_configurations: HashSet<PatternMatchingAutomataConfiguration<'a>>,
}

impl<'a> PatternMatchingAutomataRunner<'a> {
    /// Constructs a new `PatternMatchingAutomataRunner` by inserting configurations for
    /// each initial state of the given `automaton`.
    ///
    /// # Arguments
    ///
    /// * `automaton` - The automaton containing states and transitions.
    /// * `input_sequence` - A vector of input views to be associated with each
    ///   newly created configuration.
    ///
    /// # Returns
    ///
    /// A new `PatternMatchingAutomataRunner` with initial configurations set up.
    pub fn new(automaton: &'a Automata<'a>, input_sequence: Vec<ReadableView<String>>) -> Self {
        let mut current_configurations = HashSet::new();
        for initial_state in automaton.initial_states.iter() {
            let config =
                PatternMatchingAutomataConfiguration::new(initial_state, input_sequence.clone());
            current_configurations.insert(config);
        }
        Self {
            current_configurations,
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

impl<'a> AutomataRunner<'a, PatternMatchingAutomataConfiguration<'a>>
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
    fn insert_from_initial_states(
        &mut self,
        automaton: &'a Automata<'a>,
        input_sequence: Vec<ReadableView<String>>,
    ) {
        for initial_state in automaton.initial_states.iter() {
            let config =
                PatternMatchingAutomataConfiguration::new(initial_state, input_sequence.clone());
            self.current_configurations.insert(config);
        }
    }
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct PatternMatchingAutomataConfiguration<'a> {
    /// The current state of the automaton.
    pub current_state: &'a State<'a>,

    /// A vector of readable views over the input(s) that the automaton consumes.
    /// Each `ReadableView<String>` tracks how far the automaton has read.
    /// For example, if this vector has length 2, we are dealing with a 2D input.
    pub input_sequence: Vec<ReadableView<String>>,

    pub matching_begin: Vec<usize>,
}

impl<'a> PatternMatchingAutomataConfiguration<'a> {
    /// Creates a new `SimpleAutomataConfiguration` from the given state and
    /// list of `ReadableView`s for each input dimension.
    ///
    /// # Arguments
    ///
    /// * `current_state` - The automaton state this configuration points to.
    /// * `input_sequence` - A vector of `ReadableView<String>` representing
    ///   the input stream for the automaton.
    pub fn new(current_state: &'a State<'a>, input_sequence: Vec<ReadableView<String>>) -> Self {
        let matching_begin = input_sequence.iter().map(|s| s.start).collect();
        Self {
            current_state,
            input_sequence,
            matching_begin: matching_begin,
        }
    }

    pub fn is_final(&self) -> bool {
        self.current_state.is_final
    }

    pub fn is_waiting(&self) -> bool {
        self.input_sequence.iter().any(|s| s.is_empty())
    }
}

impl<'a> AutomataConfiguration<'a> for PatternMatchingAutomataConfiguration<'a> {
    fn dimensions(&self) -> usize {
        self.input_sequence.len()
    }

    fn transitions(&self) -> Ref<Vec<&Transition<'a>>> {
        self.current_state.transitions.borrow()
    }

    fn duplicate(&self, current_state: &'a State<'a>) -> Self {
        Self {
            current_state,
            input_sequence: self.input_sequence.clone(),
            matching_begin: self.matching_begin.clone(),
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
        let mut automata = Automata::new(&state_arena, &transition_arena);

        let s1 = automata.add_state(true, false);
        let s2 = automata.add_state(false, false);
        let s3 = automata.add_state(false, true);

        automata.add_transition(s1, vec!["a".to_string(), "b".to_string()], s2);
        automata.add_transition(s1, vec!["a".to_string(), "".to_string()], s1);
        automata.add_transition(s1, vec!["".to_string(), "b".to_string()], s1);
        automata.add_transition(s1, vec!["".to_string(), "".to_string()], s1);
        automata.add_transition(s1, vec!["c".to_string(), "d".to_string()], s3);

        let mut sequences = vec![AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].append("c".to_string());
        sequences[1].append("d".to_string());

        let config = PatternMatchingAutomataConfiguration::new(
            &s1,
            sequences.iter().map(|s| s.readable_view()).collect(),
        );

        let successors = config.successors();

        assert_eq!(successors.len(), 4);

        assert_eq!(successors[0].current_state, s2);
        assert_eq!(successors[1].current_state, s1);
        assert_eq!(successors[2].current_state, s1);
        assert_eq!(successors[3].current_state, s1);

        assert_eq!(*successors[0].input_sequence[0].readable_slice(), ["c"]);
        assert_eq!(*successors[0].input_sequence[1].readable_slice(), ["d"]);

        assert_eq!(*successors[1].input_sequence[0].readable_slice(), ["c"]);
        assert_eq!(
            *successors[1].input_sequence[1].readable_slice(),
            ["b", "d"]
        );

        assert_eq!(
            *successors[2].input_sequence[0].readable_slice(),
            ["a", "c"]
        );
        assert_eq!(*successors[2].input_sequence[1].readable_slice(), ["d"]);

        assert_eq!(
            *successors[3].input_sequence[0].readable_slice(),
            ["a", "c"]
        );
        assert_eq!(
            *successors[3].input_sequence[1].readable_slice(),
            ["b", "d"]
        );
    }

    #[test]
    fn test_automata_runner() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = Automata::new(&state_arena, &transition_arena);

        let s1 = automata.add_state(true, false);
        let s2 = automata.add_state(false, false);
        let s3 = automata.add_state(false, true);

        automata.add_transition(s1, vec!["a".to_string(), "b".to_string()], s2);
        automata.add_transition(s1, vec!["a".to_string(), "".to_string()], s1);
        automata.add_transition(s1, vec!["".to_string(), "b".to_string()], s1);
        automata.add_transition(s1, vec!["".to_string(), "".to_string()], s1);
        automata.add_transition(s1, vec!["c".to_string(), "d".to_string()], s3);

        let mut sequences = vec![AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].append("c".to_string());
        sequences[1].append("d".to_string());

        let mut runner = PatternMatchingAutomataRunner::new(
            &automata,
            sequences.iter().map(|s| s.readable_view()).collect(),
        );
        runner.consume();

        let successors = runner.current_configurations;

        assert_eq!(successors.len(), 6);

        // No transition
        assert!(
            successors.contains(&PatternMatchingAutomataConfiguration::new(
                s1,
                sequences.iter().map(|s| s.readable_view()).collect()
            ))
        );

        // Self loops
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            let mut config = PatternMatchingAutomataConfiguration::new(&s1, view);
            config.matching_begin = vec![0, 0];

            assert!(successors.contains(&config));
        }
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[1].advance_readable(1);
            let mut config = PatternMatchingAutomataConfiguration::new(&s1, view);
            config.matching_begin = vec![0, 0];

            assert!(successors.contains(&config));
        }
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            view[1].advance_readable(1);
            let mut config = PatternMatchingAutomataConfiguration::new(&s1, view);
            config.matching_begin = vec![0, 0];

            assert!(successors.contains(&config));
        }

        // Directly moves to s2
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            view[1].advance_readable(1);
            let mut config = PatternMatchingAutomataConfiguration::new(&s2, view);
            config.matching_begin = vec![0, 0];

            assert!(successors.contains(&config));
        }

        // Moves to s3 after consuming the first elements with the self loops
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(2);
            view[1].advance_readable(2);
            let mut config = PatternMatchingAutomataConfiguration::new(&s3, view);
            config.matching_begin = vec![0, 0];

            assert!(successors.contains(&config));
        }
    }
}
