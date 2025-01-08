use crate::shared_buffer::SharedBufferSource;
use std::fs::File;
use std::io::{self, Write};

// Trait of a notifier of results
trait ResultNotifier {
    // Notify the result of hyper pattern matching, which is a vector of integers representing the ranges of the matching
    fn notify(&mut self, result: Vec<i32>);
}

// Notifyer to write the result to stdout
pub struct StdoutResultNotifier;

impl ResultNotifier for StdoutResultNotifier {
    fn notify(&mut self, result: Vec<i32>) {
        // read result as a sequence of pair of integers
        for i in (0..result.len()).step_by(2) {
            print!("({}, {})", result[i], result[i + 1]);
            if i + 2 < result.len() {
                print!(", ");
            } else {
                println!();
            }
        }
    }
}

// Notifier to write the result to SharedBuffer
pub struct SharedBufferResultNotifier {
    buffer: SharedBufferSource,
}

impl SharedBufferResultNotifier {
    pub fn new(buffer: SharedBufferSource) -> Self {
        Self { buffer }
    }
}

impl ResultNotifier for SharedBufferResultNotifier {
    fn notify(&mut self, result: Vec<i32>) {
        let mut line = String::new();
        // read result as a sequence of pair of integers
        for i in (0..result.len()).step_by(2) {
            line.push_str(&format!("({}, {})", result[i], result[i + 1]));
            if i + 2 < result.len() {
                line += ", ";
            }
        }
        self.buffer.push(&line);
    }
}

// Notifier to write the result to a file
pub struct FileResultNotifier {
    file: File,
}

impl FileResultNotifier {
    pub fn new(file_path: &str) -> io::Result<Self> {
        let file = File::create(file_path)?;
        Ok(Self { file })
    }
}

impl ResultNotifier for FileResultNotifier {
    fn notify(&mut self, result: Vec<i32>) {
        let mut line = String::new();
        // read result as a sequence of pair of integers
        for i in (0..result.len()).step_by(2) {
            line.push_str(&format!("({}, {})", result[i], result[i + 1]));
            if i + 2 < result.len() {
                line += ", ";
            }
        }
        writeln!(self.file, "{}", line).expect("Failed to write to file");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_buffer::SharedBuffer;
    use std::io::BufRead;
    use tempfile::NamedTempFile;

    #[test]
    fn test_stdout_result_notifier() {
        let mut notifier = StdoutResultNotifier;
        notifier.notify(vec![1, 2, 3, 4]);
        // Manually check the output
    }

    #[test]
    fn test_file_result_notifier() -> io::Result<()> {
        let temp_file = NamedTempFile::new()?;
        {
            let mut notifier = FileResultNotifier::new(temp_file.path().to_str().unwrap())?;
            notifier.notify(vec![1, 2, 3, 4]);
        }
        let content = std::fs::read_to_string(temp_file.path())?;
        assert_eq!(content.trim(), "(1, 2), (3, 4)");
        Ok(())
    }

    #[test]
    fn test_shared_buffer_result_notifier() {
        let mut buffer = SharedBuffer::new();
        let source = buffer.make_source();
        let mut notifier = SharedBufferResultNotifier::new(source);
        notifier.notify(vec![1, 2, 3, 4]);
        let mut result = String::new();
        let _ = buffer.read_line(&mut result);
        assert_eq!(result, "(1, 2), (3, 4)\n".to_string());
    }
}
