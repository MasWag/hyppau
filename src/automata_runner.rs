use crate::automata::{Automata, State, Transition};
use std::cell::{Ref, RefCell};
use std::collections::hash_set::Iter;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

trait AutomataRunner<'a, C: AutomataConfiguration<'a>> {
    fn insert(&mut self, configuration: C);
    fn len(&self) -> usize;
    fn iter(&mut self) -> Iter<C>;
    fn insert_from_initial_states(
        &mut self,
        automaton: &'a Automata<'a>,
        input_sequence: Vec<ReadableView<String>>,
    );

    fn consume(&mut self) {
        let mut current_size = 0;
        while current_size != self.len() {
            current_size = self.len();
            let mut new_configurations = Vec::new();
            for current_configuration in self.iter() {
                new_configurations.append(&mut current_configuration.successors());
            }
            for c in new_configurations.drain(..) {
                self.insert(c);
            }
        }
    }
}

pub struct SimpleAutomataRunner<'a> {
    pub current_configurations: HashSet<SimpleAutomataConfiguration<'a>>,
}

impl<'a> SimpleAutomataRunner<'a> {
    pub fn new(automaton: &'a Automata<'a>, input_sequence: Vec<ReadableView<String>>) -> Self {
        let mut current_configurations = HashSet::new();
        for initial_state in automaton.initial_states.iter() {
            let config = SimpleAutomataConfiguration::new(initial_state, input_sequence.clone());
            current_configurations.insert(config);
        }
        Self {
            current_configurations,
        }
    }
}

impl<'a> AutomataRunner<'a, SimpleAutomataConfiguration<'a>> for SimpleAutomataRunner<'a> {
    fn insert(&mut self, configuration: SimpleAutomataConfiguration<'a>) {
        self.current_configurations.insert(configuration);
    }

    fn len(&self) -> usize {
        self.current_configurations.len()
    }

    fn iter(&mut self) -> Iter<SimpleAutomataConfiguration<'a>> {
        self.current_configurations.iter()
    }

    fn insert_from_initial_states(
        &mut self,
        automaton: &'a Automata<'a>,
        input_sequence: Vec<ReadableView<String>>,
    ) {
        for initial_state in automaton.initial_states.iter() {
            let config = SimpleAutomataConfiguration::new(initial_state, input_sequence.clone());
            self.current_configurations.insert(config);
        }
    }
}

trait AutomataConfiguration<'a> {
    fn dimensions(&self) -> usize;
    fn transitions(&self) -> Ref<Vec<&Transition<'a>>>;
    fn duplicate(&self, current_state: &'a State<'a>) -> Self;
    fn input_head(&self, i: usize) -> Option<String>;
    fn input_advance(&mut self, i: usize, count: usize);

    /// Computes all possible successor configurations from the current one
    /// by applying each outgoing transition of the current state.
    ///
    /// Returns a list of all valid successor configurations.
    fn successors(&self) -> Vec<Self>
    where
        Self: Sized,
    {
        let mut successors = Vec::new();
        for transition in self.transitions().iter() {
            assert_eq!(
                self.dimensions(),
                transition.action.len(),
                "Action length mismatch"
            );
            // Make the tentative successor
            let mut successor = self.duplicate(transition.next_state);
            // Check if the transition is available
            let mut is_valid = true;
            for i in 0..transition.action.len() {
                let head = self.input_head(i);
                if transition.action[i] != ""
                    && (head.is_none() || transition.action[i] != head.unwrap())
                {
                    is_valid = false;
                    break;
                } else if transition.action[i] != "" {
                    // Consume the input sequence
                    successor.input_advance(i, 1);
                }
            }

            if is_valid {
                successors.push(successor);
            }
        }
        successors
    }
}

/// Represents the current configuration of an automaton
#[derive(Hash, Eq, PartialEq, Debug)]
pub struct SimpleAutomataConfiguration<'a> {
    /// The current state of the automaton.
    pub current_state: &'a State<'a>,

    /// A vector of readable views over the input(s) that the automaton consumes.
    /// Each `ReadableView<String>` tracks how far the automaton has read.
    pub input_sequence: Vec<ReadableView<String>>,
}

impl<'a> SimpleAutomataConfiguration<'a> {
    /// Creates a new `AutomataConfiguration` from the automaton, current state,
    /// and a list of readable views of the input(s).
    pub fn new(current_state: &'a State<'a>, input_sequence: Vec<ReadableView<String>>) -> Self {
        Self {
            current_state,
            input_sequence,
        }
    }
}

impl<'a> AutomataConfiguration<'a> for SimpleAutomataConfiguration<'a> {
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

/// An append-only sequence container that allows multiple readers to view
/// appended elements without mutating them.
pub struct AppendOnlySequence<T> {
    data: Rc<RefCell<Vec<T>>>,
}

impl<T> AppendOnlySequence<T> {
    /// Creates a new, empty `AppendOnlySequence`.
    pub fn new() -> Self {
        Self {
            data: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Appends a value to the end of the sequence. Existing readers keep
    /// their views
    pub fn append(&mut self, value: T) {
        self.data.borrow_mut().push(value);
    }

    /// Clears all elements in the sequence.
    pub fn clear(&mut self) {
        self.data.borrow_mut().clear();
    }

    /// Creates a readable view starting from the beginning of the sequence.
    pub fn readable_view(&self) -> ReadableView<T> {
        ReadableView::new(Rc::clone(&self.data))
    }
}

/// A "read-only" view into part of an `AppendOnlySequence`.
#[derive(Debug)]
pub struct ReadableView<T> {
    /// Shared ownership of the sequence
    data: Rc<RefCell<Vec<T>>>,
    /// Start index of the readable range
    start: usize,
}

impl<T> ReadableView<T> {
    /// Creates a new `ReadableView` starting at index `0`.
    pub fn new(data: Rc<RefCell<Vec<T>>>) -> Self {
        Self { data, start: 0 }
    }

    /// Advances the readable view forward by `count` positions. If `count`
    /// would go beyond the end of the data, it clamps to the end.
    pub fn advance_readable(&mut self, count: usize) {
        let len = self.data.borrow().len();
        self.start = usize::min(self.start + count, len);
    }

    /// Returns a borrow of the underlying slice that starts at the current
    /// `start` index and goes to the end of the data.
    pub fn readable_slice(&self) -> Ref<'_, [T]> {
        Ref::map(self.data.borrow(), |vec| &vec[self.start..])
    }

    pub fn len(&self) -> usize {
        self.data.borrow().len() - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Clone for ReadableView<T> {
    fn clone(&self) -> Self {
        ReadableView {
            data: Rc::clone(&self.data),
            start: self.start,
        }
    }
}

impl<T: Hash> Hash for ReadableView<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Implement a simple hash function for the Automata
        self.data.as_ptr().hash(state);
        self.start.hash(state);
    }
}

impl<T: Eq> PartialEq for ReadableView<T> {
    fn eq(&self, other: &Self) -> bool {
        // We utilize the comparison based on the memory address of the state
        self.data.as_ptr() as *const _ == other.data.as_ptr() as *const _
            && self.start == other.start
    }
}

impl<T: Eq> Eq for ReadableView<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use typed_arena::Arena;

    #[test]
    fn test_append_only_sequence() {
        let mut seq = AppendOnlySequence::new();
        seq.append(1);
        seq.append(2);
        seq.append(3);

        let view1 = seq.readable_view();
        assert_eq!(&*view1.readable_slice(), &[1, 2, 3]);

        seq.append(4);
        let view2 = seq.readable_view();
        assert_eq!(&*view2.readable_slice(), &[1, 2, 3, 4]);

        seq.clear();
        let view3 = seq.readable_view();
        assert_eq!(&*view3.readable_slice(), &[]);
    }

    #[test]
    fn test_readable_view_advance() {
        let mut seq = AppendOnlySequence::new();
        seq.append("a");
        seq.append("b");
        seq.append("c");
        let mut view = seq.readable_view();
        assert_eq!(&*view.readable_slice(), &["a", "b", "c"]);

        view.advance_readable(1);
        assert_eq!(&*view.readable_slice(), &["b", "c"]);

        view.advance_readable(10);
        assert_eq!(&*view.readable_slice(), Vec::<String>::new());
    }

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

        let config = SimpleAutomataConfiguration::new(
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

        let mut runner = SimpleAutomataRunner::new(
            &automata,
            sequences.iter().map(|s| s.readable_view()).collect(),
        );
        runner.consume();

        let successors = runner.current_configurations;

        assert_eq!(successors.len(), 6);

        // No transition
        assert!(successors.contains(&SimpleAutomataConfiguration::new(
            s1,
            sequences.iter().map(|s| s.readable_view()).collect()
        )));

        // Self loops
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s1, view)));
        }
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[1].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s1, view)));
        }
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            view[1].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s1, view)));
        }

        // Directly moves to s2
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            view[1].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s2, view)));
        }

        // Moves to s3 after consuming the first elements with the self loops
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(2);
            view[1].advance_readable(2);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s3, view)));
        }
    }
}
