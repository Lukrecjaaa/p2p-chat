#[derive(Clone)]
pub struct ChatCompleter {
    commands: Vec<String>,
    friends: Vec<String>,
    discovered_peers: Vec<String>,
}

impl ChatCompleter {
    pub fn new(friends: Vec<String>) -> Self {
        let commands = vec![
            "send".to_string(),
            "history".to_string(),
            "friends".to_string(),
            "friend".to_string(),
            "peers".to_string(),
            "info".to_string(),
            "check".to_string(),
            "help".to_string(),
            "exit".to_string(),
        ];

        Self {
            commands,
            friends,
            discovered_peers: Vec::new(),
        }
    }

    pub fn update_friends(&mut self, friends: Vec<String>) {
        self.friends = friends;
    }

    pub fn update_discovered_peers(&mut self, peers: Vec<String>) {
        self.discovered_peers = peers;
    }

    pub fn get_suggestions(&self, input: &str) -> Vec<String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return self.commands.clone();
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();

        match parts.len() {
            1 => {
                // Completing command
                let prefix = parts[0].to_lowercase();
                let mut suggestions = Vec::new();

                // Exact matches first
                for cmd in &self.commands {
                    if cmd.starts_with(&prefix) {
                        suggestions.push(cmd.clone());
                    }
                }

                // Fuzzy matches
                for cmd in &self.commands {
                    if !cmd.starts_with(&prefix) && self.fuzzy_match(&prefix, cmd) {
                        suggestions.push(cmd.clone());
                    }
                }

                suggestions
            }
            2 => {
                // Completing first argument
                match parts[0] {
                    "send" | "history" => {
                        // Complete with friend nicknames/IDs
                        let prefix = parts[1].to_lowercase();
                        let mut suggestions = Vec::new();

                        for friend in &self.friends {
                            if friend.to_lowercase().starts_with(&prefix) {
                                suggestions.push(format!("{} {}", parts[0], friend));
                            }
                        }

                        // Add fuzzy matches
                        for friend in &self.friends {
                            if !friend.to_lowercase().starts_with(&prefix)
                                && self.fuzzy_match(&prefix, &friend.to_lowercase())
                            {
                                suggestions.push(format!("{} {}", parts[0], friend));
                            }
                        }

                        suggestions
                    }
                    "friend" => {
                        // For 'friend' command, suggest discovered peer IDs that match the prefix
                        let prefix = parts[1].to_lowercase();
                        let mut suggestions = Vec::new();

                        // Suggest discovered peers that start with the typed prefix
                        for peer_id in &self.discovered_peers {
                            if peer_id.to_lowercase().starts_with(&prefix) {
                                suggestions.push(format!("{} {}", parts[0], peer_id));
                            }
                        }

                        // If no matches, show placeholder
                        if suggestions.is_empty() {
                            suggestions.push(format!("{} <peer_id>", parts[0]));
                        }

                        suggestions
                    }
                    _ => Vec::new(),
                }
            }
            3 => {
                // Completing second argument
                match parts[0] {
                    "send" => {
                        // No autocomplete for message content - let users type freely
                        Vec::new()
                    }
                    "friend" => {
                        // Suggest e2e_key placeholder
                        vec![format!("{} {} <e2e_public_key>", parts[0], parts[1])]
                    }
                    _ => Vec::new(),
                }
            }
            4 => {
                // Completing third argument
                match parts[0] {
                    "friend" => {
                        // Suggest nickname placeholder
                        vec![format!(
                            "{} {} {} <optional_nickname>",
                            parts[0], parts[1], parts[2]
                        )]
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    fn fuzzy_match(&self, pattern: &str, text: &str) -> bool {
        if pattern.len() > text.len() {
            return false;
        }

        let mut pattern_chars = pattern.chars();
        let mut current_pattern = pattern_chars.next();

        for text_char in text.chars() {
            if let Some(pattern_char) = current_pattern {
                if text_char == pattern_char {
                    current_pattern = pattern_chars.next();
                }
            }
        }

        current_pattern.is_none()
    }
}
