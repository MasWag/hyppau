use crate::shared_buffer::SharedBufferSource;
use std::fs::File;
use std::io::{self, Write};

/// Represents a matching interval
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
pub struct MatchingInterval {
    pub start: usize,
    pub end: usize,
}

impl MatchingInterval {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchingResult {
    pub intervals: Vec<MatchingInterval>,
    pub ids: Vec<usize>,
}

impl MatchingResult {
    pub fn new(intervals: Vec<MatchingInterval>, ids: Vec<usize>) -> Self {
        if intervals.len() != ids.len() {
            panic!("intervals and ids must have the same length");
        }
        Self { intervals, ids }
    }
}

/// A trait representing a way to notify or record matching results.
///
/// Each intervals is a sequence of `MatchingInterval` values interpreted as `(start, end)` pairs.
/// For example, if `intervals = [[1, 2], [3, 4]]`, that represents two matches:
/// (1, 2) and (3, 4).
pub trait ResultNotifier {
    /// Notifies a intervals, represented as a slice of `MatchingInterval` pairs.
    fn notify(&mut self, intervals: &[MatchingInterval], ids: &[usize]);
}

/// A `ResultNotifier` implementation that prints matched ranges to `stdout`.
///
/// # Examples
///
/// ```rust,ignore
/// let mut notifier = StdoutResultNotifier;
/// notifier.notify(&[[1, 2], [3, 4]]); // prints "(1, 2), (3, 4)"
/// ```
pub struct StdoutResultNotifier;

impl ResultNotifier for StdoutResultNotifier {
    fn notify(&mut self, intervals: &[MatchingInterval], ids: &[usize]) {
        // Build a single string containing all pairs, then print once.
        // This approach is efficient in a single-threaded context.
        let mut output = String::new();
        for i in 0..intervals.len() {
            output.push_str(&format!(
                "({}: {}, {})",
                ids[i], intervals[i].start, intervals[i].end
            ));
            if i + 1 < intervals.len() {
                output.push_str(", ");
            }
        }
        println!("{}", output);
    }
}

/// A `ResultNotifier` that stores matched ranges in a shared in-memory buffer.
///
/// # Type Parameters
///
/// - `SharedBufferSource<Vec<MatchingInterval>>` is the source/sink type for the buffer.
///
pub struct SharedBufferResultNotifier {
    buffer: SharedBufferSource<MatchingResult>,
}

impl SharedBufferResultNotifier {
    /// Creates a new `SharedBufferResultNotifier` from a `SharedBufferSource`.
    pub fn new(buffer: SharedBufferSource<MatchingResult>) -> Self {
        Self { buffer }
    }
}

impl ResultNotifier for SharedBufferResultNotifier {
    fn notify(&mut self, intervals: &[MatchingInterval], ids: &[usize]) {
        // We clone because we need to store these values beyond the scope of this call.
        self.buffer.push(MatchingResult {
            intervals: intervals.to_vec(),
            ids: ids.to_vec(),
        });
    }
}

/// A `ResultNotifier` that writes matched ranges to a file.
///
/// # Examples
///
/// ```rust,ignore
/// let mut notifier = FileResultNotifier::new("output.txt").unwrap();
/// notifier.notify(&[[1, 2], [3, 4]]); // writes "(1, 2), (3, 4)" to "output.txt"
/// ```
pub struct FileResultNotifier {
    file: File,
}

impl FileResultNotifier {
    /// Creates a new `FileResultNotifier` that writes to the specified file path.
    ///
    /// # Errors
    ///
    /// Returns an `io::Error` if the file cannot be created.
    pub fn new(file_path: &str) -> io::Result<Self> {
        let file = File::create(file_path)?;
        Ok(Self { file })
    }
}

impl ResultNotifier for FileResultNotifier {
    fn notify(&mut self, intervals: &[MatchingInterval], ids: &[usize]) {
        // Build a single line, then write it at once.
        let mut line = String::new();
        for i in 0..intervals.len() {
            line.push_str(&format!(
                "{}: ({}, {})",
                ids[i], intervals[i].start, intervals[i].end
            ));
            if i + 1 < intervals.len() {
                line.push_str(", ");
            }
        }
        // Use `writeln!` to append a newline.
        writeln!(self.file, "{}", line).expect("Failed to write to file");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_buffer::SharedBuffer;
    use tempfile::NamedTempFile;

    #[test]
    fn test_stdout_result_notifier() {
        // Hard to test stdout automatically, but we can call it to ensure no panics.
        let mut notifier = StdoutResultNotifier;
        notifier.notify(
            &[MatchingInterval::new(1, 2), MatchingInterval::new(3, 4)],
            &[0, 1],
        );
        // Manually check the console output if needed.
    }

    #[test]
    fn test_file_result_notifier() -> io::Result<()> {
        let temp_file = NamedTempFile::new()?;
        {
            let mut notifier = FileResultNotifier::new(temp_file.path().to_str().unwrap())?;
            notifier.notify(
                &[MatchingInterval::new(1, 2), MatchingInterval::new(3, 4)],
                &[0, 1],
            );
        }
        let content = std::fs::read_to_string(temp_file.path())?;
        assert_eq!(content.trim(), "0: (1, 2), 1: (3, 4)");
        Ok(())
    }

    #[test]
    fn test_shared_buffer_result_notifier() {
        let buffer = SharedBuffer::new();
        let source = buffer.make_source();
        let mut notifier = SharedBufferResultNotifier::new(source);
        notifier.notify(
            &[MatchingInterval::new(1, 2), MatchingInterval::new(3, 4)],
            &[0, 1],
        );

        let mut sink = buffer.make_sink();
        let result = sink.pop().expect("No data in shared buffer");
        assert_eq!(result.intervals.len(), 2);
        assert_eq!(result.ids.len(), 2);
        assert_eq!(
            result,
            MatchingResult {
                intervals: vec![MatchingInterval::new(1, 2), MatchingInterval::new(3, 4)],
                ids: vec![0, 1]
            }
        );
    }
}
