use std::collections::VecDeque;
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};

/// A dynamic buffer that supports concurrent reads and writes.
///
/// This structure is designed for real-time data streams where multiple producers
/// and consumers can interact with the buffer safely.
pub struct SharedBuffer<T> {
    buffer: Arc<Mutex<VecDeque<T>>>,
    internal_buf: Vec<u8>, // Persistent internal buffer for `fill_buf`
}

impl<T: Clone> SharedBuffer<T> {
    /// Creates a new shared buffer.
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            internal_buf: Vec::new(),
        }
    }

    /// Adds a line of data to the buffer.
    ///
    /// # Arguments
    /// - `line`: The data to be added.
    pub fn push(&self, line: T) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push_back(line);
    }

    /// Creates a source for the shared buffer.
    ///
    /// # Returns
    /// A `SharedBufferSource` that can write to the buffer.
    pub fn make_source(&self) -> SharedBufferSource<T> {
        SharedBufferSource::new(self.buffer.clone())
    }

    /// Creates a sink for the shared buffer.
    ///
    /// # Returns
    /// A `SharedBufferSink` that can read from the buffer.
    pub fn make_sink(&self) -> SharedBufferSink<T> {
        SharedBufferSink::new(self.buffer.clone())
    }
}

impl<T> Clone for SharedBuffer<T> {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            internal_buf: Vec::new(),
        }
    }
}

impl io::Read for SharedBuffer<&str> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buffer = self.buffer.lock().unwrap();

        if let Some(line) = buffer.pop_front() {
            let bytes = line.as_bytes();
            let len = bytes.len().min(buf.len());
            buf[..len].copy_from_slice(&bytes[..len]);
            Ok(len)
        } else {
            Ok(0) // No more data to read
        }
    }
}

impl BufRead for SharedBuffer<&str> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.internal_buf.is_empty() {
            if let Some(line) = self.buffer.lock().unwrap().pop_front() {
                self.internal_buf.extend_from_slice(line.as_bytes());
                // We need to add a line feed to the internal buffer
                self.internal_buf.push(b'\n');
            }
        }
        Ok(&self.internal_buf)
    }

    fn consume(&mut self, amt: usize) {
        if amt > self.internal_buf.len() {
            panic!("Cannot consume more than available");
        } else {
            self.internal_buf.drain(..amt);
        }
    }
}

/// A producer for the `SharedBuffer`, allowing data to be added.
pub struct SharedBufferSource<T> {
    buffer: Arc<Mutex<VecDeque<T>>>,
}

impl<T> SharedBufferSource<T> {
    /// Creates a new source for the given shared buffer.
    ///
    /// # Arguments
    /// - `buffer`: The shared buffer to which this source will write data.
    pub fn new(buffer: Arc<Mutex<VecDeque<T>>>) -> Self {
        Self { buffer }
    }

    /// Adds a line of data to the buffer.
    ///
    /// # Arguments
    /// - `line`: The data to be added to the buffer.
    pub fn push(&self, line: T) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push_back(line);
    }
}

/// A consumer for the `SharedBuffer`, allowing data to be read in sequence.
pub struct SharedBufferSink<T> {
    buffer: Arc<Mutex<VecDeque<T>>>,
    start: usize, // Start index of the readable range
}

impl<T: Clone> SharedBufferSink<T> {
    /// Creates a new sink for the given shared buffer.
    ///
    /// # Arguments
    /// - `buffer`: The shared buffer from which this sink will read data.
    pub fn new(buffer: Arc<Mutex<VecDeque<T>>>) -> Self {
        Self { buffer, start: 0 }
    }

    /// Reads the next line of data from the buffer.
    ///
    /// # Returns
    /// - `Some(T)`: The next line of data if available.
    /// - `None`: If no more data is available.
    pub fn pop(&mut self) -> Option<T> {
        let buffer = self.buffer.lock().unwrap();
        if buffer.len() > self.start {
            self.start += 1;
            Some(buffer[self.start - 1].clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_pop() {
        let buffer = SharedBuffer::new();
        let source = buffer.make_source();
        let mut sink = buffer.make_sink();

        source.push("hello");
        source.push("world");

        assert_eq!(sink.pop(), Some("hello"));
        assert_eq!(sink.pop(), Some("world"));
        assert_eq!(sink.pop(), None);
    }

    #[test]
    fn test_shared_access() {
        let buffer = SharedBuffer::new();
        let source = buffer.make_source();
        let mut sink1 = buffer.make_sink();
        let mut sink2 = buffer.make_sink();

        source.push("data1");
        source.push("data2");

        assert_eq!(sink1.pop(), Some("data1"));
        assert_eq!(sink2.pop(), Some("data1"));
        assert_eq!(sink1.pop(), Some("data2"));
        assert_eq!(sink2.pop(), Some("data2"));
    }

    #[test]
    fn test_read_line_trait() {
        let mut buffer = SharedBuffer::new();
        buffer.push("line1");
        buffer.push("line2");

        let mut output: String = "".to_string();
        assert_eq!(buffer.read_line(&mut output).unwrap(), 6);
        assert_eq!(&output, "line1\n");

        output.clear();
        assert_eq!(buffer.read_line(&mut output).unwrap(), 6);
        assert_eq!(&output, "line2\n");

        assert_eq!(buffer.read_line(&mut output).unwrap(), 0);
    }

    #[test]
    fn test_buf_read_trait() {
        let mut buffer = SharedBuffer::new();
        buffer.push("line1");
        buffer.push("line2");

        assert_eq!(buffer.fill_buf().unwrap(), b"line1\n");
        buffer.consume(6); // Consume "line1\n"

        assert_eq!(buffer.fill_buf().unwrap(), b"line2\n");
        buffer.consume(6); // Consume "line2\n"

        assert_eq!(buffer.fill_buf().unwrap(), b"");
    }
}
