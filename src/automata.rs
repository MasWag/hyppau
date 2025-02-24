use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
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

    /// A helper version of `project_with_map` that returns both the new EpsilonNFA
    /// **and** a mapping from old (pointer) state to the newly created state.
    ///
    /// # Arguments
    /// * `var` - the variable index to project
    /// * `states_arena`, `trans_arena`: where to allocate the new EpsilonNFA
    /// * `trans_arena`: where to allocate the new transitions
    /// * `is_final`: function to decide whether the state should be final
    /// * `old_to_new`: a map to fill with the old->new state mapping
    fn project_with_map<'b, F>(
        &self,
        var: usize,
        states_arena: &'b Arena<State<'b, Option<String>>>,
        trans_arena: &'b Arena<Transition<'b, Option<String>>>,
        is_final: F,
        old_to_new: &mut HashMap<*const NFAHState<'a>, &'b State<'b, Option<String>>>,
    ) -> EpsilonNFA<'b>
    where
        F: Fn(*const NFAHState<'a>) -> bool,
    {
        let mut projected = EpsilonNFA::new(states_arena, trans_arena, 0);

        let mut queue = std::collections::VecDeque::new();

        // 1) Create new states for each initial state in `self` and queue them
        for &init_state in &self.initial_states {
            let new_init = projected.add_state(true, is_final(init_state));
            old_to_new.insert(init_state as *const _, new_init);
            queue.push_back(init_state);
        }

        // 2) BFS replicate transitions
        while let Some(old_st) = queue.pop_front() {
            let new_st = old_to_new[&(old_st as *const _)];
            for &trans in old_st.transitions.borrow().iter() {
                let (ref action, old_var) = trans.label;
                let old_next = trans.next_state as *const _;

                // If next not seen, add
                if let std::collections::hash_map::Entry::Vacant(e) = old_to_new.entry(old_next) {
                    let new_next = projected.add_state(false, is_final(trans.next_state));
                    e.insert(new_next);
                    queue.push_back(trans.next_state);
                }
                let new_next = old_to_new[&old_next];

                // Label is Some(...) if old_var == var, else None
                let new_label = if old_var == var {
                    Some(action.clone())
                } else {
                    None
                };
                projected.add_transition(new_st, new_label, new_next);
            }
        }

        projected
    }

    /// Given a variable `var`, produce an epsilon-NFA over `String` by:
    ///  - turning transitions labeled with `(action, var)` into Some(action),
    ///  - and every other transition into an epsilon transition (None).
    ///
    /// We copy only those states reachable from the initial states (via BFS).
    /// Return a newly constructed `EpsilonNFA` that uses two new arenas for states/transitions.
    ///
    /// *If you need to track the old->new state mapping for further processing,
    /// consider using `project_with_map` directly.*
    pub fn project<'b>(
        &self,
        states_arena: &'b Arena<State<'b, Option<String>>>,
        trans_arena: &'b Arena<Transition<'b, Option<String>>>,
        var: usize,
    ) -> EpsilonNFA<'b> {
        // Just a thin wrapper around `project_with_map`, discarding the map
        let mut dummy_map = HashMap::new();
        self.project_with_map(
            var,
            states_arena,
            trans_arena,
            |old_state| unsafe { (*old_state).is_final },
            &mut dummy_map,
        )
    }

    /// Builds a projected EpsilonNFA over `String`, but forces exactly one final location = `loc`.
    ///
    /// Internally, it:
    ///   1) calls `project(...)` on the entire NFAH (with all old finals preserved)
    ///   2) then traverses the resulting EpsilonNFA, marking exactly the new state
    ///      that corresponds to `loc` as final, and all others not final.
    ///
    /// # Arguments
    /// * `loc` - pointer to the unique final location in the original NFAH
    /// * `var` - which variable index is being projected
    /// * `states_arena`, `trans_arena`: where to allocate the new EpsilonNFA
    pub fn project_with_final<'b>(
        &self,
        loc: *const NFAHState<'_>,
        var: usize,
        states_arena: &'b Arena<State<'b, Option<String>>>,
        trans_arena: &'b Arena<Transition<'b, Option<String>>>,
    ) -> EpsilonNFA<'b> {
        // First, do the normal projection
        // But we need to track old->new to correct final states afterward
        let mut old_to_new = HashMap::new();

        // We'll create a custom version of `project` that returns the map.
        self.project_with_map(
            var,
            states_arena,
            trans_arena,
            |old_state| old_state == (loc as *const NFAHState<'_>),
            &mut old_to_new,
        )
    }
}

impl<'a> EpsilonNFA<'a> {
    /// Return all states reachable by ε-transitions (None) from any state in `start_set`,
    /// including `start_set` itself. That is, the standard ε-closure for this automaton.
    fn epsilon_closure(
        &self,
        start_set: &HashSet<&'a State<'a, Option<String>>>,
    ) -> HashSet<&'a State<'a, Option<String>>> {
        let mut closure = start_set.clone();
        let mut queue: VecDeque<_> = start_set.iter().copied().collect();

        while let Some(st) = queue.pop_front() {
            // For every None (ε) transition out of st, add that state to closure
            for &trans in st.transitions.borrow().iter() {
                if trans.label.is_none() && !closure.contains(&trans.next_state) {
                    closure.insert(trans.next_state);
                    queue.push_back(trans.next_state);
                }
            }
        }
        closure
    }

    fn gather_alphabet(&self) -> HashSet<String> {
        let mut alphabet = HashSet::new();

        // BFS over all states to find transitions
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        for &initial_state in &self.initial_states {
            queue.push_back(initial_state);
            visited.insert(initial_state as *const _);
        }
        while let Some(st) = queue.pop_front() {
            for &trans in st.transitions.borrow().iter() {
                if let Some(ref sym) = trans.label {
                    alphabet.insert(sym.clone());
                }
                let nxt_ptr = trans.next_state as *const _;
                if !visited.contains(&nxt_ptr) {
                    visited.insert(nxt_ptr);
                    queue.push_back(trans.next_state);
                }
            }
        }
        alphabet
    }

    /// Convert this EpsilonNFA to a plain NFA over `String` via the powerset construction.
    ///
    /// * `new_states_arena`, `new_trans_arena`: typed_arena for the newly constructed NFA.
    ///
    /// Returns a new `NFA<'b>` whose states correspond to subsets of states from `self`.
    /// Transitions carry a `String` label (no epsilons left).
    pub fn to_nfa_powerset<'b>(
        &self,
        new_states_arena: &'b Arena<State<'b, String>>,
        new_trans_arena: &'b Arena<Transition<'b, String>>,
    ) -> NFA<'b> {
        // 1) Collect the full alphabet of "real" symbols (ignoring None).
        let alphabet = self.gather_alphabet();

        // 2) Create the new Automata structure for the resulting NFA
        let mut result = NFA::new(new_states_arena, new_trans_arena, 0);

        // 3) Our "super-states" in the new NFA are sets of old states in the EpsilonNFA.
        //    We'll store them in a canonical manner (for hashing/equality).
        //    A typical approach is to store the pointer addresses in a sorted Vec.
        //    We'll define a small function to produce a stable representation:
        fn canonical_set<'x>(
            states: &HashSet<&'x State<'x, Option<String>>>,
        ) -> Vec<*const State<'x, Option<String>>> {
            let mut addrs: Vec<_> = states
                .iter()
                .map(|s| *s as *const State<'x, Option<String>>)
                .collect::<Vec<*const State<'x, Option<String>>>>();
            addrs.sort();
            addrs
        }

        // 4) We compute the new initial super-state = ε-closure of all initial states.
        let mut initial_set = HashSet::new();
        for &initial_state in &self.initial_states {
            initial_set.insert(initial_state);
        }
        let initial_closure = self.epsilon_closure(&initial_set);

        // The new "super-state" for that closure:
        let init_canonical = canonical_set(&initial_closure);

        // A map from set-of-old-states -> new allocated State
        let mut subset_to_state = HashMap::new();

        // Create the new initial state in the result
        let is_final = initial_closure.iter().any(|st| st.is_final);
        let new_init = result.add_state(true, is_final);
        subset_to_state.insert(init_canonical.clone(), new_init);

        // 5) BFS over the powerset space: from each super-state S, for each symbol in `alphabet`,
        //    gather all destinations and take ε-closure. Then produce a new super-state if needed.
        let mut queue = VecDeque::new();
        queue.push_back(init_canonical);

        while let Some(current_set_repr) = queue.pop_front() {
            let current_state = subset_to_state[&current_set_repr];
            // Reconstruct the set from the canonical representation
            let mut current_set = HashSet::new();
            for ptr in &current_set_repr {
                current_set.insert(unsafe { &*(*ptr) }); // pointer -> reference
            }

            // For each symbol in the alphabet, collect all states reachable by that symbol
            for sym in &alphabet {
                let mut dest_set = HashSet::new();
                // For each old state in `current_set`, follow transitions labeled Some(sym)
                for &old_st in &current_set {
                    for &trans in old_st.transitions.borrow().iter() {
                        if let Some(ref label_str) = trans.label {
                            if label_str == sym {
                                dest_set.insert(trans.next_state);
                            }
                        }
                    }
                }
                // Then take ε-closure of that
                let closure = self.epsilon_closure(&dest_set);
                if closure.is_empty() {
                    // No next super-state => no transition
                    continue;
                }
                let closure_repr = canonical_set(&closure);

                // If we don’t already have a super-state for closure_repr, create one
                let new_super_state = match subset_to_state.get(&closure_repr) {
                    Some(&st) => st,
                    None => {
                        let is_final = closure.iter().any(|st| st.is_final);
                        let st_new = result.add_state(false, is_final);
                        subset_to_state.insert(closure_repr.clone(), st_new);
                        queue.push_back(closure_repr.clone());
                        st_new
                    }
                };

                // Add a transition in the result from current_state to new_super_state
                // labeled by `sym`.
                result.add_transition(current_state, sym.clone(), new_super_state);
            }
        }

        result
    }
}

impl<'a> NFA<'a> {
    /// Builds the (intersection) product automaton of `self` and `other`.
    ///
    /// Both NFAs must be over the same alphabet type (`String`).
    /// The resulting automaton is also an `NFA<'b>` using new arenas.
    ///
    /// Intersection semantics:
    ///  - The product state is final if both component states are final.
    ///  - A transition with label ℓ from (s1, s2) to (t1, t2) exists if
    ///    there is a transition labeled ℓ from s1 -> t1 in `self`
    ///    **and** a transition labeled ℓ from s2 -> t2 in `other`.
    pub fn product<'b>(
        &self,
        other: &NFA<'a>,
        new_states_arena: &'b Arena<State<'b, String>>,
        new_trans_arena: &'b Arena<Transition<'b, String>>,
    ) -> NFA<'b> {
        // Create the new NFA (product automaton).
        // Dimension can be 0 because we are not using "var" indexing here.
        let mut product_nfa = NFA::new(new_states_arena, new_trans_arena, 0);

        // We'll map (s1_ptr, s2_ptr) -> newly created product state
        let mut pair_to_state = HashMap::new();
        let mut queue = VecDeque::new();

        // 1) Create product initial states from all pairs of (init1, init2)
        for &init1 in &self.initial_states {
            for &init2 in &other.initial_states {
                let is_final = init1.is_final && init2.is_final;
                let prod_init = product_nfa.add_state(true, is_final);
                pair_to_state.insert((init1 as *const _, init2 as *const _), prod_init);
                queue.push_back((init1, init2));
            }
        }

        // 2) BFS in the space of (s1, s2) pairs
        while let Some((old_s1, old_s2)) = queue.pop_front() {
            // The product state we already created:
            let new_current = pair_to_state[&(old_s1 as *const _, old_s2 as *const _)];

            // We'll gather transitions by label for each side
            let mut transitions_1: HashMap<&str, Vec<&State<'_, String>>> = HashMap::new();
            for &t1 in old_s1.transitions.borrow().iter() {
                transitions_1
                    .entry(&t1.label)
                    .or_default()
                    .push(t1.next_state);
            }

            let mut transitions_2: HashMap<&str, Vec<&State<'_, String>>> = HashMap::new();
            for &t2 in old_s2.transitions.borrow().iter() {
                transitions_2
                    .entry(&t2.label)
                    .or_default()
                    .push(t2.next_state);
            }

            // For each label that appears in both transition sets, cross-product
            // all possible next states
            for (lbl, nexts1) in &transitions_1 {
                if let Some(nexts2) = transitions_2.get(lbl) {
                    // For each possible next pair (n1, n2)
                    for &n1 in nexts1 {
                        for &n2 in nexts2 {
                            let key = (n1 as *const _, n2 as *const _);
                            let new_next = match pair_to_state.get(&key) {
                                Some(&existing) => existing,
                                None => {
                                    let is_fin = n1.is_final && n2.is_final;
                                    let st_new = product_nfa.add_state(false, is_fin);
                                    pair_to_state.insert(key, st_new);
                                    queue.push_back((n1, n2));
                                    st_new
                                }
                            };
                            // Add the transition in the product
                            product_nfa.add_transition(new_current, (*lbl).to_string(), new_next);
                        }
                    }
                }
            }
        }

        product_nfa
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
    fn test_project() {
        use typed_arena::Arena;

        // Step 1: Create original NFAH with 3 possible variables: 0, 1, 2
        let nfa_states = Arena::new();
        let nfa_trans = Arena::new();
        let mut nfa_h = NFAH::new(&nfa_states, &nfa_trans, 3);

        // States s0 (initial, not final), s1 (not initial, final).
        let s0 = nfa_h.add_state(true, false);
        let s1 = nfa_h.add_state(false, true);

        // s0 --( "a", var=0 )--> s1
        nfa_h.add_nfah_transition(s0, "a".to_string(), 0, s1);
        // s0 --( "b", var=1 )--> s1
        nfa_h.add_nfah_transition(s0, "b".to_string(), 1, s1);
        // s1 --( "c", var=2 )--> s0
        nfa_h.add_nfah_transition(s1, "c".to_string(), 2, s0);

        // Step 2: Project onto var=1
        let eps_states = Arena::new();
        let eps_trans = Arena::new();
        let eps_nfa = nfa_h.project(&eps_states, &eps_trans, 1);

        // Step 3: Check the transitions in the resulting EpsilonNFA
        // We can BFS or simply check from each known state.
        // The BFS approach is typically how we find all states:

        use std::collections::{HashSet, VecDeque};
        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();

        for &init in &eps_nfa.initial_states {
            queue.push_back(init);
            seen.insert(init as *const _);
        }

        // We'll store transitions in a vector to check them more easily
        let mut transitions_vec = Vec::new();

        while let Some(st) = queue.pop_front() {
            for &trans in st.transitions.borrow().iter() {
                transitions_vec.push(trans);
                let nxt_ptr = trans.next_state as *const _;
                if !seen.contains(&nxt_ptr) {
                    queue.push_back(trans.next_state);
                    seen.insert(nxt_ptr);
                }
            }
        }

        // Now examine the transitions
        // - ( "b", var=1 ) in the old automaton => Some("b") in new automaton
        // - ( "a", var=0 ) => None
        // - ( "c", var=2 ) => None
        let mut found_some_b = false;
        let mut found_none_a = false;
        let mut found_none_c = false;

        for t in &transitions_vec {
            match &t.label {
                Some(label_str) if label_str == "b" => {
                    found_some_b = true;
                }
                None => {
                    // Could be from the old ( "a", var=0 ) or ( "c", var=2 )
                    // We won't know which original was which, but we know at least one was "a", one was "c".
                    // Here, we can attempt to see if next_state is final or not to guess.
                    let st_desc = format!("{:?}", t);
                    // Just do a quick check for variety
                    if st_desc.contains("a") {
                        found_none_a = true; // (heuristic if you do more direct debugging)
                    }
                    if st_desc.contains("c") {
                        found_none_c = true;
                    }
                }
                Some(other) => {
                    panic!("Found unexpected label: {:?}", other);
                }
            }
        }

        assert!(
            found_some_b,
            "Expected Some(\"b\") transition in projected automaton."
        );
        // Because we might not precisely differentiate the None edges by 'a' vs 'c',
        // just ensure that we saw 2 distinct None transitions in total:
        let none_count = transitions_vec.iter().filter(|t| t.label.is_none()).count();
        assert_eq!(none_count, 2, "Expected exactly 2 ε-transitions (None).");
    }

    #[test]
    fn test_epsilon_nfa_to_nfa_powerset() {
        use std::collections::{HashSet, VecDeque};
        use typed_arena::Arena;

        // 1) Build a small EpsilonNFA
        // States:
        //   s0 (initial), s1 (final), s2 (final)
        // Transitions:
        //   s0 -- None --> s1
        //   s1 -- Some("a") --> s2
        //   s2 -- None --> s0   (cyclic ε)
        let eps_states = Arena::new();
        let eps_trans = Arena::new();
        let mut eps_nfa = EpsilonNFA::new(&eps_states, &eps_trans, 0);

        let s0 = eps_nfa.add_state(true, false);
        let s1 = eps_nfa.add_state(false, true);
        let s2 = eps_nfa.add_state(false, true);

        // s0 -- ε --> s1
        eps_nfa.add_transition(s0, None, s1);
        // s1 -- "a" --> s2
        eps_nfa.add_transition(s1, Some("a".to_string()), s2);
        // s2 -- ε --> s0
        eps_nfa.add_transition(s2, None, s0);

        // 2) Convert to plain NFA
        let nfa_states = Arena::new();
        let nfa_trans = Arena::new();
        let result_nfa = eps_nfa.to_nfa_powerset(&nfa_states, &nfa_trans);

        // 3) Examine the resulting NFA
        // We'll BFS from the new initial states and collect transitions for inspection.
        assert_eq!(result_nfa.initial_states.len(), 1);
        let new_init = result_nfa.initial_states[0];

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        queue.push_back(new_init);
        visited.insert(new_init as *const _);

        let mut transitions_info = Vec::new();

        while let Some(st) = queue.pop_front() {
            for &tr in st.transitions.borrow().iter() {
                // record (from, label, to, is_final_of_to)
                transitions_info.push((
                    format!("{:p}", st),
                    tr.label.clone(),
                    format!("{:p}", tr.next_state),
                    tr.next_state.is_final,
                ));
                let nxt_ptr = tr.next_state as *const State<String>;
                if !visited.contains(&nxt_ptr) {
                    visited.insert(nxt_ptr);
                    queue.push_back(tr.next_state);
                }
            }
        }

        // We expect to see transitions labeled "a" in the new NFA, but no None transitions.
        // We also expect at least one final state in the new NFA because the original had final states.

        // Let's just check that there's at least one transition with label "a"
        // (the original E-NFA had "a" from s1 -> s2).
        let has_a_transition = transitions_info
            .iter()
            .any(|(_from, lbl, _to, _fin)| lbl == "a");
        assert!(
            has_a_transition,
            "Expected at least one transition labeled \"a\" in the powerset NFA."
        );

        // Check that there's at least one final state in the new NFA
        let any_final_state = visited.iter().any(|&ptr| unsafe { &*ptr }.is_final);
        assert!(
            any_final_state,
            "Expected at least one final state in the powerset-constructed NFA."
        );

        // For debugging, you could print out the transitions_info
        // println!("Transitions in powerset NFA: {:#?}", transitions_info);
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

    #[test]
    fn test_product_nfa() {
        use typed_arena::Arena;

        // Build first NFA: accepts "ab" or "aab" (small example)
        //
        //  States: s0 (initial), s1, s2 (final)
        //  Transitions:
        //    s0 --"a"--> s1
        //    s1 --"b"--> s2
        //    s0 --"a"--> s0  (so it can read multiple 'a's before a single 'b'?)
        //  => Accepts strings of one or more 'a' followed by a single 'b' ("ab", "aab", "aaab", ...)
        let arena_s1 = Arena::new();
        let arena_t1 = Arena::new();
        let mut nfa1 = NFA::new(&arena_s1, &arena_t1, 0);

        let s0_1 = nfa1.add_state(true, false);
        let s1_1 = nfa1.add_state(false, false);
        let s2_1 = nfa1.add_state(false, true); // final

        // transitions
        nfa1.add_transition(s0_1, "a".to_string(), s1_1);
        nfa1.add_transition(s1_1, "b".to_string(), s2_1);
        // optional "loop on a" at s0_1 if we want multiple a's
        nfa1.add_transition(s0_1, "a".to_string(), s0_1);

        // Build second NFA: accepts "aab" or "abc"
        //
        //   States: p0 (initial), p1, p2 (final), p3 (final)
        //   Transitions:
        //       p0 --"a"--> p0
        //       p0 --"a"--> p1
        //       p1 --"b"--> p2 (final)
        //       p1 --"b"--> p3 (final) but then p3 --"c"--> ??? Actually let's do:
        //         p1 --"b"--> p3
        //         p3 --"c"--> p2 (final). So "abc" is recognized if we do p0->p1->p3->p2 with "a","b","c"?
        //   => Accepts infinite set that always starts with >=1 'a',
        //      but has 2 final paths:  "ab" (p1->p2) or "abc" (p1->p3->p2).
        let arena_s2 = Arena::new();
        let arena_t2 = Arena::new();
        let mut nfa2 = NFA::new(&arena_s2, &arena_t2, 0);

        let p0_2 = nfa2.add_state(true, false);
        let p1_2 = nfa2.add_state(false, false);
        let p2_2 = nfa2.add_state(false, true); // final
        let p3_2 = nfa2.add_state(false, false);

        // transitions
        nfa2.add_transition(p0_2, "a".to_string(), p0_2);
        nfa2.add_transition(p0_2, "a".to_string(), p1_2);
        nfa2.add_transition(p1_2, "b".to_string(), p2_2);
        nfa2.add_transition(p1_2, "b".to_string(), p3_2);
        // let p3 lead to p2 on "c"
        nfa2.add_transition(p3_2, "c".to_string(), p2_2);

        // The second NFA accepts strings like "ab", "aab", "aaab", plus "abc", "aabc", etc.

        // 3) Build the product
        let product_s_arena = Arena::new();
        let product_t_arena = Arena::new();
        let product_nfa = nfa1.product(&nfa2, &product_s_arena, &product_t_arena);

        // 4) Check if the product is empty or not
        // Because both accept strings that start with >=1 'a' and end with 'b',
        // they definitely share some accepted strings. For example, "ab" is accepted by both:
        //   - NFA1: s0->s1->s2 with "a","b"  (or s0->s0->s0->...->s1->s2 if multiple 'a's)
        //   - NFA2: p0->p1->p2 with "a","b"
        // => The product language is definitely non-empty
        assert!(
            !product_nfa.is_empty(),
            "They both accept 'ab', so intersection is not empty."
        );

        // (Optional) We can do more checks, e.g. shortest_accepted_word_length
        let length = product_nfa.shortest_accepted_word_length();
        assert_eq!(
            length, 2,
            "Shortest word in the intersection is 'ab' of length 2."
        );

        // For more debugging, you could BFS over the product states and print them:
        // println!("product automaton states: ...");
    }

    #[test]
    fn test_project_with_final() {
        use typed_arena::Arena;

        // Step 1: Create original NFAH with 3 possible variables: 0, 1, 2
        let nfa_states = Arena::new();
        let nfa_trans = Arena::new();
        let mut nfa_h = NFAH::new(&nfa_states, &nfa_trans, 3);

        // States s0 (initial, not final), s1 (not initial, final).
        let s0 = nfa_h.add_state(true, false);
        let s1 = nfa_h.add_state(false, true);

        // s0 --( "a", var=0 )--> s1
        nfa_h.add_nfah_transition(s0, "a".to_string(), 0, s1);
        // s0 --( "b", var=1 )--> s1
        nfa_h.add_nfah_transition(s0, "b".to_string(), 1, s1);
        // s1 --( "c", var=0 )--> s0
        nfa_h.add_nfah_transition(s1, "c".to_string(), 0, s0);

        // Step 2: Project onto var=0
        let eps_states = Arena::new();
        let eps_trans = Arena::new();
        let eps_nfa = nfa_h.project(&eps_states, &eps_trans, 0);

        // Step 3: Check the transitions in the resulting EpsilonNFA
        // We can BFS or simply check from each known state.
        // The BFS approach is typically how we find all states:

        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();

        assert_eq!(1, eps_nfa.initial_states.len());
        for &init in &eps_nfa.initial_states {
            queue.push_back(init);
            seen.insert(init as *const _);
        }

        // We'll store transitions in a vector to check them more easily
        let mut transitions_vec = Vec::new();

        while let Some(st) = queue.pop_front() {
            for &trans in st.transitions.borrow().iter() {
                transitions_vec.push(trans);
                let nxt_ptr = trans.next_state as *const _;
                if !seen.contains(&nxt_ptr) {
                    queue.push_back(trans.next_state);
                    seen.insert(nxt_ptr);
                }
            }
        }

        // Now examine the transitions
        let mut found_some_a = false;
        let mut found_some_c = false;

        for t in &transitions_vec {
            match &t.label {
                Some(label_str) if label_str == "a" => {
                    found_some_a = true;
                }
                Some(label_str) if label_str == "c" => {
                    found_some_c = true;
                }
                None => {}
                Some(other) => {
                    panic!("Found unexpected label: {:?}", other);
                }
            }
        }

        assert!(
            found_some_a,
            "Expected Some(\"a\") transition in projected automaton."
        );
        assert!(
            found_some_c,
            "Expected Some(\"c\") transition in projected automaton."
        );
        let none_count = transitions_vec.iter().filter(|t| t.label.is_none()).count();
        assert_eq!(none_count, 1, "Expected exactly 1 ε-transitions (None).");
    }
}
