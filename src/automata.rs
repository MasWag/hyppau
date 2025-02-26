use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use typed_arena::Arena;

/// Represents a transition for an NFA.
#[derive(Debug, PartialEq, Clone)]
pub struct Transition<'a, L> {
    pub label: L,
    pub next_state: &'a State<'a, L>,
}

impl<L: Hash> Hash for Transition<'_, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.label.hash(state);
        self.next_state.hash(state);
    }
}

/// Represents a transition for an NFA over Σ x Vars.
///
/// Each transition is labeled by a pair (letter, var) where 'letter'
/// is from the alphabet Σ and 'var' identifies the variable.
pub type NFAHTransition<'a> = Transition<'a, (String, usize)>;

/// Represents a state in an NFA.
///
/// Stores whether it is final (accepting) and its outgoing transitions.
pub struct State<'a, L> {
    /// Outgoing transitions.
    pub transitions: RefCell<Vec<&'a Transition<'a, L>>>,
    /// Whether this state is an accepting state.
    pub is_final: bool,
}

impl<L> PartialEq for State<'_, L> {
    fn eq(&self, other: &Self) -> bool {
        // States compared by their pointer address.
        std::ptr::eq(self, other)
    }
}

impl<L> Eq for State<'_, L> {}

impl<L> Debug for State<'_, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State({:p}, is_final: {})", self, self.is_final)
    }
}

impl<L> Hash for State<'_, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Simple hash based on the pointer.
        state.write_usize(self as *const _ as usize);
    }
}

pub type NFAHState<'a> = State<'a, (String, usize)>;
pub type NFAState<'a> = State<'a, String>;

/// Represents an NFA over Σ x Vars.
pub type NFAH<'a> = Automata<'a, (String, usize)>;
/// Represents an NFA over Σ.
pub type NFA<'a> = Automata<'a, String>;
/// An epsilon-NFA (where transitions are labeled by `Some(symbol)` or `None` for ε).
pub type EpsilonNFA<'a> = Automata<'a, Option<String>>;

pub trait ValidLabel {
    /// Checks that the label is valid given an optional dimension.
    /// For automata over Σ×Vars, the dimension is required.
    /// For automata over Σ, the dimension is ignored.
    fn validate(&self, dimensions: usize);
}

impl ValidLabel for (String, usize) {
    fn validate(&self, dimensions: usize) {
        let (_, var) = self;
        if *var >= dimensions {
            panic!("Variable index out of bounds");
        }
    }
}

impl ValidLabel for String {
    fn validate(&self, _dimensions: usize) {
        // No validity check is necessary for simple letter labels.
    }
}

impl ValidLabel for Option<String> {
    fn validate(&self, _dimensions: usize) {
        // No validity check is necessary for simple letter labels.
    }
}

impl ValidLabel for char {
    fn validate(&self, _dimensions: usize) {
        // No validity check is necessary for simple letter labels.
    }
}

pub struct Automata<'a, L> {
    /// Arena for `State` allocations.
    pub states: &'a Arena<State<'a, L>>,
    /// Arena for `Transition` allocations.
    pub transitions: &'a Arena<Transition<'a, L>>,
    /// The initial states.
    pub initial_states: Vec<&'a State<'a, L>>,
    /// The number of variables.
    pub dimensions: usize,
}

impl<'a, L: Eq + Hash + Clone + ValidLabel> Automata<'a, L> {
    /// Creates a new automaton.
    pub fn new(
        states: &'a Arena<State<'a, L>>,
        transitions: &'a Arena<Transition<'a, L>>,
        dimension: usize,
    ) -> Self {
        Self {
            states,
            transitions,
            initial_states: Vec::new(),
            dimensions: dimension,
        }
    }

    /// Adds a new state to the automaton.
    ///
    /// # Arguments
    /// * `is_initial` - whether this state is one of the initial states
    /// * `is_final` - whether this state is accepting
    pub fn add_state(&mut self, is_initial: bool, is_final: bool) -> &'a State<'a, L> {
        let state = self.states.alloc(State {
            transitions: RefCell::new(Vec::new()),
            is_final,
        });
        if is_initial {
            self.initial_states.push(state);
        }
        state
    }

    /// Adds a transition (action, var) from `from` to `to`.
    pub fn add_transition(
        &self,
        from: &'a State<'a, L>,
        label: L,
        to: &'a State<'a, L>,
    ) -> &'a Transition<'a, L> {
        label.validate(self.dimensions);
        let transition = self.transitions.alloc(Transition {
            label,
            next_state: to,
        });
        from.transitions.borrow_mut().push(transition);
        transition
    }

    /// Returns the length of the shortest accepted word in the automaton using BFS.
    pub fn shortest_accepted_word_length(&self) -> usize {
        // (state, current_length) is the BFS node;
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        // Start from all initial states
        for &initial_state in &self.initial_states {
            queue.push_back((initial_state, 0));
            visited.insert((initial_state as *const _, 0));
        }

        let mut shortest_length: Option<usize> = None;

        while let Some((current_state, length)) = queue.pop_front() {
            // If current_state is final, update the shortest_length length
            if current_state.is_final {
                // Update the shortest length
                match shortest_length {
                    None => shortest_length = Some(length),
                    Some(prev) if length < prev => shortest_length = Some(length),
                    _ => {}
                }
            }

            // Skip if the current word is longer than the shortest length we've found so far
            if let Some(best) = shortest_length {
                if length >= best {
                    continue;
                }
            }

            // Explore all transitions from current state
            for &transition in current_state.transitions.borrow().iter() {
                let next_state = transition.next_state;
                let next_state_ptr = next_state as *const _;

                if !visited.contains(&(next_state_ptr, length)) {
                    visited.insert((next_state_ptr, length + 1));
                    queue.push_back((next_state, length + 1));
                }
            }
        }

        shortest_length.unwrap()
    }

    /// Returns all prefixes of length `n` that can appear along a path from any initial state.
    ///
    /// We store the entire label `(action, var)` in the prefix.
    pub fn accepted_prefixes(&self, n: usize) -> HashSet<Vec<L>> {
        let mut prefixes = HashSet::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        // Initialize BFS from each initial state with empty prefix
        for &init in &self.initial_states {
            queue.push_back((init, Vec::<L>::new(), 0));
            // we add length=0 for BFS
            visited.insert((init as *const _, vec![]));
        }

        while let Some((current_state, prefix, length)) = queue.pop_front() {
            // If we have a prefix of length `n`, store it
            if length == n {
                prefixes.insert(prefix);
                // do not explore any deeper from here
                continue;
            }

            // Explore outgoing transitions
            for &transition in current_state.transitions.borrow().iter() {
                let new_length = length + 1;
                if new_length <= n {
                    // Build the new prefix
                    let mut new_prefix = prefix.clone();
                    new_prefix.push(transition.label.clone());
                    let next_ptr = transition.next_state as *const _;
                    if !visited.contains(&(next_ptr, new_prefix.clone())) {
                        visited.insert((next_ptr, new_prefix.clone()));
                        queue.push_back((transition.next_state, new_prefix, new_length));
                    }
                }
            }
        }

        prefixes
    }

    /// Removes transitions to states that cannot lead to a final state.
    /// A standard "reverse" reachability: keep only states from which
    /// a final state is reachable, removing transitions that lead to
    /// states outside that set.
    pub fn remove_unreachable_transitions(&self) {
        // 1) Collect all states reachable from an initial state
        let mut reachable = HashSet::new();
        let mut worklist = VecDeque::new();

        for &init in &self.initial_states {
            reachable.insert(init);
            worklist.push_back(init);
        }

        while let Some(current_state) = worklist.pop_front() {
            for &transition in current_state.transitions.borrow().iter() {
                let next = transition.next_state;
                if !reachable.contains(&next) {
                    reachable.insert(next);
                    worklist.push_back(next);
                }
            }
        }

        // 2) Among these, keep only states from which some final state is reachable.
        //    We do a backward search from final states among the "reachable" set.
        let mut can_reach_final = HashSet::new();
        let mut final_queue = VecDeque::new();

        // Start from final states (that are in `reachable`)
        for &current_state in &reachable {
            if current_state.is_final {
                can_reach_final.insert(current_state);
                final_queue.push_back(current_state);
            }
        }

        // Go backwards: if `X` transitions to `Y` and `Y` is known to reach final,
        // then `X` also can reach final.
        loop {
            let before = can_reach_final.len();
            let mut newly_added = Vec::new();
            for &current_state in &reachable {
                if can_reach_final.contains(current_state) {
                    // skip
                    continue;
                }
                // If current_state transitions to some state in can_reach_final, add current_state
                let trans_out = current_state.transitions.borrow();
                if trans_out
                    .iter()
                    .any(|t| can_reach_final.contains(&t.next_state))
                {
                    newly_added.push(current_state);
                }
            }
            for current_state in newly_added {
                can_reach_final.insert(current_state);
                final_queue.push_back(current_state);
            }
            if can_reach_final.len() == before {
                break;
            }
        }

        // 3) Remove transitions that lead to states not in can_reach_final.
        for &current_state in &reachable {
            let mut trans_out = current_state.transitions.borrow_mut();
            trans_out.retain(|t| can_reach_final.contains(&t.next_state));
        }
    }
}

impl<L> Automata<'_, L> {
    /// Returns `true` if this automaton's language is empty
    /// (i.e., if no final state can be reached from any initial state).
    /// Otherwise, returns `false`.
    pub fn is_empty(&self) -> bool {
        // If there are no initial states, it's trivially empty.
        if self.initial_states.is_empty() {
            return true;
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Initialize queue with all initial states
        for &init in &self.initial_states {
            // If an initial state is already final, language is non-empty
            if init.is_final {
                return false;
            }
            queue.push_back(init);
            visited.insert(init as *const _);
        }

        // BFS over the transition graph
        while let Some(state) = queue.pop_front() {
            for &transition in state.transitions.borrow().iter() {
                let next = transition.next_state;
                if next.is_final {
                    // Found a final state reachable => language is non-empty
                    return false;
                }
                let next_ptr = next as *const _;
                if !visited.contains(&next_ptr) {
                    visited.insert(next_ptr);
                    queue.push_back(next);
                }
            }
        }

        // No final state encountered => empty
        true
    }
}

impl<'a, L> Automata<'a, L>
where
    L: Clone,
{
    /// Returns a set of states in `dfa` that are reachable in exactly `steps` transitions
    /// from any of dfa's initial states.
    pub fn states_reachable_in_exactly_n_steps(&self, steps: usize) -> HashSet<&'a State<'a, L>> {
        // We do BFS layer by layer
        let mut current_layer: HashSet<&State<'a, L>> =
            self.initial_states.iter().copied().collect();
        let mut next_layer = HashSet::new();

        for _ in 0..steps {
            next_layer.clear();
            // move from each state in current_layer by any outgoing transition
            for st in &current_layer {
                for &trans in st.transitions.borrow().iter() {
                    next_layer.insert(trans.next_state);
                }
            }
            std::mem::swap(&mut current_layer, &mut next_layer);
        }

        current_layer
    }
}

impl<'a, L> Automata<'a, L> {
    pub fn iter_states(&'a self) -> AutomataStateIter<'a, L> {
        AutomataStateIter::new(self)
    }
}

pub struct AutomataStateIter<'a, L> {
    seen: HashSet<*const State<'a, L>>,
    queue: VecDeque<&'a State<'a, L>>,
}

impl<'a, L> AutomataStateIter<'a, L> {
    pub fn new(automata: &'a Automata<'a, L>) -> Self {
        Self {
            seen: HashSet::new(),
            queue: automata.initial_states.iter().copied().collect(),
        }
    }
}

impl<'a, L: Clone> Iterator for AutomataStateIter<'a, L> {
    type Item = &'a State<'a, L>;
    fn next(&mut self) -> Option<Self::Item> {
        let next_state = self.queue.pop_front();
        if let Some(next) = next_state {
            self.seen.insert(next as *const _);
            next.transitions.borrow().iter().for_each(|transition| {
                if !self.seen.contains(&((transition.next_state) as *const _)) {
                    self.queue.push_back(transition.next_state)
                }
            });
        }

        next_state
    }
}

impl<L> PartialEq for Automata<'_, L> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl<L> Eq for Automata<'_, L> {}

impl<L> Hash for Automata<'_, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Just hash based on the pointer to self’s initial_states for simplicity
        self.initial_states.hash(state);
    }
}
impl<L> Debug for Automata<'_, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NFAH({:p})", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::NFAH;
    use typed_arena::Arena;

    #[test]
    fn test_add_state() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 1);

        let current_state = automaton.add_state(true, false);
        assert_eq!(automaton.initial_states.len(), 1);
        assert!(automaton.initial_states.contains(&current_state));
        assert!(!current_state.is_final);
        assert_eq!(current_state.transitions.borrow().len(), 0);
    }

    #[test]
    fn test_add_transition() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 1);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, true);

        let t = automaton.add_nfah_transition(s1, "a".to_string(), 0, s2);
        assert_eq!(t.label.0, "a");
        assert_eq!(t.label.1, 0);
        assert_eq!(t.next_state, s2);

        let all_outgoing = s1.transitions.borrow();
        assert_eq!(all_outgoing.len(), 1);
        assert_eq!(all_outgoing[0].label.0, "a");
        assert_eq!(all_outgoing[0].label.1, 0);
        assert_eq!(all_outgoing[0].next_state, s2);
    }

    #[test]
    fn test_iter() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 2);

        let s0 = automaton.add_state(true, false);
        let s1 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, false);
        let sf = automaton.add_state(false, true);

        automaton.add_nfah_transition(s0, "c".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "c".to_string(), 1, s2);
        automaton.add_nfah_transition(s2, "a".to_string(), 0, s3);
        automaton.add_nfah_transition(s3, "b".to_string(), 1, s2);
        automaton.add_nfah_transition(s2, "c".to_string(), 0, sf);

        let mut iter = automaton.iter_states();
        assert_eq!(iter.next(), Some(s0));
        assert_eq!(iter.next(), Some(s1));
        assert_eq!(iter.next(), Some(s2));
        assert_eq!(iter.next(), Some(s3));
        assert_eq!(iter.next(), Some(sf));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_shortest_accepted_word_length() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 2);

        let s0 = automaton.add_state(true, false);
        let s1 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, false);
        let sf = automaton.add_state(false, true);

        automaton.add_nfah_transition(s0, "c".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "c".to_string(), 1, s2);
        automaton.add_nfah_transition(s2, "a".to_string(), 0, s3);
        automaton.add_nfah_transition(s3, "b".to_string(), 1, s2);
        automaton.add_nfah_transition(s2, "c".to_string(), 0, sf);

        assert_eq!(automaton.shortest_accepted_word_length(), 3);
        let prefixes = automaton.accepted_prefixes(3);
        assert_eq!(prefixes.len(), 2);
    }

    #[test]
    fn test_accepted_prefixes() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 2);

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

        // accepted prefixes of length 0 => the empty prefix
        let p0 = automaton.accepted_prefixes(0);
        assert_eq!(p0.len(), 1);
        assert!(p0.contains(&Vec::new()));

        // accepted prefixes of length 1 => either [("a", x0)] or [("b", x1)]
        let p1 = automaton.accepted_prefixes(1);
        println!("{:?}", p1);
        assert_eq!(p1.len(), 3);
        assert!(p1.contains(&vec![("a".to_string(), 0)]));
        assert!(p1.contains(&vec![("b".to_string(), 1)]));
        assert!(p1.contains(&vec![("c".to_string(), 0)]));
    }

    #[test]
    fn test_remove_unreachable_transitions() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 3);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);
        let s4 = automaton.add_state(false, false);

        // s1 --( "a", x0 )--> s2
        automaton.add_nfah_transition(s1, "a".to_string(), 0, s2);
        // s2 --( "", x1 )--> s3
        automaton.add_nfah_transition(s2, "".to_string(), 1, s3);
        // s1 --( "x", x2 )--> s4  (but s4 does not lead to any final state)

        automaton.add_nfah_transition(s1, "x".to_string(), 2, s4);
        // Right now, s1->s4 is reachable from an initial state,
        // but s4 is not leading to any final => s4 is not "useful."
        // So that transition should be removed.

        automaton.remove_unreachable_transitions();

        // s1 should have only 1 transition left
        {
            let s1_trans = s1.transitions.borrow();
            assert_eq!(s1_trans.len(), 1);
            assert_eq!(s1_trans[0].label.0, "a");
            assert_eq!(s1_trans[0].label.1, 0);
        }
        // s4 transitions are empty, but that doesn't matter since
        // s4 won't even be recognized as "reachable to final"
        assert_eq!(s4.transitions.borrow().len(), 0);
    }

    #[test]
    fn test_emptiness_check() {
        use typed_arena::Arena;

        // =========== CASE 1: Non-empty automaton ===========
        // We'll have s0 (initial) --"a"--> s1 (final).
        let arena_s1 = Arena::new();
        let arena_t1 = Arena::new();
        let mut nfa1 = NFA::new(&arena_s1, &arena_t1, 0);

        let s0_1 = nfa1.add_state(true, false);
        let s1_1 = nfa1.add_state(false, true); // final
        nfa1.add_transition(s0_1, "a".to_string(), s1_1);

        assert!(!nfa1.is_empty(), "There is a path to a final state.");

        // =========== CASE 2: No final states at all ===========
        let arena_s2 = Arena::new();
        let arena_t2 = Arena::new();
        let mut nfa2 = NFA::new(&arena_s2, &arena_t2, 0);

        let s0_2 = nfa2.add_state(true, false);
        let s1_2 = nfa2.add_state(false, false);
        nfa2.add_transition(s0_2, "b".to_string(), s1_2);
        // No state is final here
        assert!(nfa2.is_empty(), "No final state => empty language.");

        // =========== CASE 3: Final but unreachable ===========
        let arena_s3 = Arena::new();
        let arena_t3 = Arena::new();
        let mut nfa3 = NFA::new(&arena_s3, &arena_t3, 0);

        let s0_3 = nfa3.add_state(true, false);
        let _s1_3 = nfa3.add_state(false, false);
        let s2_3 = nfa3.add_state(false, true); // final but not connected

        // s0_3 has no outgoing transitions at all. So s2_3 is unreachable.
        assert!(
            nfa3.is_empty(),
            "Final state is unreachable => empty language."
        );
    }
}
