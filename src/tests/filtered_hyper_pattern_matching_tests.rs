use crate::automata::{NFAHState, NFAHTransition, NFAH};
use crate::filtered_hyper_pattern_matching::FilteredHyperPatternMatching;
use crate::filtered_single_hyper_pattern_matching::NaiveFilteredSingleHyperPatternMatching;
use crate::hyper_pattern_matching::HyperPatternMatching;
use crate::multi_stream_reader::{MultiStreamReader, StreamSource};
use crate::reading_scheduler::ReadingScheduler;
use crate::result_notifier::{MatchingResult, SharedBufferResultNotifier};
use crate::shared_buffer::SharedBuffer;
use typed_arena::Arena;

use super::utils::{verify_ids, verify_intervals};

/// Helper function to create a standard test automaton with 2 dimensions
fn create_small_automaton<'a>(
    state_arena: &'a Arena<NFAHState<'a>>,
    transition_arena: &'a Arena<NFAHTransition<'a>>,
) -> NFAH<'a> {
    let mut automaton = NFAH::new(state_arena, transition_arena, 2);

    // Create states
    let s0 = automaton.add_state(true, false); // Initial state
    let s1 = automaton.add_state(false, false);
    let s2 = automaton.add_state(false, false);
    let s3 = automaton.add_state(false, false);
    let s4 = automaton.add_state(false, true); // Final state

    // Add transitions
    automaton.add_nfah_transition(s0, "a".to_string(), 0, s1); // from: 0, to: 1, label: ["a", 0]
    automaton.add_nfah_transition(s1, "b".to_string(), 1, s2); // from: 1, to: 2, label: ["b", 1]
    automaton.add_nfah_transition(s0, "a".to_string(), 0, s0); // from: 0, to: 0, label: ["a", 0]
    automaton.add_nfah_transition(s0, "b".to_string(), 1, s0); // from: 0, to: 0, label: ["b", 1]
    automaton.add_nfah_transition(s0, "c".to_string(), 0, s3); // from: 0, to: 3, label: ["c", 0]
    automaton.add_nfah_transition(s3, "d".to_string(), 1, s4); // from: 3, to: 4, label: ["d", 1]

    automaton
}

/// Helper function to create a FilteredHyperPatternMatching instance with a result sink
fn create_matching<'a>(
    automaton: &'a NFAH<'a>,
) -> (
    FilteredHyperPatternMatching<
        'a,
        NaiveFilteredSingleHyperPatternMatching<'a, SharedBufferResultNotifier>,
        SharedBufferResultNotifier,
    >,
    impl FnMut() -> Option<MatchingResult>,
) {
    let result_buffer = SharedBuffer::new();
    let notifier = SharedBufferResultNotifier::new(result_buffer.make_source());
    let mut result_sink = result_buffer.make_sink();

    let matching = FilteredHyperPatternMatching::<
        NaiveFilteredSingleHyperPatternMatching<SharedBufferResultNotifier>,
        SharedBufferResultNotifier,
    >::new(automaton, notifier);

    let pop_result = move || result_sink.pop();

    (matching, pop_result)
}

#[test]
fn test_run() {
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

    let input_buffers = vec![SharedBuffer::new(), SharedBuffer::new()];
    let reader = MultiStreamReader::new(
        input_buffers
            .clone()
            .into_iter()
            .map(|buf| Box::new(buf) as Box<dyn StreamSource>)
            .collect(),
    );

    let result_buffer = SharedBuffer::new();
    let notifier = SharedBufferResultNotifier::new(result_buffer.make_source());
    let mut result_sink = result_buffer.make_sink();

    let matching = FilteredHyperPatternMatching::<
        NaiveFilteredSingleHyperPatternMatching<SharedBufferResultNotifier>,
        SharedBufferResultNotifier,
    >::new(&automaton, notifier);

    let mut scheduler = ReadingScheduler::new(matching, reader);

    input_buffers[0].push("a");
    input_buffers[1].push("b");
    input_buffers[0].push("a");
    input_buffers[1].push("b");
    input_buffers[0].push("c");
    input_buffers[1].push("d");

    scheduler.run();

    // Expected results as (start1, end1, start2, end2) for each match
    let expected_intervals = [
        vec![0, 2, 0, 2],
        vec![0, 2, 1, 2],
        vec![0, 2, 2, 2],
        vec![1, 2, 0, 2],
        vec![1, 2, 1, 2],
        vec![1, 2, 2, 2],
        vec![2, 2, 0, 2],
        vec![2, 2, 1, 2],
        vec![2, 2, 2, 2],
    ];

    // Collect all results
    let mut results = Vec::new();
    while let Some(result) = result_sink.pop() {
        results.push(result);
    }

    verify_intervals(&results, &expected_intervals);
}

#[test]
fn test_small() {
    let state_arena = Arena::new();
    let transition_arena = Arena::new();
    let automaton = create_small_automaton(&state_arena, &transition_arena);

    let (mut matching, mut pop_result) = create_matching(&automaton);

    // Feed input sequence
    matching.feed("a", 0);
    matching.consume();
    matching.feed("a", 1);
    matching.consume();
    matching.feed("a", 0);
    matching.consume();
    matching.feed("d", 1);
    matching.consume();
    matching.feed("c", 0);
    matching.consume();
    matching.set_eof(0);
    matching.feed("d", 1);
    matching.set_eof(1);
    matching.consume();
    matching.consume_remaining();

    // The expected results as (start1, end1, start2, end2) for each match
    let expected_intervals = [
        vec![0, 2, 1, 1],
        vec![0, 2, 1, 1],
        vec![0, 2, 2, 2],
        vec![1, 2, 1, 1],
        vec![1, 2, 2, 2],
        vec![2, 2, 1, 1],
        vec![2, 2, 2, 2],
    ];

    // Collect all results
    let mut results = Vec::new();
    while let Some(result) = pop_result() {
        results.push(result);
    }

    verify_intervals(&results, &expected_intervals);
}

#[test]
fn test_small_double() {
    let state_arena = Arena::new();
    let transition_arena = Arena::new();
    let automaton = create_small_automaton(&state_arena, &transition_arena);

    let (mut matching, mut pop_result) = create_matching(&automaton);

    // Feed first sequence
    matching.feed("a", 0);
    matching.consume();
    matching.feed("a", 1);
    matching.consume();
    matching.feed("a", 0);
    matching.consume();
    matching.feed("d", 1);
    matching.consume();
    matching.feed("c", 0);
    matching.consume();
    matching.feed("d", 1);
    matching.set_eof(1);
    matching.consume();

    // Feed second sequence
    matching.feed("a", 0);
    matching.consume();
    matching.feed("a", 0);
    matching.consume();
    matching.feed("c", 0);
    matching.set_eof(0);
    matching.consume_remaining();

    // The expected results as (start1, end1, start2, end2) for each match
    let expected_intervals = vec![
        vec![0, 2, 1, 1],
        vec![0, 2, 1, 1],
        vec![0, 2, 1, 1],
        vec![0, 2, 1, 1],
        vec![0, 2, 1, 1],
        vec![0, 2, 1, 1],
        vec![0, 2, 2, 2],
        vec![1, 2, 1, 1],
        vec![1, 2, 2, 2],
        vec![2, 2, 1, 1],
        vec![2, 2, 2, 2],
        vec![3, 5, 1, 1],
        vec![3, 5, 2, 2],
        vec![4, 5, 1, 1],
        vec![4, 5, 2, 2],
        vec![5, 5, 1, 1],
        vec![5, 5, 2, 2],
    ];

    // Collect all results
    let mut results = Vec::new();
    while let Some(result) = pop_result() {
        results.push(result);
    }

    verify_intervals(&results, &expected_intervals);
}

/// Test with a more complex automaton and input sequence
#[test]
fn test_complex_pattern() {
    let state_arena = Arena::new();
    let transition_arena = Arena::new();
    let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

    // Create a more complex automaton that matches (a+b*c, d+e*f)
    let s0 = automaton.add_state(true, false);
    let s1 = automaton.add_state(false, false);
    let s2 = automaton.add_state(false, false);
    let s3 = automaton.add_state(false, false);
    let s4 = automaton.add_state(false, false);
    let s5 = automaton.add_state(false, true);

    // First dimension: a+b*c
    automaton.add_nfah_transition(s0, "a".to_string(), 0, s1);
    automaton.add_nfah_transition(s1, "a".to_string(), 0, s1);
    automaton.add_nfah_transition(s1, "b".to_string(), 0, s2);
    automaton.add_nfah_transition(s2, "b".to_string(), 0, s2);
    automaton.add_nfah_transition(s2, "c".to_string(), 0, s3);
    automaton.add_nfah_transition(s1, "c".to_string(), 0, s3);

    // Second dimension: d+e*f
    automaton.add_nfah_transition(s3, "d".to_string(), 1, s4);
    automaton.add_nfah_transition(s4, "d".to_string(), 1, s4);
    automaton.add_nfah_transition(s4, "e".to_string(), 1, s4);
    automaton.add_nfah_transition(s4, "f".to_string(), 1, s5);

    let (mut matching, mut pop_result) = create_matching(&automaton);

    // Feed input sequence
    matching.feed("a", 0);
    matching.consume();
    matching.feed("a", 0);
    matching.consume();
    matching.feed("b", 0);
    matching.consume();
    matching.feed("c", 0);
    matching.consume();
    matching.feed("d", 1);
    matching.consume();
    matching.feed("d", 1);
    matching.consume();
    matching.feed("e", 1);
    matching.consume();
    matching.feed("f", 1);
    matching.consume();
    matching.set_eof(0);
    matching.set_eof(1);
    matching.consume_remaining();

    // Collect all results
    let mut results = Vec::new();
    while let Some(result) = pop_result() {
        results.push(result);
    }

    // We don't check specific expected results here since they're complex,
    // but we verify that we got some results and they have the right structure
    assert!(!results.is_empty(), "Should have found some matches");

    for result in results {
        assert_eq!(
            result.intervals.len(),
            2,
            "Each result should have 2 intervals"
        );
        assert_eq!(result.ids.len(), 2, "Each result should have 2 IDs");
    }
}

/// Test using the small automaton with input from abcd.log
#[test]
fn test_small_with_abcd_10() {
    let state_arena = Arena::new();
    let transition_arena = Arena::new();
    let automaton = create_small_automaton(&state_arena, &transition_arena);

    let (mut matching, mut pop_result) = create_matching(&automaton);

    // Inputs generated by `seq 10 | gen_abcd.awk`
    let inputs = vec!["d", "b", "d", "d", "d", "a", "b", "d", "b", "c"];

    // Feed all the inputs to stream 0
    for input in inputs.iter() {
        matching.feed(input, 0);
        matching.consume();
    }

    // Set EOF for stream 0
    matching.set_eof(0);

    // We do not use stream 1
    matching.set_eof(1);

    // Collect all results
    let mut results = Vec::new();
    while let Some(result) = pop_result() {
        results.push(result);
    }

    // The expected results as (start1, end1, start2, end2) for each match
    let expected_intervals = [
        vec![9, 9, 0, 0],
        vec![9, 9, 1, 2],
        vec![9, 9, 2, 2],
        vec![9, 9, 3, 3],
        vec![9, 9, 4, 4],
        vec![9, 9, 6, 7],
        vec![9, 9, 7, 7],
    ];

    let expected_ids = vec![
        vec![0, 0],
        vec![0, 0],
        vec![0, 0],
        vec![0, 0],
        vec![0, 0],
        vec![0, 0],
        vec![0, 0],
    ];

    verify_intervals(&results, &expected_intervals);

    verify_ids(&results, &expected_ids);
}
