use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use typed_arena::Arena;

/// Represents a transition for an NFA over Σ x Vars.
///
/// Each transition is labeled by a pair (letter, var) where 'letter'
/// is from the alphabet Σ and 'var' identifies the variable.
#[derive(Debug, PartialEq)]
pub struct Transition<'a> {
    /// The action in the alphabet part of the label.
    pub action: String,
    /// The variable part of the label.
    pub var: usize,
    /// The state to transition to.
    pub next_state: &'a State<'a>,
}

impl<'a> Hash for Transition<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.action.hash(state);
        self.var.hash(state);
        self.next_state.hash(state);
    }
}

/// Represents a state in an NFA.
///
/// Stores whether it is final (accepting) and its outgoing transitions.
pub struct State<'a> {
    /// Outgoing transitions.
    pub transitions: RefCell<Vec<&'a Transition<'a>>>,
    /// Whether this state is an accepting state.
    pub is_final: bool,
}

impl<'a> PartialEq for State<'a> {
    fn eq(&self, other: &Self) -> bool {
        // States compared by their pointer address.
        std::ptr::eq(self, other)
    }
}

impl<'a> Eq for State<'a> {}

impl<'a> Debug for State<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State({:p}, is_final: {})", self, self.is_final)
    }
}

impl<'a> Hash for State<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Simple hash based on the pointer.
        state.write_usize(self as *const _ as usize);
    }
}

/// Represents an NFA over Σ x Vars.
pub struct Automata<'a> {
    /// Arena for `State` allocations.
    pub states: &'a Arena<State<'a>>,
    /// Arena for `Transition` allocations.
    pub transitions: &'a Arena<Transition<'a>>,
    /// The initial states.
    pub initial_states: Vec<&'a State<'a>>,
    /// The number of variables.
    pub dimensions: usize,
}

impl<'a> Automata<'a> {
    /// Creates a new automaton.
    pub fn new(
        states: &'a Arena<State<'a>>,
        transitions: &'a Arena<Transition<'a>>,
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
    pub fn add_state(&mut self, is_initial: bool, is_final: bool) -> &'a State<'a> {
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
        from: &'a State<'a>,
        action: String,
        var: usize,
        to: &'a State<'a>,
    ) -> &'a Transition<'a> {
        if var >= self.dimensions {
            panic!("Variable index out of bounds");
        }
        let transition = self.transitions.alloc(Transition {
            action,
            var,
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
                    let new_length = if transition.var == var {
                        length + 1
                    } else {
                        length
                    };

                    visited.insert((next_state_ptr, new_length));
                    queue.push_back((next_state, new_length));
                }
            }
        }

        shortest_length
    }

    /// Returns all (action, var)-prefixes of length `n` that can appear along a path from any initial state.
    ///
    /// We store the entire label `(action, var)` in the prefix.
    pub fn accepted_prefixes(&self, n: usize) -> HashSet<Vec<(String, usize)>> {
        let mut prefixes = HashSet::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        // Initialize BFS from each initial state with empty prefix
        for &init in &self.initial_states {
            queue.push_back((init, Vec::<(String, usize)>::new(), 0));
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
                    new_prefix.push((transition.action.clone(), transition.var.clone()));
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

impl<'a> PartialEq for Automata<'a> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl<'a> Eq for Automata<'a> {}

impl<'a> Hash for Automata<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Just hash based on the pointer to self’s initial_states for simplicity
        self.initial_states.hash(state);
    }
}
impl<'a> Debug for Automata<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Automata({:p})", self)
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
        let mut automaton = Automata::new(&state_arena, &trans_arena, 1);

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
        let mut automaton = Automata::new(&state_arena, &trans_arena, 1);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, true);

        let t = automaton.add_transition(s1, "a".to_string(), 0, s2);
        assert_eq!(t.action, "a");
        assert_eq!(t.var, 0);
        assert_eq!(t.next_state, s2);

        let all_outgoing = s1.transitions.borrow();
        assert_eq!(all_outgoing.len(), 1);
        assert_eq!(all_outgoing[0].action, "a");
        assert_eq!(all_outgoing[0].var, 0);
        assert_eq!(all_outgoing[0].next_state, s2);
    }

    #[test]
    fn test_shortest_accepted_word_length() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = Automata::new(&state_arena, &trans_arena, 2);

        let s1 = automaton.add_state(true, false);
        let s12 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_transition(s1, "a".to_string(), 0, s12);
        automaton.add_transition(s12, "b".to_string(), 1, s2);
        automaton.add_transition(s1, "a".to_string(), 0, s1);
        automaton.add_transition(s1, "b".to_string(), 1, s1);
        automaton.add_transition(s1, "d".to_string(), 1, s3);

        assert_eq!(automaton.shortest_accepted_word_length(0), Some(0));
        assert_eq!(automaton.shortest_accepted_word_length(1), Some(1));
    }

    #[test]
    fn test_accepted_prefixes() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = Automata::new(&state_arena, &trans_arena, 2);

        let s1 = automaton.add_state(true, false);
        let s12 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s13 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_transition(s1, "a".to_string(), 0, s12);
        automaton.add_transition(s12, "b".to_string(), 1, s2);
        automaton.add_transition(s1, "a".to_string(), 0, s1);
        automaton.add_transition(s1, "b".to_string(), 1, s1);
        automaton.add_transition(s1, "c".to_string(), 0, s13);
        automaton.add_transition(s13, "d".to_string(), 1, s3);

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
        let mut automaton = Automata::new(&state_arena, &trans_arena, 3);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);
        let s4 = automaton.add_state(false, false);

        // s1 --( "a", x0 )--> s2
        automaton.add_transition(s1, "a".to_string(), 0, s2);
        // s2 --( "", x1 )--> s3
        automaton.add_transition(s2, "".to_string(), 1, s3);
        // s1 --( "x", x2 )--> s4  (but s4 does not lead to any final state)

        automaton.add_transition(s1, "x".to_string(), 2, s4);
        // Right now, s1->s4 is reachable from an initial state,
        // but s4 is not leading to any final => s4 is not "useful."
        // So that transition should be removed.

        automaton.remove_unreachable_transitions();

        // s1 should have only 1 transition left
        {
            let s1_trans = s1.transitions.borrow();
            assert_eq!(s1_trans.len(), 1);
            assert_eq!(s1_trans[0].action, "a");
            assert_eq!(s1_trans[0].var, 0);
        }
        // s4 transitions are empty, but that doesn't matter since
        // s4 won't even be recognized as "reachable to final"
        assert_eq!(s4.transitions.borrow().len(), 0);
    }
}
