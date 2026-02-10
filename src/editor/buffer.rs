use ropey::Rope;

/// Cursor position in the editor buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Zero-based line index.
    pub line: usize,
    /// Zero-based column (byte offset within the line).
    pub col: usize,
    /// Remembered column for vertical movement (sticky column).
    col_memory: usize,
}

impl Cursor {
    /// Create a cursor at line 0, column 0.
    pub const fn new() -> Self {
        Self {
            line: 0,
            col: 0,
            col_memory: 0,
        }
    }

    /// Create a cursor at a specific position.
    pub const fn at(line: usize, col: usize) -> Self {
        Self {
            line,
            col,
            col_memory: col,
        }
    }

    /// Update column and reset column memory to match.
    const fn set_col(&mut self, col: usize) {
        self.col = col;
        self.col_memory = col;
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Direction for cursor movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// A text buffer backed by a rope data structure.
///
/// Provides efficient insertion, deletion, and line-based operations
/// for editing text files. The cursor tracks the current editing position.
pub struct EditorBuffer {
    rope: Rope,
    cursor: Cursor,
    dirty: bool,
}

impl EditorBuffer {
    /// Create a new buffer from a string.
    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            cursor: Cursor::new(),
            dirty: false,
        }
    }

    /// Create an empty buffer.
    pub fn empty() -> Self {
        Self::from_text("")
    }

    /// The current cursor position.
    pub const fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Whether the buffer has been modified since creation or last save.
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the buffer as clean (e.g., after saving).
    pub const fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Total number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Get the content of a line (without trailing newline).
    pub fn line_at(&self, line_idx: usize) -> Option<String> {
        if line_idx >= self.rope.len_lines() {
            return None;
        }
        let line = self.rope.line(line_idx);
        let s = line.to_string();
        // Strip trailing newline if present
        Some(s.trim_end_matches('\n').trim_end_matches('\r').to_string())
    }

    /// Length of a line in bytes (without trailing newline).
    pub fn line_len(&self, line_idx: usize) -> usize {
        self.line_at(line_idx).map_or(0, |s| s.len())
    }

    /// The full text content of the buffer.
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        let char_idx = self.cursor_char_idx();
        self.rope.insert_char(char_idx, ch);
        self.cursor.set_col(self.cursor.col + ch.len_utf8());
        self.dirty = true;
    }

    /// Insert a string at the cursor position.
    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        let char_idx = self.cursor_char_idx();
        self.rope.insert(char_idx, s);

        // Move cursor to end of inserted text
        let lines: Vec<&str> = s.split('\n').collect();
        if lines.len() > 1 {
            self.cursor.line += lines.len() - 1;
            self.cursor.set_col(lines.last().map_or(0, |l| l.len()));
        } else {
            self.cursor.set_col(self.cursor.col + s.len());
        }
        self.dirty = true;
    }

    /// Split the current line at the cursor (Enter key).
    pub fn split_line(&mut self) {
        let char_idx = self.cursor_char_idx();
        self.rope.insert_char(char_idx, '\n');
        self.cursor.line += 1;
        self.cursor.set_col(0);
        self.dirty = true;
    }

    /// Delete the character before the cursor (Backspace).
    ///
    /// Returns `true` if a character was deleted.
    pub fn delete_back(&mut self) -> bool {
        if self.cursor.col == 0 && self.cursor.line == 0 {
            return false;
        }

        if self.cursor.col == 0 {
            // Join with previous line
            let prev_line_len = self.line_len(self.cursor.line - 1);
            let char_idx = self.cursor_char_idx();
            // Delete the newline at end of previous line
            self.rope.remove(char_idx - 1..char_idx);
            self.cursor.line -= 1;
            self.cursor.set_col(prev_line_len);
        } else {
            // Delete character before cursor
            let char_idx = self.cursor_char_idx();
            // Find the byte length of the character before cursor
            let line = self.rope.line(self.cursor.line);
            let line_str = line.to_string();
            let before = &line_str[..self.cursor.col];
            let prev_char_len = before.chars().next_back().map_or(1, char::len_utf8);
            self.rope.remove(char_idx - 1..char_idx);
            self.cursor.set_col(self.cursor.col - prev_char_len);
        }
        self.dirty = true;
        true
    }

    /// Delete the character at the cursor (Delete key).
    ///
    /// Returns `true` if a character was deleted.
    pub fn delete_forward(&mut self) -> bool {
        let line_len = self.line_len(self.cursor.line);

        if self.cursor.col >= line_len && self.cursor.line + 1 >= self.line_count() {
            return false;
        }

        let char_idx = self.cursor_char_idx();
        self.rope.remove(char_idx..=char_idx);
        self.dirty = true;
        true
    }

    /// Move the cursor in the given direction.
    pub fn move_cursor(&mut self, direction: Direction) {
        match direction {
            Direction::Left => self.move_left(),
            Direction::Right => self.move_right(),
            Direction::Up => self.move_up(),
            Direction::Down => self.move_down(),
        }
    }

    /// Move cursor to the beginning of the line (Home).
    pub const fn move_home(&mut self) {
        self.cursor.set_col(0);
    }

    /// Move cursor to the end of the line (End).
    pub fn move_end(&mut self) {
        let len = self.line_len(self.cursor.line);
        self.cursor.set_col(len);
    }

    /// Move cursor one word to the left (Ctrl+Left).
    pub fn move_word_left(&mut self) {
        if self.cursor.col == 0 {
            if self.cursor.line > 0 {
                self.cursor.line -= 1;
                self.cursor.set_col(self.line_len(self.cursor.line));
            }
            return;
        }

        let line = self.line_at(self.cursor.line).unwrap_or_default();
        let before = &line[..self.cursor.col];
        let trimmed = before.trim_end();

        if trimmed.is_empty() {
            self.cursor.set_col(0);
            return;
        }

        // Find start of previous word
        let pos = trimmed
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map_or(0, |i| i + 1);
        self.cursor.set_col(pos);
    }

    /// Move cursor one word to the right (Ctrl+Right).
    pub fn move_word_right(&mut self) {
        let line_len = self.line_len(self.cursor.line);

        if self.cursor.col >= line_len {
            if self.cursor.line + 1 < self.line_count() {
                self.cursor.line += 1;
                self.cursor.set_col(0);
            }
            return;
        }

        let line = self.line_at(self.cursor.line).unwrap_or_default();
        let after = &line[self.cursor.col..];

        // Skip current word characters
        let word_end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());

        // Skip whitespace/punctuation after word
        let rest = &after[word_end..];
        let space_end = rest
            .find(|c: char| c.is_alphanumeric() || c == '_')
            .unwrap_or(rest.len());

        self.cursor.set_col(self.cursor.col + word_end + space_end);
    }

    /// Move cursor to a specific line and column.
    pub fn move_to(&mut self, line: usize, col: usize) {
        let max_line = self.line_count().saturating_sub(1);
        self.cursor.line = line.min(max_line);
        let max_col = self.line_len(self.cursor.line);
        self.cursor.set_col(col.min(max_col));
    }

    /// Move cursor to the start of the buffer (Ctrl+Home).
    pub const fn move_to_start(&mut self) {
        self.cursor.line = 0;
        self.cursor.set_col(0);
    }

    /// Move cursor to the end of the buffer (Ctrl+End).
    pub fn move_to_end(&mut self) {
        let last_line = self.line_count().saturating_sub(1);
        self.cursor.line = last_line;
        self.cursor.set_col(self.line_len(last_line));
    }

    // --- Private helpers ---

    /// Convert cursor position to a ropey char index.
    fn cursor_char_idx(&self) -> usize {
        let line_start = self.rope.line_to_char(self.cursor.line);
        let line = self.rope.line(self.cursor.line);
        let line_str: String = line.chars().collect();
        // Convert byte offset to char offset within the line
        let byte_col = self.cursor.col.min(line_str.len());
        let char_offset = line_str[..byte_col].chars().count();
        line_start + char_offset
    }

    fn move_left(&mut self) {
        if self.cursor.col > 0 {
            let line = self.line_at(self.cursor.line).unwrap_or_default();
            let before = &line[..self.cursor.col];
            let prev_char_len = before.chars().next_back().map_or(1, char::len_utf8);
            self.cursor.set_col(self.cursor.col - prev_char_len);
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.set_col(self.line_len(self.cursor.line));
        }
    }

    fn move_right(&mut self) {
        let line_len = self.line_len(self.cursor.line);
        if self.cursor.col < line_len {
            let line = self.line_at(self.cursor.line).unwrap_or_default();
            let next_char_len = line[self.cursor.col..]
                .chars()
                .next()
                .map_or(1, char::len_utf8);
            self.cursor.set_col(self.cursor.col + next_char_len);
        } else if self.cursor.line + 1 < self.line_count() {
            self.cursor.line += 1;
            self.cursor.set_col(0);
        }
    }

    fn move_up(&mut self) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            let max_col = self.line_len(self.cursor.line);
            self.cursor.col = self.cursor.col_memory.min(max_col);
        }
    }

    fn move_down(&mut self) {
        if self.cursor.line + 1 < self.line_count() {
            self.cursor.line += 1;
            let max_col = self.line_len(self.cursor.line);
            self.cursor.col = self.cursor.col_memory.min(max_col);
        }
    }
}

impl std::fmt::Debug for EditorBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorBuffer")
            .field(
                "rope",
                &format_args!("Rope({} lines)", self.rope.len_lines()),
            )
            .field("cursor", &self.cursor)
            .field("dirty", &self.dirty)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Construction and basic queries ---

    #[test]
    fn test_empty_buffer_has_one_line() {
        let buf = EditorBuffer::empty();
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line_at(0), Some(String::new()));
    }

    #[test]
    fn test_from_text_preserves_content() {
        let buf = EditorBuffer::from_text("hello\nworld");
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.line_at(0), Some("hello".to_string()));
        assert_eq!(buf.line_at(1), Some("world".to_string()));
    }

    #[test]
    fn test_from_text_trailing_newline() {
        let buf = EditorBuffer::from_text("hello\n");
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.line_at(0), Some("hello".to_string()));
        assert_eq!(buf.line_at(1), Some(String::new()));
    }

    #[test]
    fn test_line_at_out_of_bounds_returns_none() {
        let buf = EditorBuffer::from_text("hello");
        assert_eq!(buf.line_at(1), None);
    }

    #[test]
    fn test_line_len() {
        let buf = EditorBuffer::from_text("hello\nhi");
        assert_eq!(buf.line_len(0), 5);
        assert_eq!(buf.line_len(1), 2);
    }

    #[test]
    fn test_text_roundtrip() {
        let content = "line one\nline two\nline three";
        let buf = EditorBuffer::from_text(content);
        assert_eq!(buf.text(), content);
    }

    // --- Cursor initial state ---

    #[test]
    fn test_cursor_starts_at_origin() {
        let buf = EditorBuffer::from_text("hello\nworld");
        assert_eq!(buf.cursor(), Cursor::at(0, 0));
    }

    // --- Dirty tracking ---

    #[test]
    fn test_new_buffer_is_clean() {
        let buf = EditorBuffer::from_text("hello");
        assert!(!buf.is_dirty());
    }

    #[test]
    fn test_insert_marks_dirty() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.insert_char('!');
        assert!(buf.is_dirty());
    }

    #[test]
    fn test_mark_clean_resets_dirty() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.insert_char('!');
        buf.mark_clean();
        assert!(!buf.is_dirty());
    }

    // --- Character insertion ---

    #[test]
    fn test_insert_char_at_start() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.insert_char('H');
        assert_eq!(buf.line_at(0), Some("Hhello".to_string()));
        assert_eq!(buf.cursor(), Cursor::at(0, 1));
    }

    #[test]
    fn test_insert_char_at_end() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_end();
        buf.insert_char('!');
        assert_eq!(buf.line_at(0), Some("hello!".to_string()));
        assert_eq!(buf.cursor(), Cursor::at(0, 6));
    }

    #[test]
    fn test_insert_char_in_middle() {
        let mut buf = EditorBuffer::from_text("hllo");
        buf.move_cursor(Direction::Right); // after 'h'
        buf.insert_char('e');
        assert_eq!(buf.line_at(0), Some("hello".to_string()));
        assert_eq!(buf.cursor(), Cursor::at(0, 2));
    }

    #[test]
    fn test_insert_multibyte_char() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_end();
        buf.insert_char('é');
        assert_eq!(buf.line_at(0), Some("helloé".to_string()));
    }

    // --- String insertion ---

    #[test]
    fn test_insert_str_single_line() {
        let mut buf = EditorBuffer::from_text("hd");
        buf.move_cursor(Direction::Right);
        buf.insert_str("ello worl");
        assert_eq!(buf.line_at(0), Some("hello world".to_string()));
    }

    #[test]
    fn test_insert_str_empty_is_noop() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.insert_str("");
        assert!(!buf.is_dirty());
        assert_eq!(buf.text(), "hello");
    }

    // --- Line splitting (Enter) ---

    #[test]
    fn test_split_line_at_end() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_end();
        buf.split_line();
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.line_at(0), Some("hello".to_string()));
        assert_eq!(buf.line_at(1), Some(String::new()));
        assert_eq!(buf.cursor(), Cursor::at(1, 0));
    }

    #[test]
    fn test_split_line_at_start() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.split_line();
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.line_at(0), Some(String::new()));
        assert_eq!(buf.line_at(1), Some("hello".to_string()));
        assert_eq!(buf.cursor(), Cursor::at(1, 0));
    }

    #[test]
    fn test_split_line_in_middle() {
        let mut buf = EditorBuffer::from_text("hello world");
        buf.move_to(0, 5);
        buf.split_line();
        assert_eq!(buf.line_at(0), Some("hello".to_string()));
        assert_eq!(buf.line_at(1), Some(" world".to_string()));
        assert_eq!(buf.cursor(), Cursor::at(1, 0));
    }

    // --- Backspace deletion ---

    #[test]
    fn test_delete_back_at_start_is_noop() {
        let mut buf = EditorBuffer::from_text("hello");
        assert!(!buf.delete_back());
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn test_delete_back_removes_char() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_to(0, 5);
        buf.delete_back();
        assert_eq!(buf.line_at(0), Some("hell".to_string()));
        assert_eq!(buf.cursor(), Cursor::at(0, 4));
    }

    #[test]
    fn test_delete_back_joins_lines() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(1, 0);
        buf.delete_back();
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line_at(0), Some("helloworld".to_string()));
        assert_eq!(buf.cursor(), Cursor::at(0, 5));
    }

    // --- Forward deletion (Delete key) ---

    #[test]
    fn test_delete_forward_at_end_is_noop() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_end();
        assert!(!buf.delete_forward());
    }

    #[test]
    fn test_delete_forward_removes_char() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.delete_forward();
        assert_eq!(buf.line_at(0), Some("ello".to_string()));
        assert_eq!(buf.cursor(), Cursor::at(0, 0));
    }

    #[test]
    fn test_delete_forward_joins_lines() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(0, 5);
        buf.delete_forward();
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line_at(0), Some("helloworld".to_string()));
        assert_eq!(buf.cursor(), Cursor::at(0, 5));
    }

    // --- Cursor movement: left/right ---

    #[test]
    fn test_move_left_at_start_is_noop() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_cursor(Direction::Left);
        assert_eq!(buf.cursor(), Cursor::at(0, 0));
    }

    #[test]
    fn test_move_left_decreases_col() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_to(0, 3);
        buf.move_cursor(Direction::Left);
        assert_eq!(buf.cursor(), Cursor::at(0, 2));
    }

    #[test]
    fn test_move_left_wraps_to_prev_line() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(1, 0);
        buf.move_cursor(Direction::Left);
        assert_eq!(buf.cursor(), Cursor::at(0, 5));
    }

    #[test]
    fn test_move_right_at_end_is_noop() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_end();
        buf.move_cursor(Direction::Right);
        assert_eq!(buf.cursor(), Cursor::at(0, 5));
    }

    #[test]
    fn test_move_right_increases_col() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_cursor(Direction::Right);
        assert_eq!(buf.cursor(), Cursor::at(0, 1));
    }

    #[test]
    fn test_move_right_wraps_to_next_line() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(0, 5);
        buf.move_cursor(Direction::Right);
        assert_eq!(buf.cursor(), Cursor::at(1, 0));
    }

    // --- Cursor movement: up/down ---

    #[test]
    fn test_move_up_at_first_line_is_noop() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_cursor(Direction::Up);
        assert_eq!(buf.cursor(), Cursor::at(0, 0));
    }

    #[test]
    fn test_move_down_at_last_line_is_noop() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(1, 0);
        buf.move_cursor(Direction::Down);
        assert_eq!(buf.cursor(), Cursor::at(1, 0));
    }

    #[test]
    fn test_move_up_preserves_column() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(1, 3);
        buf.move_cursor(Direction::Up);
        assert_eq!(buf.cursor(), Cursor::at(0, 3));
    }

    #[test]
    fn test_move_down_preserves_column() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(0, 3);
        buf.move_cursor(Direction::Down);
        assert_eq!(buf.cursor(), Cursor::at(1, 3));
    }

    #[test]
    fn test_move_up_clamps_to_shorter_line() {
        let mut buf = EditorBuffer::from_text("hi\nhello");
        buf.move_to(1, 4);
        buf.move_cursor(Direction::Up);
        // col is clamped to 2 ("hi" length) but col_memory stays at 4
        assert_eq!(buf.cursor().line, 0);
        assert_eq!(buf.cursor().col, 2);
    }

    #[test]
    fn test_move_down_clamps_to_shorter_line() {
        let mut buf = EditorBuffer::from_text("hello\nhi");
        buf.move_to(0, 4);
        buf.move_cursor(Direction::Down);
        // col is clamped to 2 ("hi" length) but col_memory stays at 4
        assert_eq!(buf.cursor().line, 1);
        assert_eq!(buf.cursor().col, 2);
    }

    // --- Column memory (sticky column) ---

    #[test]
    fn test_column_memory_across_short_line() {
        let mut buf = EditorBuffer::from_text("hello\nhi\nworld");
        buf.move_to(0, 4);
        buf.move_cursor(Direction::Down); // "hi" → col 2
        assert_eq!(buf.cursor().line, 1);
        assert_eq!(buf.cursor().col, 2);
        buf.move_cursor(Direction::Down); // "world" → col 4 (restored from memory)
        assert_eq!(buf.cursor().line, 2);
        assert_eq!(buf.cursor().col, 4);
    }

    // --- Home / End ---

    #[test]
    fn test_move_home() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_to(0, 3);
        buf.move_home();
        assert_eq!(buf.cursor(), Cursor::at(0, 0));
    }

    #[test]
    fn test_move_end() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_end();
        assert_eq!(buf.cursor(), Cursor::at(0, 5));
    }

    // --- Word movement ---

    #[test]
    fn test_move_word_left_from_middle_of_word() {
        let mut buf = EditorBuffer::from_text("hello world");
        buf.move_to(0, 8); // in "world"
        buf.move_word_left();
        assert_eq!(buf.cursor().col, 6); // start of "world"
    }

    #[test]
    fn test_move_word_left_from_start_of_word() {
        let mut buf = EditorBuffer::from_text("hello world");
        buf.move_to(0, 6); // start of "world"
        buf.move_word_left();
        assert_eq!(buf.cursor().col, 0); // start of "hello"
    }

    #[test]
    fn test_move_word_left_at_start_of_line_wraps() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(1, 0);
        buf.move_word_left();
        assert_eq!(buf.cursor(), Cursor::at(0, 5)); // end of prev line
    }

    #[test]
    fn test_move_word_right_from_start() {
        let mut buf = EditorBuffer::from_text("hello world");
        buf.move_word_right();
        assert_eq!(buf.cursor().col, 6); // start of "world"
    }

    #[test]
    fn test_move_word_right_at_end_of_line_wraps() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(0, 5);
        buf.move_word_right();
        assert_eq!(buf.cursor(), Cursor::at(1, 0));
    }

    // --- move_to ---

    #[test]
    fn test_move_to_clamps_line() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_to(100, 0);
        assert_eq!(buf.cursor().line, 0);
    }

    #[test]
    fn test_move_to_clamps_col() {
        let mut buf = EditorBuffer::from_text("hello");
        buf.move_to(0, 100);
        assert_eq!(buf.cursor().col, 5);
    }

    // --- move_to_start / move_to_end ---

    #[test]
    fn test_move_to_start() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to(1, 3);
        buf.move_to_start();
        assert_eq!(buf.cursor(), Cursor::at(0, 0));
    }

    #[test]
    fn test_move_to_end() {
        let mut buf = EditorBuffer::from_text("hello\nworld");
        buf.move_to_end();
        assert_eq!(buf.cursor(), Cursor::at(1, 5));
    }

    // --- Multi-byte character handling ---

    #[test]
    fn test_insert_and_navigate_multibyte() {
        let mut buf = EditorBuffer::from_text("café");
        buf.move_end();
        assert_eq!(buf.cursor().col, 5); // 'é' is 2 bytes
        buf.move_cursor(Direction::Left);
        assert_eq!(buf.cursor().col, 3); // before 'é'
    }

    #[test]
    fn test_delete_back_multibyte() {
        let mut buf = EditorBuffer::from_text("café");
        buf.move_end();
        buf.delete_back();
        assert_eq!(buf.line_at(0), Some("caf".to_string()));
    }

    // --- Complex editing sequences ---

    #[test]
    fn test_type_then_backspace_then_type() {
        let mut buf = EditorBuffer::from_text("");
        buf.insert_char('h');
        buf.insert_char('e');
        buf.insert_char('l');
        buf.delete_back();
        buf.insert_char('l');
        buf.insert_char('p');
        assert_eq!(buf.line_at(0), Some("help".to_string()));
    }

    #[test]
    fn test_split_and_rejoin() {
        let mut buf = EditorBuffer::from_text("helloworld");
        buf.move_to(0, 5);
        buf.split_line();
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.line_at(0), Some("hello".to_string()));
        assert_eq!(buf.line_at(1), Some("world".to_string()));

        buf.delete_back();
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line_at(0), Some("helloworld".to_string()));
    }
}
