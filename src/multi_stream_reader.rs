use crate::shared_buffer::SharedBuffer;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};

/// A trait representing a generic stream source.
pub trait StreamSource: BufRead + Send {
    #[allow(unused)]
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Implement `StreamSource` for all types that implement `BufRead` and `Send`.
impl<T: BufRead + Send + 'static> StreamSource for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A multi-stream reader supporting generic stream sources.
pub struct MultiStreamReader {
    pub readers: Vec<Arc<Mutex<Box<dyn StreamSource>>>>,
    positions: Mutex<HashMap<usize, usize>>, // Keeps track of the read positions
}

impl MultiStreamReader {
    /// Constructs a new `MultiStreamReader` from a vector of generic stream sources.
    pub fn new(sources: Vec<Box<dyn StreamSource>>) -> Self {
        let mut readers = Vec::new();
        let mut positions = HashMap::new();

        for (i, source) in sources.into_iter().enumerate() {
            // Wrap each source in Arc<Mutex<_>>
            readers.push(Arc::new(Mutex::new(source)));
            positions.insert(i, 0);
        }

        Self {
            readers,
            positions: Mutex::new(positions),
        }
    }

    /// Returns the number of streams.
    pub fn size(&self) -> usize {
        self.readers.len()
    }

    /// Reads a line from the specified stream.
    pub fn read_line(&self, n: usize) -> io::Result<String> {
        let reader = self
            .readers
            .get(n)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid stream index"))?;

        let mut reader = reader.lock().unwrap(); // Acquire mutable access
        let mut line = String::new();
        reader.read_line(&mut line)?;

        // Update the position
        let mut positions = self.positions.lock().unwrap();
        if let Some(pos) = positions.get_mut(&n) {
            *pos += 1;
        }

        Ok(line)
    }

    /// Checks if a line can be read from the specified stream without blocking.
    pub fn is_available(&self, n: usize) -> io::Result<bool> {
        let reader = self
            .readers
            .get(n)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid stream index"))?;

        let mut reader = reader.lock().unwrap(); // Acquire mutable access
        match reader.fill_buf() {
            Ok(buf) => Ok(!buf.is_empty()), // Data available
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper function to create a temporary file with given content.
    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().expect("Failed to create temporary file");
        if !content.is_empty() {
            writeln!(temp_file, "{}", content).expect("Failed to write to temporary file");
        }
        temp_file
    }

    #[test]
    fn test_dynamic_buffer() -> io::Result<()> {
        let buffer1 = SharedBuffer::new();
        let buffer2 = SharedBuffer::new();

        buffer1.push("dynamic line1");
        buffer1.push("dynamic line2");
        buffer2.push("buffer lineA");
        buffer2.push("buffer lineB");

        let sources: Vec<Box<dyn StreamSource>> = vec![Box::new(buffer1), Box::new(buffer2)];
        let multi_reader = MultiStreamReader::new(sources);

        assert_eq!(multi_reader.read_line(0)?, "dynamic line1\n");
        assert_eq!(multi_reader.read_line(0)?, "dynamic line2\n");
        assert_eq!(multi_reader.read_line(1)?, "buffer lineA\n");
        assert_eq!(multi_reader.read_line(1)?, "buffer lineB\n");

        Ok(())
    }

    /// Tests initialization with stdin and temporary files.
    #[test]
    fn test_initialization() -> io::Result<()> {
        let temp_file1 = create_temp_file("line1\nline2");
        let temp_file2 = create_temp_file("lineA\nlineB");

        let sources: Vec<Box<dyn StreamSource>> = vec![
            Box::new(BufReader::new(io::stdin())),
            Box::new(BufReader::new(File::open(temp_file1.path())?)),
            Box::new(BufReader::new(File::open(temp_file2.path())?)),
        ];

        let reader = MultiStreamReader::new(sources);

        assert_eq!(reader.size(), 3);
        Ok(())
    }

    /// Tests reading lines from stdin and temporary files.
    #[test]
    fn test_read_line_with_temp_files() -> io::Result<()> {
        let temp_file1 = create_temp_file("line1\nline2");
        let temp_file2 = create_temp_file("lineA\nlineB");

        let sources: Vec<Box<dyn StreamSource>> = vec![
            Box::new(BufReader::new(File::open(temp_file1.path())?)),
            Box::new(BufReader::new(File::open(temp_file2.path())?)),
        ];

        let multi_reader = MultiStreamReader::new(sources);

        assert_eq!(multi_reader.read_line(0)?, "line1\n");
        assert_eq!(multi_reader.read_line(0)?, "line2\n");
        assert_eq!(multi_reader.read_line(1)?, "lineA\n");
        assert_eq!(multi_reader.read_line(1)?, "lineB\n");

        Ok(())
    }

    /// Tests reading lines and checking availability.
    #[test]
    fn test_is_available_with_temp_files() -> io::Result<()> {
        let temp_file1 = create_temp_file("line1\nline2");
        let temp_file2 = create_temp_file("");

        let sources: Vec<Box<dyn StreamSource>> = vec![
            Box::new(BufReader::new(File::open(temp_file1.path())?)),
            Box::new(BufReader::new(File::open(temp_file2.path())?)),
            Box::new(BufReader::new(File::open(temp_file1.path())?)),
        ];

        let reader = MultiStreamReader::new(sources);

        assert_eq!(reader.size(), 3);

        // Check availability and read lines
        assert!(reader.is_available(0)?);
        assert_eq!(reader.read_line(0)?, "line1\n");
        assert!(reader.is_available(0)?);
        assert_eq!(reader.read_line(0)?, "line2\n");

        // File 2 should not be available
        assert!(!reader.is_available(1)?);

        // Check availability and read lines
        assert!(reader.is_available(2)?);
        assert_eq!(reader.read_line(2)?, "line1\n");
        assert!(reader.is_available(2)?);
        assert_eq!(reader.read_line(2)?, "line2\n");

        Ok(())
    }

    #[test]
    fn test_is_available_with_dynamic_buffer() -> io::Result<()> {
        let buffer1 = SharedBuffer::new();
        let buffer2 = SharedBuffer::new();

        buffer1.push("dynamic line1");
        buffer2.push("buffer lineA");

        let buffer_source1 = buffer1.make_source();
        let buffer_source2 = buffer2.make_source();

        let sources: Vec<Box<dyn StreamSource>> =
            vec![Box::new(buffer1.clone()), Box::new(buffer2.clone())];
        let multi_reader = MultiStreamReader::new(sources);

        assert!(multi_reader.is_available(0).is_ok_and(|x| x));
        assert_eq!(multi_reader.read_line(0)?, "dynamic line1\n");
        assert!(multi_reader.is_available(0).is_ok_and(|x| !x));

        assert!(multi_reader.is_available(1).is_ok_and(|x| x));
        assert_eq!(multi_reader.read_line(1)?, "buffer lineA\n");
        assert!(multi_reader.is_available(1).is_ok_and(|x| !x));

        buffer_source1.push("dynamic line2");
        assert!(multi_reader.is_available(0).is_ok_and(|x| x));
        assert_eq!(multi_reader.read_line(0)?, "dynamic line2\n");

        buffer_source2.push("buffer lineB");
        assert!(multi_reader.is_available(1).is_ok_and(|x| x));
        assert_eq!(multi_reader.read_line(1)?, "buffer lineB\n");

        Ok(())
    }

    /// Tests handling invalid stream indices.
    #[test]
    fn test_invalid_index() -> io::Result<()> {
        let temp_file = create_temp_file("line1\nline2");

        let sources: Vec<Box<dyn StreamSource>> =
            vec![Box::new(BufReader::new(File::open(temp_file.path())?))];

        let reader = MultiStreamReader::new(sources);

        // Attempt to read from an invalid index
        let result = reader.read_line(1);
        assert!(result.is_err());

        // Attempt to check availability of an invalid index
        let result = reader.is_available(1);
        assert!(result.is_err());

        Ok(())
    }
}
