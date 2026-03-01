use crate::providers::traits::Message;

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
                if pos > 0 {
                    self.messages.remove(pos);
                }
            } else {
                break;
            }
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}
