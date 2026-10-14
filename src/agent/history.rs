use crate::providers::Message;

pub struct History {
    messages: Vec<Message>,
    max_messages: usize,
    max_chars: usize,
}

impl History {
    pub fn new(max_messages: usize, max_chars: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages,
            max_chars,
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

    /// 送給模型的歷史。超過字元上限時，從最舊的非 system 訊息開始移除。
    /// 最新的使用者訊息，以及它之後尚未完成的工具結果，會留下。
    pub fn for_provider(&self) -> Vec<Message> {
        let mut kept = self.messages.clone();
        while content_chars(&kept) > self.max_chars {
            let protected_from = kept.iter().rposition(|message| message.role == "user");
            let Some(pos) = kept.iter().enumerate().position(|(index, message)| {
                message.role != "system"
                    && protected_from.is_none_or(|start| index < start)
            }) else {
                break;
            };
            kept.remove(pos);
        }
        kept
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

fn content_chars(messages: &[Message]) -> usize {
    messages.iter().map(|message| message.content.chars().count()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_new() {
        let history = History::new(10, 32_000);
        assert!(history.messages().is_empty());
    }

    #[test]
    fn test_history_add_message() {
        let mut history = History::new(10, 32_000);
        history.add_message(Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
        });
        assert_eq!(history.messages().len(), 1);
    }

    #[test]
    fn test_history_max_messages() {
        let mut history = History::new(2, 32_000);
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
        let mut history = History::new(10, 32_000);
        history.add_message(Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
        });
        history.clear();
        assert!(history.messages().is_empty());
    }

    #[test]
    fn test_history_preserve_system_message() {
        let mut history = History::new(2, 32_000);
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

    #[test]
    fn test_history_drops_oldest_by_chars_but_keeps_latest_user_and_tools() {
        let mut history = History::new(50, 10);
        history.add_message(Message {
            role: "system".to_string(),
            content: "sys".to_string(),
        });
        history.add_message(Message {
            role: "user".to_string(),
            content: "old-message".to_string(),
        });
        history.add_message(Message {
            role: "user".to_string(),
            content: "new".to_string(),
        });
        history.add_message(Message {
            role: "tool".to_string(),
            content: "pending-tool".to_string(),
        });

        let sent = history.for_provider();
        assert!(sent.iter().any(|message| message.role == "system"));
        assert!(sent.iter().all(|message| message.content != "old-message"));
        assert!(sent.iter().any(|message| message.content == "new"));
        assert!(sent.iter().any(|message| message.content == "pending-tool"));
    }
}
