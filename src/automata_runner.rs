use crate::automata::{NFAHState, NFAHTransition, ValidLabel, NFAH};
use std::cell::{Ref, RefCell};
use std::collections::hash_set::Iter;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// A trait defining the behavior of an automaton runner, which tracks and expands
/// sets of configurations over time.
///
/// This trait is generic over:
/// - `'a`: the lifetime of the automaton and its states/transitions.
/// - `C`: the type of `NFAHConfiguration` that represents a single state of
///   the automaton and the positions in the input(s).
pub trait NFAHRunner<'a, C: NFAHConfiguration<'a> + std::cmp::Eq + std::hash::Hash> {
    /// Inserts a single new configuration into the runner's internal set.
    ///
    /// # Arguments
    ///
    /// * `configuration` - The new configuration to add.
    fn insert(&mut self, configuration: C);

    /// Returns the current number of unique configurations tracked by the runner.
    fn len(&self) -> usize;

    /// Returns an iterator over the current configurations.
    ///
    /// Note: This returns a concrete `Iter<C>` in the trait. If you need more
    /// flexibility in the future (like returning different iterator types), you
    /// could return `Box<dyn Iterator<Item = &C> + '_>` instead.
    fn iter(&mut self) -> Iter<C>;

    /// Given an automaton and an initial input sequence, inserts configurations
    /// corresponding to each initial state of the automaton.
    ///
    /// # Arguments
    ///
    /// * `automaton` - A reference to the automaton.
    /// * `input_sequence` - A vector of `ReadableView<String>` representing the
    ///   inputs to the automaton.
    fn insert_from_initial_states(&mut self, input_sequence: Vec<ReadableView<String>>);

    /// Consumes the input sequence and move to the successors.
    fn consume(&mut self) {
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
    }
}

/// A simple implementation of an `NFAHRunner` that stores its configurations
/// in a `HashSet`.
///
/// # Type Parameters
/// * `'a` - The lifetime of the associated `Automata`, `State`, and `Transition`.
///
/// This runner supports inserting configurations, iterating over them, and
/// performing saturation expansions with `consume`.
pub struct SimpleAutomataRunner<'a> {
    automaton: &'a NFAH<'a>,
    /// The current set of configurations of type `SimpleAutomataConfiguration`.
    /// Each configuration is unique in the set (thanks to `Hash`/`Eq`).
    pub current_configurations: HashSet<SimpleAutomataConfiguration<'a>>,
}

impl<'a> SimpleAutomataRunner<'a> {
    /// Constructs a new `SimpleAutomataRunner` by inserting configurations for
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
    /// A new `SimpleAutomataRunner` with initial configurations set up.
    pub fn new(automaton: &'a NFAH<'a>, input_sequence: Vec<ReadableView<String>>) -> Self {
        let mut current_configurations = HashSet::new();
        for initial_state in automaton.initial_states.iter() {
            let config = SimpleAutomataConfiguration::new(initial_state, input_sequence.clone());
            current_configurations.insert(config);
        }
        Self {
            automaton,
            current_configurations,
        }
    }
}

impl<'a> NFAHRunner<'a, SimpleAutomataConfiguration<'a>> for SimpleAutomataRunner<'a> {
    /// Inserts a new configuration into the `HashSet`. Duplicate configurations
    /// (i.e., those that are `Eq`) will be automatically skipped.
    fn insert(&mut self, configuration: SimpleAutomataConfiguration<'a>) {
        self.current_configurations.insert(configuration);
    }

    /// Returns the number of unique configurations in the `HashSet`.
    fn len(&self) -> usize {
        self.current_configurations.len()
    }

    /// Returns an iterator over the current configurations in the `HashSet`.
    fn iter(&mut self) -> Iter<SimpleAutomataConfiguration<'a>> {
        self.current_configurations.iter()
    }

    /// Inserts new configurations for each initial state of the given automaton,
    /// using the provided `input_sequence`.
    fn insert_from_initial_states(&mut self, input_sequence: Vec<ReadableView<String>>) {
        for initial_state in self.automaton.initial_states.iter() {
            let config = SimpleAutomataConfiguration::new(initial_state, input_sequence.clone());
            self.current_configurations.insert(config);
        }
    }
}

/// A trait defining what it means to be a single "configuration" of an automaton.
/// This encapsulates:
/// - The current automaton state,
/// - The input slices/views (one per dimension if multi-dimensional).
///
/// # Lifetime Parameters
/// * `'a`: lifetime that ties this configuration to the automaton’s states and transitions.
pub trait NFAHConfiguration<'a> {
    /// Returns the number of variables in the automaton.
    fn dimensions(&self) -> usize;

    /// Returns a shared reference to the list of outgoing transitions from
    /// the current state.
    fn transitions(&self) -> Ref<Vec<&NFAHTransition<'a>>>;

    /// Creates a new configuration that is identical to `self` except its
    /// current state is replaced with `current_state`. Typically used before
    /// checking or applying transitions.
    fn duplicate(&self, current_state: &'a NFAHState<'a>) -> Self;

    /// Returns the current "head" element of the `i`-th input sequence, if it exists.
    /// If the sequence is empty at that index, returns `None`.
    ///
    /// # Arguments
    ///
    /// * `i` - The index of the input sequence to examine.
    fn input_head(&self, i: usize) -> Option<String>;

    /// Advances the `i`-th input sequence by `count` elements. If the sequence
    /// is shorter than `count`, is clamps the new start index to the sequence length.
    ///
    /// # Arguments
    ///
    /// * `i` - The index of the input sequence to advance.
    /// * `count` - How many elements to consume.
    fn input_advance(&mut self, i: usize, count: usize);

    /// Computes all possible successor configurations from the current one
    /// by applying each outgoing transition of the current state.
    ///
    /// Returns a list of all valid successor configurations. A successor is
    /// considered valid if for every dimension of the transition’s action:
    /// - If the transition’s action is non-empty, it must match the head of
    ///   the corresponding input sequence,
    /// - Then that matching symbol is consumed (the input is advanced).
    fn successors(&self) -> Vec<Self>
    where
        Self: Sized,
        Self: Eq + Hash,
    {
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

/// Represents the current configuration of an automaton, including:
/// - The current state of the automaton,
/// - A vector of input sequences (as `ReadableView<String>`), indicating how
///   far each dimension of input has been read.
#[derive(Hash, Eq, PartialEq, Debug)]
pub struct SimpleAutomataConfiguration<'a> {
    /// The current state of the automaton.
    pub current_state: &'a NFAHState<'a>,

    /// A vector of readable views over the input(s) that the automaton consumes.
    /// Each `ReadableView<String>` tracks how far the automaton has read.
    /// For example, if this vector has length 2, we are dealing with a 2D input.
    pub input_sequence: Vec<ReadableView<String>>,
}

impl<'a> SimpleAutomataConfiguration<'a> {
    /// Creates a new `SimpleAutomataConfiguration` from the given state and
    /// list of `ReadableView`s for each input dimension.
    pub fn new(
        current_state: &'a NFAHState<'a>,
        input_sequence: Vec<ReadableView<String>>,
    ) -> Self {
        Self {
            current_state,
            input_sequence,
        }
    }
}

impl<'a> NFAHConfiguration<'a> for SimpleAutomataConfiguration<'a> {
    fn dimensions(&self) -> usize {
        self.input_sequence.len()
    }

    fn transitions(&self) -> Ref<Vec<&NFAHTransition<'a>>> {
        self.current_state.transitions.borrow()
    }

    fn duplicate(&self, current_state: &'a NFAHState<'a>) -> Self {
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
/// appended elements without mutating them. Once appended, elements remain at
/// their positions, so existing `ReadableView`s stay valid.
///
/// # Type Parameters
/// * `T` - The type of elements stored in the sequence.
pub struct AppendOnlySequence<T> {
    /// Internal shared storage of all elements in the sequence.
    data: Rc<RefCell<Vec<T>>>,
}

impl<T> AppendOnlySequence<T> {
    /// Creates a new, empty `AppendOnlySequence`.
    ///
    /// # Returns
    ///
    /// An empty sequence capable of storing `T`.
    pub fn new() -> Self {
        Self {
            data: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Appends a value to the end of the sequence. Existing readers keep
    /// their same start index but the underlying slice grows.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to add at the end of this sequence.
    pub fn append(&mut self, value: T) {
        self.data.borrow_mut().push(value);
    }

    /// Clears all elements in the sequence, removing them permanently.
    /// Any existing `ReadableView`s will now see an empty slice.
    pub fn clear(&mut self) {
        self.data.borrow_mut().clear();
    }

    /// Creates a readable view starting from the beginning of the sequence.
    ///
    /// # Returns
    ///
    /// A `ReadableView` that starts at index `0` and can be advanced but not
    /// rewound.
    pub fn readable_view(&self) -> ReadableView<T> {
        ReadableView::new(Rc::clone(&self.data))
    }
}

/// A "read-only" view into part of an `AppendOnlySequence`. It keeps track of
/// where in the sequence it is currently "reading" (via `start`).
///
/// # Type Parameters
/// * `T` - The type of the elements in the underlying sequence.
#[derive(Debug)]
pub struct ReadableView<T> {
    /// Shared ownership of the sequence data.
    data: Rc<RefCell<Vec<T>>>,
    /// The current starting index for reading.
    pub start: usize,
}

impl<T> ReadableView<T> {
    /// Creates a new `ReadableView` starting at index `0`.
    ///
    /// # Arguments
    ///
    /// * `data` - A reference-counted pointer to the shared vector of `T`.
    ///
    /// # Returns
    ///
    /// A readable view that will initially see the entire sequence.
    pub fn new(data: Rc<RefCell<Vec<T>>>) -> Self {
        Self { data, start: 0 }
    }

    /// Advances the readable view forward by `count` positions. If `count`
    /// would go beyond the end of the data, it clamps to the end of the
    /// sequence.
    ///
    /// # Arguments
    ///
    /// * `count` - The number of elements to consume from the front of this view.
    pub fn advance_readable(&mut self, count: usize) {
        let len = self.data.borrow().len();
        self.start = usize::min(self.start + count, len);
    }

    /// Returns a borrow of the underlying slice that starts at the current
    /// `start` index and goes to the end of the data.
    ///
    /// # Returns
    ///
    /// An immutable slice of type `[T]`.
    pub fn readable_slice(&self) -> Ref<'_, [T]> {
        Ref::map(self.data.borrow(), |vec| &vec[self.start..])
    }

    /// Returns the current length of the readable slice, i.e., how many items
    /// remain from `start` to the end.
    pub fn len(&self) -> usize {
        self.data.borrow().len() - self.start
    }

    /// Returns `true` if there are no more items left to read.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if this view shares the same underlying data as another
    /// ReadableView
    pub fn same_data(&self, other: &ReadableView<T>) -> bool {
        std::ptr::eq(self.data.as_ptr(), other.data.as_ptr())
    }
}

impl<T> Clone for ReadableView<T> {
    /// Cloning a `ReadableView` shares the same underlying data and the same
    /// `start` index. Both views will move independently if advanced later.
    fn clone(&self) -> Self {
        ReadableView {
            data: Rc::clone(&self.data),
            start: self.start,
        }
    }
}

impl<T: Hash> Hash for ReadableView<T> {
    /// We hash by pointer address of `data` and the `start` index. This means
    /// two `ReadableView`s of the same slice (by pointer) and same start
    /// index will have the same hash, but distinct sequence objects or indices
    /// will differ.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.as_ptr().hash(state);
        self.start.hash(state);
    }
}

impl<T: Eq> PartialEq for ReadableView<T> {
    /// Two `ReadableView`s are considered equal if they point to the same
    /// underlying sequence (same `Rc`) and have the same `start` index.
    /// They do not compare the actual *contents* in the sequence.
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.data.as_ptr(), other.data.as_ptr()) && self.start == other.start
    }
}

impl<T: Eq> Eq for ReadableView<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::NFAH;
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
        assert_eq!(&*view3.readable_slice(), &[] as &[i32]);
    }

    #[test]
    fn test_readable_view_advance() {
        let mut seq = AppendOnlySequence::new();
        seq.append("a".to_string());
        seq.append("b".to_string());
        seq.append("c".to_string());
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

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].append("c".to_string());
        sequences[1].append("d".to_string());

        let config = SimpleAutomataConfiguration::new(
            s1,
            sequences.iter().map(|s| s.readable_view()).collect(),
        );

        let successors = config.successors();

        assert_eq!(successors.len(), 3);

        // Moves to s12 using (a, 0)
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s12, view)));
        }
        // Moves to s1 using (a, 0)
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s1, view)));
        }
        // Moves to s1 using (b, 1)
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[1].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s1, view)));
        }
    }

    #[test]
    fn test_automata_runner() {
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

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].append("c".to_string());
        sequences[1].append("d".to_string());

        let mut runner = SimpleAutomataRunner::new(
            &automaton,
            sequences.iter().map(|s| s.readable_view()).collect(),
        );
        runner.consume();

        let successors = runner.current_configurations;

        assert_eq!(successors.len(), 10);

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

        // Moves to s12
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s12, view)));
        }

        // Moves to s12 after consuming the first element of the second dimension with a self-loop
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            view[1].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s12, view)));
        }

        // Moves to s2 via s12
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(1);
            view[1].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s2, view)));
        }

        // Moves to s13 after consuming the first element of the first dimension with the self loops
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(2);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s13, view)));
        }

        // Moves to s13 after consuming the first elements with the self loops
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(2);
            view[1].advance_readable(1);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s13, view)));
        }

        // Moves to s3 via s13 after consuming the first elements with the self loops
        {
            let mut view: Vec<ReadableView<String>> =
                sequences.iter().map(|s| s.readable_view()).collect();
            view[0].advance_readable(2);
            view[1].advance_readable(2);
            assert!(successors.contains(&SimpleAutomataConfiguration::new(s3, view)));
        }
    }
}
