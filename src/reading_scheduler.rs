use crate::hyper_pattern_matching::HyperPatternMatching;
use crate::multi_stream_reader::MultiStreamReader;

struct ReadingScheduler<Matching: HyperPatternMatching> {
    matching: Matching,
    reader: MultiStreamReader,
}

impl<Matching: HyperPatternMatching> ReadingScheduler<Matching> {
    pub fn new(matching: Matching, reader: MultiStreamReader) -> Self {
        Self { matching, reader }
    }

    pub fn run(&mut self) {
        let mut done: Vec<bool> = (0..self.reader.size()).map(|_| false).collect();
        while done.iter().any(|x| !*x) {
            for i in 0..self.reader.size() {
                if !done[i] {
                    let line = self.reader.read_line(i);
                    if line.is_err() {
                        done[i] = true;
                    } else {
                        let line = line.unwrap().trim_end().to_string();
                        self.matching.feed(&line, i);
                        let availability = self.reader.is_available(i);
                        done[i] = availability.is_err() || availability.is_ok_and(|f| !f);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::Automata;
    use crate::automata_runner::AppendOnlySequence;
    use crate::hyper_pattern_matching::NaiveHyperPatternMatching;
    use crate::multi_stream_reader::StreamSource;
    use crate::result_notifier::SharedBufferResultNotifier;
    use crate::shared_buffer::{SharedBuffer, SharedBufferSource};
    use std::collections::HashSet;
    use typed_arena::Arena;

    #[test]
    fn test_run() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = Automata::new(&state_arena, &transition_arena);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_transition(s1, vec!["a".to_string(), "b".to_string()], s2);
        automaton.add_transition(s1, vec!["a".to_string(), "".to_string()], s1);
        automaton.add_transition(s1, vec!["".to_string(), "b".to_string()], s1);
        automaton.add_transition(s1, vec!["".to_string(), "".to_string()], s1);
        automaton.add_transition(s1, vec!["c".to_string(), "d".to_string()], s3);

        let input_buffers = vec![SharedBuffer::new(), SharedBuffer::new()];
        let input_buffers_source: Vec<SharedBufferSource<&str>> =
            input_buffers.iter().map(|buf| buf.make_source()).collect();
        let reader = MultiStreamReader::new(
            input_buffers.clone()
                .into_iter()
                .map(|buf| Box::new(buf) as Box<dyn StreamSource>)
                .collect(),
        );

        let result_buffer = SharedBuffer::new();
        let notifier = SharedBufferResultNotifier::new(result_buffer.make_source());
        let mut result_sink = result_buffer.make_sink();

        let matching = NaiveHyperPatternMatching::<SharedBufferResultNotifier>::new(
            &automaton,
            notifier,
            vec![AppendOnlySequence::new(), AppendOnlySequence::new()],
        );

        let mut scheduler = ReadingScheduler::new(matching, reader);

        input_buffers_source[0].push("a");
        input_buffers_source[0].push("a");
        input_buffers_source[0].push("c");

        input_buffers_source[1].push("a");
        input_buffers_source[1].push("d");
        input_buffers_source[1].push("d");

        scheduler.run();

        let mut results = HashSet::new();
        let mut result = result_sink.pop();
        while result.is_some() {
            results.insert(result.unwrap());
            result = result_sink.pop();
        }

        assert_eq!(results.len(), 6);
        assert!(results.contains(&vec![0, 2, 1, 1]));
        assert!(results.contains(&vec![1, 2, 1, 1]));
        assert!(results.contains(&vec![2, 2, 1, 1]));
        assert!(results.contains(&vec![0, 2, 2, 2]));
        assert!(results.contains(&vec![1, 2, 2, 2]));
        assert!(results.contains(&vec![2, 2, 2, 2]));
    }
}
