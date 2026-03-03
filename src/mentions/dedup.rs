pub struct ContentDeduplicator {
    seen: std::collections::HashSet<String>,
}

impl Default for ContentDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentDeduplicator {
    pub fn new() -> Self {
        todo!()
    }

    pub fn is_duplicate(&mut self, _content: &str) -> bool {
        todo!()
    }
}
