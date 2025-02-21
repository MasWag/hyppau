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

impl<'a, L: Hash> Hash for Transition<'a, L> {
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

impl<'a, L> PartialEq for State<'a, L> {
    fn eq(&self, other: &Self) -> bool {
        // States compared by their pointer address.
        std::ptr::eq(self, other)
    }
}

impl<'a, L> Eq for State<'a, L> {}

impl<'a, L> Debug for State<'a, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State({:p}, is_final: {})", self, self.is_final)
    }
}

impl<'a, L> Hash for State<'a, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Simple hash based on the pointer.
        state.write_usize(self as *const _ as usize);
    }
}

pub type NFAHState<'a> = State<'a, (String, usize)>;

/// Represents an NFA over Σ x Vars.
pub type NFAH<'a> = Automata<'a, (String, usize)>;
/// Represents an NFA over Σ.
pub type NFA<'a> = Automata<'a, String>;

pub trait TransitionCost: Debug + Clone + Eq + Hash {
    /// Computes the cost for a transition.
    ///
    /// For an automaton over Σ, you might simply return 1
    /// For an automaton over Σ×Vars, you might return 1 only when a certain condition holds (e.g. a given variable).
    fn cost(&self, variable: Option<usize>) -> usize;
}

impl TransitionCost for String {
    fn cost(&self, _variable: Option<usize>) -> usize {
        // Every letter contributes 1.
        1
    }
}

impl TransitionCost for (String, usize) {
    fn cost(&self, variable: Option<usize>) -> usize {
        let (ref _action, var) = *self;
        // Only count a cost when `var` matches the variable.
        if let Some(filter_var) = variable {
            if var == filter_var { 1 } else { 0 }
        } else {
            panic!("Variable index required for transition cost");
        }
    }
}


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

impl<'a, L: Eq + Hash + Clone + TransitionCost + ValidLabel> Automata<'a, L> {
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
    pub fn shortest_accepted_word_length(&self, var: usize) -> Option<usize> {
        // (state, current_length) is the BFS node;
        // length increments whenever action != "".
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
                    let new_length = length + transition.label.cost(Some(var));

                    visited.insert((next_state_ptr, new_length));
                    queue.push_back((next_state, new_length));
                }
            }
        }

        shortest_length
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

impl<'a, L> PartialEq for Automata<'a, L> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl<'a, L> Eq for Automata<'a, L> {}

impl<'a, L> Hash for Automata<'a, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Just hash based on the pointer to self’s initial_states for simplicity
        self.initial_states.hash(state);
    }
}
impl<'a, L> Debug for Automata<'a, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NFAH({:p})", self)
    }
}

impl<'a> NFAH<'a> {
    /// Adds a transition (action, var) from `from` to `to`.
    pub fn add_nfah_transition(
        &self,
        from: &'a NFAHState<'a>,
        action: String,
        var: usize,
        to: &'a NFAHState<'a>,
    ) -> &'a NFAHTransition<'a> {
        self.add_transition(from, (action, var), to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_shortest_accepted_word_length() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 2);

        let s1 = automaton.add_state(true, false);
        let s12 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_nfah_transition(s1, "a".to_string(), 0, s12);
        automaton.add_nfah_transition(s12, "b".to_string(), 1, s2);
        automaton.add_nfah_transition(s1, "a".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "b".to_string(), 1, s1);
        automaton.add_nfah_transition(s1, "d".to_string(), 1, s3);

        assert_eq!(automaton.shortest_accepted_word_length(0), Some(0));
        assert_eq!(automaton.shortest_accepted_word_length(1), Some(1));
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
}
