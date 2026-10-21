//! One dock tab per agent thread: message history plus the composer.
//! Streaming + tool-call rendering land here next; v1 shows the thread
//! snapshot so the sidebar/dock shell is fully navigable.

use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::thread::{Thread, ThreadId};

pub struct ChatPanel {
    focus_handle: FocusHandle,
    // Unread until tab-close sync + streaming land (next steps).
    #[allow(dead_code)]
    thread_id: ThreadId,
    title: String,
    model: String,
    message_count: usize,
}

impl ChatPanel {
    pub fn from_thread(thread: &Thread, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            thread_id: thread.id,
            title: thread.title.clone(),
            model: thread.model.clone(),
            message_count: thread.messages.len(),
        }
    }

}

impl EventEmitter<PanelEvent> for ChatPanel {}

impl Focusable for ChatPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for ChatPanel {
    fn panel_name(&self) -> &'static str {
        "ChatPanel"
    }

    fn closable(&self, _: &App) -> bool {
        true
    }
}

impl Panel for ChatPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some(self.title.clone().into())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.title.clone()
    }
}

impl Render for ChatPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_sm().child(self.model.clone()))
                    .child(
                        div()
                            .text_xs()
                            .child(format!("{} messages", self.message_count)),
                    ),
            )
            .child(div().flex_1().child(
                "Streaming chat lands next — threads open here, talk in the CLI for now.",
            ))
    }
}
