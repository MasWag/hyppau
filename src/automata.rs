use std::cell::RefCell;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use typed_arena::Arena;

/// Represents a transition between states in an automaton.
///
/// A transition includes an action, which is a k-tuple of strings that triggers the transition and a reference to the next state.
#[derive(Debug, PartialEq)]
pub struct Transition<'a> {
    /// The action that triggers this transition.
    pub action: Vec<String>,
    /// The state to transition to.
    pub next_state: &'a State<'a>,
}

impl<'a> Hash for Transition<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.action.hash(state);
        self.next_state.hash(state);
    }
}

/// Represents a state in an automaton.
///
/// A state can have multiple transitions and may be marked as a final state.
pub struct State<'a> {
    /// The transitions originating from this state.
    pub transitions: RefCell<Vec<&'a Transition<'a>>>,
    /// Indicates whether this state is a final (accepting) state.
    pub is_final: bool,
}

impl<'a> PartialEq for State<'a> {
    fn eq(&self, other: &Self) -> bool {
        // We utilize the comparison based on the memory address of the state
        self as *const _ == other as *const _
    }
}

impl<'a> Eq for State<'a> {}

impl<'a> Debug for State<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "State({:?}, is_final: {})",
            self as *const _, self.is_final
        )
    }
}

impl<'a> Hash for State<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Implement a simple hash function for the State
        state.write_usize(self as *const _ as usize);
    }
}

impl<'a> State<'a> {
    /// Given a vector of actions, consumes it and returns the successors.
    ///
    /// # Arguments
    ///
    /// * `action` - The vector of actions to consume. Notice that each action is a vector of strings because we are working on k-track automata.
    ///
    /// # Returns
    ///
    /// A vector of pairs, where each pair consists of the target state and the remaining actions.
    pub fn consume(&self, action: Vec<String>) -> Vec<(&State<'a>, Vec<String>)> {
        let mut successors = Vec::new();
        for transition in self.transitions.borrow().iter() {
            // Make a copy of action
            let mut remaining_action = Vec::with_capacity(action.len());
            assert_eq!(
                remaining_action.len(),
                transition.action.len(),
                "Action length mismatch"
            );
            // Check if for element in transition.action, there is a corresponding element in action or transition.action is epsilon
            let mut is_valid = true;
            for i in 0..transition.action.len() {
                if transition.action[i] != "" && transition.action[i] != remaining_action[i] {
                    is_valid = false;
                    break;
                } else {
                    remaining_action[i] = "".to_string();
                }
            }
            if is_valid {
                successors.push((transition.next_state, remaining_action));
            }
        }
        successors
    }
}

/// Represents a finite automaton.
///
/// An automaton consists of a set of states, transitions, and initial states.
pub struct Automata<'a> {
    /// The arena for allocating states.
    pub states: &'a Arena<State<'a>>,
    /// The arena for allocating transitions.
    pub transitions: &'a Arena<Transition<'a>>,
    /// The initial states of the automaton.
    pub initial_states: Vec<&'a State<'a>>,
}

impl<'a> Automata<'a> {
    /// Creates a new automaton.
    ///
    /// # Arguments
    ///
    /// * `states` - An arena for managing state allocations.
    /// * `transitions` - An arena for managing transition allocations.
    ///
    /// # Returns
    ///
    /// A new `Automata` instance.
    pub fn new(states: &'a Arena<State<'a>>, transitions: &'a Arena<Transition<'a>>) -> Self {
        Self {
            states,
            transitions,
            initial_states: Vec::new(),
        }
    }

    /// Adds a new state to the automaton.
    ///
    /// # Arguments
    ///
    /// * `is_initial` - Whether the state is an initial state.
    /// * `is_final` - Whether the state is a final (accepting) state.
    ///
    /// # Returns
    ///
    /// A reference to the newly created state.
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

    /// Adds a transition between two states.
    ///
    /// # Arguments
    ///
    /// * `from` - The state from which the transition originates.
    /// * `action` - The action that triggers this transition. We use the empty string for unobservable transitions.
    /// * `to` - The state to which the transition leads.
    ///
    /// # Returns
    ///
    /// A reference to the newly created transition.
    pub fn add_transition(
        &self,
        from: &'a State<'a>,
        action: Vec<String>,
        to: &'a State<'a>,
    ) -> &'a Transition<'a> {
        let transition = self.transitions.alloc(Transition {
            action,
            next_state: to,
        });
        from.transitions.borrow_mut().push(transition);
        transition
    }
}

impl<'a> PartialEq for Automata<'a> {
    fn eq(&self, other: &Self) -> bool {
        // We utilize the comparison based on the memory address of the Automata
        self as *const _ == other as *const _
    }
}

impl<'a> Eq for Automata<'a> {}

impl<'a> Hash for Automata<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Implement a simple hash function for the Automata
        self.initial_states.hash(state);
    }
}

impl<'a> Debug for Automata<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Automata({:?})", self as *const _)
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Import everything from the parent module
    use typed_arena::Arena;

    /// Tests that a state can be added to the automaton.
    #[test]
    fn test_add_state() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = Automata::new(&state_arena, &transition_arena);

        let state = automata.add_state(true, false);
        assert!(!state.is_final); // Verify the state is not final
        assert!(state.transitions.borrow().is_empty()); // Verify the state has no transitions
    }

    /// Tests that a transition can be added to the automaton.
    #[test]
    fn test_add_transition() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = Automata::new(&state_arena, &transition_arena);

        let state1 = automata.add_state(true, false); // Initial, non-final state
        let state2 = automata.add_state(false, true); // Non-initial, final state

        let transition = automata.add_transition(state1, vec!["a".to_string()], state2);

        // Verify transition properties
        assert_eq!(transition.action, vec!["a"]);
        assert_eq!(transition.next_state, state2);

        // Verify state1's transitions
        let transitions = state1.transitions.borrow();
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].action, vec!["a"]);
        assert_eq!(transitions[0].next_state, state2);
    }

    /// Tests that initial states are correctly added to the automaton.
    #[test]
    fn test_initial_states() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = Automata::new(&state_arena, &transition_arena);

        let state1 = automata.add_state(true, false); // Initial, non-final state
        automata.add_state(false, true); // Non-initial, final state

        // Verify initial states
        assert_eq!(automata.initial_states.len(), 1);
        assert_eq!(automata.initial_states[0], state1);
    }
}
