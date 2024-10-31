use mail_parser::{Message, MessageParser};

pub struct OwnedEmail {
    message: String,
}


impl OwnedEmail {
    pub fn new(message: String) -> Self {
        Self {
            message
        }
    }

    pub fn raw_str(&self) -> &str {
        &self.message
    }

    pub fn get_text(self) -> String {
        self.message
    }

    pub fn parse(&self) -> Message {
        MessageParser::new().parse(&self.message).unwrap()
    }
}