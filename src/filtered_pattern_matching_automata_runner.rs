use log::trace;

use crate::automata::{NFAHState, NFAHTransition, ValidLabel, NFAH};
use crate::automata_runner::ReadableView;
use std::{
    cell::Ref,
    collections::{hash_set::Iter, HashSet},
};

pub struct FilteredPatternMatchingAutomataRunner<'a> {
    /// The current set of configurations of type `PatternMatchingAutomataConfiguration`.
    automaton: &'a NFAH<'a>,
    /// Each configuration is unique in the set (thanks to `Hash`/`Eq`).
    pub current_configurations: HashSet<FilteredPatternMatchingAutomataConfiguration<'a>>,
    /// The list of IDs of words we are handling in this configuration.
    ids: Vec<usize>,
}

impl<'a> FilteredPatternMatchingAutomataRunner<'a> {
    /// Constructs a new `FilteredPatternMatchingAutomataRunner` by inserting configurations for
    /// each initial state of the given `automaton`.
    ///
    /// # Arguments
    ///
    /// * `automaton` - The automaton containing states and transitions.
    ///
    /// # Returns
    ///
    /// A new `FilteredPatternMatchingAutomataRunner` with initial configurations set up.
    pub fn new(automaton: &'a NFAH<'a>, ids: Vec<usize>) -> Self {
        assert_eq!(automaton.dimensions, ids.len());
        let current_configurations = HashSet::new();
        Self {
            automaton,
            current_configurations,
            ids,
        }
    }

    /// Returns the final configurations in the current set.
    pub fn get_final_configurations(
        &self,
    ) -> Vec<&FilteredPatternMatchingAutomataConfiguration<'a>> {
        self.current_configurations
            .iter()
            .filter(|c| c.is_final())
            .collect()
    }

    /// Removes all configurations that are not in a waiting state.
    pub fn remove_non_waiting_configurations(&mut self) {
        self.current_configurations.retain(|c| c.is_waiting());
    }

    /// Removes all configurations that are not in a waiting state.
    pub fn remove_masked_configurations(&mut self) {
        self.current_configurations.retain(|c| !c.is_masked());
    }

    /// Inserts a new configuration into the `HashSet`. Duplicate configurations
    /// (i.e., those that are `Eq`) will be automatically skipped.
    pub fn insert(&mut self, configuration: FilteredPatternMatchingAutomataConfiguration<'a>) {
        self.current_configurations.insert(configuration);
    }

    /// Returns the number of unique configurations in the `HashSet`.
    pub fn len(&self) -> usize {
        self.current_configurations.len()
    }

    /// Returns an iterator over the current configurations in the `HashSet`.
    pub fn iter(&mut self) -> Iter<FilteredPatternMatchingAutomataConfiguration<'a>> {
        self.current_configurations.iter()
    }

    /// Inserts new configurations for each initial state of the given automaton,
    /// using the provided `input_sequence`.
    pub fn insert_from_initial_states(
        &mut self,
        input_sequence: Vec<ReadableView<Option<String>>>,
    ) {
        if self.automaton.dimensions != input_sequence.len() {
            panic!(
                "Input sequence dimensions do not match automaton dimensions: expected {}, got {}",
                self.automaton.dimensions,
                input_sequence.len()
            );
        }

        for initial_state in self.automaton.initial_states.iter() {
            let config = FilteredPatternMatchingAutomataConfiguration::new(
                initial_state,
                input_sequence.clone(),
                self.ids.clone(),
            );
            self.current_configurations.insert(config);
        }
    }

    /// Consumes the input sequence and move to the successors.
    ///
    /// Returns `true` if the configuration set has updated.
    pub fn consume(&mut self) -> bool {
        let initial_size = self.len();
        let mut current_size = 0;
        while current_size != self.len() {
            current_size = self.len();
            let mut new_configurations = Vec::new();

            // Collect successors from every configuration we currently have.
            for current_configuration in self.iter() {
                new_configurations.append(&mut current_configuration.successors());
            }

            // Insert all newly discovered configurations back into our set.
            for c in new_configurations.drain(..) {
                self.insert(c);
            }
        }
        trace!(
            "initial_size, current_size: {}, {}",
            initial_size,
            current_size
        );
        initial_size != current_size
    }
}

fn masked_head(readable_view: &ReadableView<Option<String>>) -> bool {
    !readable_view.is_empty() && readable_view.readable_slice()[0].is_none()
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct FilteredPatternMatchingAutomataConfiguration<'a> {
    /// The current state of the automaton.
    pub current_state: &'a NFAHState<'a>,

    /// A vector of readable views over the input(s) that the automaton consumes.
    /// Each `ReadableView<String>` tracks how far the automaton has read.
    /// For example, if this vector has length 2, we are dealing with a 2D input.
    pub input_sequence: Vec<ReadableView<Option<String>>>,

    pub matching_begin: Vec<usize>,

    /// The list of IDs of words we are handling in this configuration.
    pub ids: Vec<usize>,
}

impl<'a> FilteredPatternMatchingAutomataConfiguration<'a> {
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
        input_sequence: Vec<ReadableView<Option<String>>>,
        ids: Vec<usize>,
    ) -> Self {
        let matching_begin = input_sequence.iter().map(|s| s.start).collect();
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

    pub fn is_masked(&self) -> bool {
        self.input_sequence.iter().any(masked_head)
    }

    pub fn dimensions(&self) -> usize {
        self.input_sequence.len()
    }

    pub fn transitions(&self) -> Ref<Vec<&NFAHTransition<'a>>> {
        self.current_state.transitions.borrow()
    }

    pub fn duplicate(&self, current_state: &'a NFAHState<'a>) -> Self {
        Self {
            current_state,
            input_sequence: self.input_sequence.clone(),
            matching_begin: self.matching_begin.clone(),
            ids: self.ids.clone(),
        }
    }

    pub fn input_head(&self, i: usize) -> Option<String> {
        if i < self.input_sequence.len() {
            let head = self.input_sequence[i].readable_slice();
            if head.is_empty() {
                None
            } else {
                head[0].clone()
            }
        } else {
            None
        }
    }

    pub fn input_advance(&mut self, i: usize, count: usize) {
        if i < self.input_sequence.len() {
            self.input_sequence[i].advance_readable(count);
        }
    }

    /// Computes all possible successor configurations from the current one
    /// by applying each outgoing transition of the current state.
    ///
    /// Returns a list of all valid successor configurations. A successor is
    /// considered valid if for every dimension of the transition’s action:
    /// - If the transition’s action is non-empty, it must match the head of
    ///   the corresponding input sequence,
    /// - Then that matching symbol is consumed (the input is advanced).
    fn successors(&self) -> Vec<Self> {
        let mut successors_set = HashSet::new();
        for transition in self.transitions().iter() {
            // Ensure transition.var is within bounds.
            transition.label.validate(self.dimensions());
            // Create a tentative successor configuration.
            let mut successor = self.duplicate(transition.next_state);
            // Check if the transition is applicable.
            let head = self.input_head(transition.label.1);
            if head.is_none() || transition.label.0 != head.unwrap() {
                continue;
            }
            // Consume one symbol on the input for the given dimension.
            successor.input_advance(transition.label.1, 1);
            // Insert into the HashSet for deduplication.
            successors_set.insert(successor);
        }
        successors_set.into_iter().collect()
    }
}
