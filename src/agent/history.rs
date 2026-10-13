use crate::providers::Message;

pub struct History {
    messages: Vec<Message>,
    max_messages: usize,
}

impl History {
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);

        while self.messages.len() > self.max_messages {
            if let Some(pos) = self.messages.iter().position(|m| m.role != "system") {
                self.messages.remove(pos);
            } else {
                break;
            }
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn replace(&mut self, messages: Vec<Message>) {
        self.messages.clear();
        for message in messages {
            self.add_message(message);
        }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_new() {
        let history = History::new(10);
        assert!(history.messages().is_empty());
    }

    #[test]
    fn test_history_add_message() {
        let mut history = History::new(10);
        history.add_message(Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
        });
        assert_eq!(history.messages().len(), 1);
    }

    #[test]
    fn test_history_max_messages() {
        let mut history = History::new(2);
        history.add_message(Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
        });
        history.add_message(Message {
            role: "assistant".to_string(),
            content: "Hi".to_string(),
        });
        history.add_message(Message {
            role: "user".to_string(),
            content: "How are you?".to_string(),
        });
        assert_eq!(history.messages().len(), 2);
    }

    #[test]
    fn test_history_clear() {
        let mut history = History::new(10);
        history.add_message(Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
        });
        history.clear();
        assert!(history.messages().is_empty());
    }

    #[test]
    fn test_history_preserve_system_message() {
        let mut history = History::new(2);
        history.add_message(Message {
            role: "system".to_string(),
            content: "You are a helpful assistant".to_string(),
        });
        history.add_message(Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
        });
        history.add_message(Message {
            role: "assistant".to_string(),
            content: "Hi".to_string(),
        });
        history.add_message(Message {
            role: "user".to_string(),
            content: "How are you?".to_string(),
        });
        let msgs = history.messages();
        assert!(msgs.iter().any(|m| m.role == "system"));
    }
}
