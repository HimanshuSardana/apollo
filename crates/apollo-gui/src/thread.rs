//! Agent thread model shared by the sidebar and the chat tabs.
//!
//! A [`Thread`] is the GUI analogue of one CLI session: a title and a model.
//! The workspace owns the list; the open tab's [`ChatPanel`] owns the live
//! message history for that thread.
//!
//! [`ChatPanel`]: crate::panels::chat::ChatPanel

pub type ThreadId = usize;

#[derive(Debug, Clone)]
pub struct Thread {
    pub id: ThreadId,
    pub title: String,
    pub model: String,
}

impl Thread {
    pub fn new(id: ThreadId, model: impl Into<String>) -> Self {
        Self {
            id,
            title: format!("Thread {id}"),
            model: model.into(),
        }
    }
}
