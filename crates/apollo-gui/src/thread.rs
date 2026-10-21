//! Agent thread model shared by the sidebar and the chat tabs.
//!
//! A [`Thread`] is the GUI analogue of one CLI session: a title, a model,
//! and the apollo-core message history. The workspace owns the list; each
//! open thread also gets a dock tab rendering a snapshot.

use apollo_core::client::ChatMessage;

pub type ThreadId = usize;

#[derive(Debug, Clone)]
pub struct Thread {
    pub id: ThreadId,
    pub title: String,
    pub model: String,
    pub messages: Vec<ChatMessage>,
}

impl Thread {
    pub fn new(id: ThreadId, model: impl Into<String>) -> Self {
        Self {
            id,
            title: format!("Thread {id}"),
            model: model.into(),
            messages: Vec::new(),
        }
    }
}
