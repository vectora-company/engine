use crate::chat::ChatError;

/// Bounded SSE decoder that handles LF, CRLF, CR, comments and multi-line data.
pub(crate) struct SseDecoder {
    pending: Vec<u8>,
    data: Vec<String>,
    frame_bytes: usize,
    max_frame_bytes: usize,
    has_data: bool,
}

impl SseDecoder {
    pub(crate) fn new(max_frame_bytes: usize) -> Self {
        Self {
            pending: Vec::new(),
            data: Vec::new(),
            frame_bytes: 0,
            max_frame_bytes,
            has_data: false,
        }
    }

    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ChatError> {
        self.pending.extend_from_slice(bytes);
        let mut frames = Vec::new();
        loop {
            let Some((index, delimiter_len)) = line_delimiter(&self.pending, false) else {
                if self.pending.len().saturating_add(self.frame_bytes) > self.max_frame_bytes {
                    return Err(ChatError::ResponseTooLarge);
                }
                break;
            };
            let line = self.pending.drain(..index).collect::<Vec<_>>();
            self.pending.drain(..delimiter_len);
            self.process_line(&line, &mut frames)?;
        }
        Ok(frames)
    }

    pub(crate) fn finish(&mut self) -> Result<Vec<String>, ChatError> {
        let mut frames = Vec::new();
        if !self.pending.is_empty() {
            if self.pending.last() == Some(&b'\r') {
                self.pending.pop();
            }
            let line = std::mem::take(&mut self.pending);
            self.process_line(&line, &mut frames)?;
        }
        self.dispatch(&mut frames);
        Ok(frames)
    }

    fn process_line(&mut self, line: &[u8], frames: &mut Vec<String>) -> Result<(), ChatError> {
        self.frame_bytes = self.frame_bytes.saturating_add(line.len() + 1);
        if self.frame_bytes > self.max_frame_bytes {
            return Err(ChatError::ResponseTooLarge);
        }
        if line.is_empty() {
            self.dispatch(frames);
            return Ok(());
        }
        if line.first() == Some(&b':') {
            return Ok(());
        }
        let split = line.iter().position(|byte| *byte == b':');
        let (field, value) = match split {
            Some(index) => {
                let value = &line[index + 1..];
                (&line[..index], value.strip_prefix(b" ").unwrap_or(value))
            }
            None => (line, &[][..]),
        };
        if field == b"data" {
            let value = std::str::from_utf8(value).map_err(|_| ChatError::InvalidResponse)?;
            self.data.push(value.to_owned());
            self.has_data = true;
        }
        Ok(())
    }

    fn dispatch(&mut self, frames: &mut Vec<String>) {
        if self.has_data {
            frames.push(self.data.join("\n"));
        }
        self.data.clear();
        self.has_data = false;
        self.frame_bytes = 0;
    }
}

fn line_delimiter(bytes: &[u8], eof: bool) -> Option<(usize, usize)> {
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'\n' => return Some((index, 1)),
            b'\r' => {
                if bytes.get(index + 1) == Some(&b'\n') {
                    return Some((index, 2));
                }
                if bytes.get(index + 1).is_some() || eof {
                    return Some((index, 1));
                }
                return None;
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::SseDecoder;

    #[test]
    fn decodes_crlf_comments_multiline_and_partial_frames() {
        let mut decoder = SseDecoder::new(128);
        assert!(
            decoder
                .push(b": heartbeat\r\ndata: {\"x\":\r\n")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            decoder
                .push(b"data: 1}\r\n\r\ndata: [DONE]\r\n\r\n")
                .unwrap(),
            vec!["{\"x\":\n1}".to_owned(), "[DONE]".to_owned()]
        );
        assert!(decoder.finish().unwrap().is_empty());
    }

    #[test]
    fn rejects_an_unbounded_frame() {
        let mut decoder = SseDecoder::new(8);
        assert!(decoder.push(b"data: 123456789").is_err());
    }
}
