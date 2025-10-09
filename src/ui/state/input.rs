use super::UIState;

impl UIState {
    pub fn safe_insert_char(&mut self, c: char) {
        let char_indices: Vec<_> = self.input_buffer.char_indices().collect();
        let byte_pos = if self.cursor_pos >= char_indices.len() {
            self.input_buffer.len()
        } else {
            char_indices[self.cursor_pos].0
        };

        self.input_buffer.insert(byte_pos, c);
        self.cursor_pos += 1;
    }

    pub fn safe_remove_char_before(&mut self) -> bool {
        if self.cursor_pos > 0 {
            let char_indices: Vec<_> = self.input_buffer.char_indices().collect();
            if self.cursor_pos <= char_indices.len() {
                let byte_pos = char_indices[self.cursor_pos - 1].0;
                self.input_buffer.remove(byte_pos);
                self.cursor_pos -= 1;
                return true;
            }
        }
        false
    }

    pub fn safe_remove_char_at(&mut self) -> bool {
        let char_indices: Vec<_> = self.input_buffer.char_indices().collect();
        if self.cursor_pos < char_indices.len() {
            let byte_pos = char_indices[self.cursor_pos].0;
            self.input_buffer.remove(byte_pos);
            return true;
        }
        false
    }

    pub fn safe_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn safe_cursor_right(&mut self) {
        let char_count = self.input_buffer.chars().count();
        if self.cursor_pos < char_count {
            self.cursor_pos += 1;
        }
    }

    pub fn safe_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn safe_cursor_end(&mut self) {
        self.cursor_pos = self.input_buffer.chars().count();
    }
}
