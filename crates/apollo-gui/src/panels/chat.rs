//! One dock tab per agent thread: message list + composer.
//!
//! The composer is a single bordered box with the text field on top and a
//! toolbar row below it: `+`, model chip, effort chip, spacer, send button.
//! Model and effort chips open inline menus rendered inside the composer
//! (not OS popovers) so they can carry a search field and provider groups.

use std::collections::{HashMap, HashSet};

use apollo_core::client::{ChatMessage, Client, ToolCall};
use apollo_core::{models, tools};
use gpui_kit::assets::IconName;
use gpui_kit::component::collapsible::Collapsible;
use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent};
use gpui_kit::component::input::{
    Editor, EditorState, Input, InputEvent, InputState, Textarea, TextareaState,
};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::markdown::{self, Segment};
use crate::thread::{Thread, ThreadId};

const MAX_TOOL_STEPS: usize = 24;
const COMPOSER_MAX_H: f32 = 200.;
const MENU_W: f32 = 420.;
/// Lines shown per tool result before truncating with a "more lines" note.
const TOOL_PREVIEW_LINES: usize = 40;
/// Lines shown per assistant code fence before truncating.
const CODE_PREVIEW_LINES: usize = 150;
/// Visible editor rows before the block scrolls internally.
const MAX_CODE_ROWS: usize = 24;
/// Accent for the send affordance. The default dark theme's `primary` is
/// near-white, so the one primary action in the UI carries its own blue.
const SEND_BLUE: u32 = 0x2563eb;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    None,
    Model,
    Effort,
}

/// Render state for one message, built on arrival (editors need a window).
#[derive(Default)]
struct MsgView {
    /// Highlighted editors in segment order: one per assistant code fence,
    /// or a single capped output editor for tool results.
    editors: Vec<Entity<EditorState>>,
    /// Language label per editor, parallel to `editors`.
    code_langs: Vec<String>,
    /// Line count per editor, parallel to `editors` (for explicit height).
    code_rows: Vec<usize>,
    /// Tool calls of assistant messages, for collapsible headers.
    calls: Vec<ToolCall>,
}

fn tool_icon(name: &str) -> IconName {
    match name {
        "read" => IconName::FileText,
        "write" => IconName::SquarePen,
        "edit" => IconName::Pencil,
        "bash" => IconName::Terminal,
        _ => IconName::Wrench,
    }
}

pub struct ChatPanel {
    focus_handle: FocusHandle,
    #[allow(dead_code)]
    thread_id: ThreadId,
    title: String,
    model: String,
    effort: Option<String>,
    messages: Vec<ChatMessage>,
    /// Render state parallel to `messages` (editors must be created with a
    /// window, so they are built on arrival, not during render).
    msg_views: Vec<MsgView>,
    /// Open tool-call collapsibles as `(message_idx, call_idx)`.
    open_calls: HashSet<(usize, usize)>,
    input: Entity<TextareaState>,
    model_query: Entity<InputState>,
    menu: Menu,
    busy: bool,
    error: Option<String>,
    _subs: Vec<Subscription>,
}

impl ChatPanel {
    pub fn from_thread(thread: &Thread, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("What would you like to know?")
                .submit_on_enter(true)
        });
        let model_query = cx.new(|cx| InputState::new(window, cx).placeholder("Search models…"));

        let mut this = Self {
            focus_handle: cx.focus_handle(),
            thread_id: thread.id,
            title: thread.title.clone(),
            model: thread.model.clone(),
            effort: None,
            messages: Vec::new(),
            msg_views: Vec::new(),
            open_calls: HashSet::new(),
            input,
            model_query,
            menu: Menu::None,
            busy: false,
            error: None,
            _subs: Vec::new(),
        };

        // Enter in the composer sends; Shift+Enter inserts a newline.
        let sub = cx.subscribe_in(&this.input, window, |this, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { shift: false, .. }) {
                this.send(window, cx);
            }
        });
        this._subs.push(sub);

        this
    }

    fn set_model(&mut self, id: &str, cx: &mut Context<Self>) {
        self.model = id.to_string();
        // Keep effort valid for the new model, mirroring the CLI.
        if let Some(e) = self.effort.clone() {
            if !models::efforts_for(&self.model).contains(&e.as_str()) {
                self.effort = None;
            }
        }
        self.menu = Menu::None;
        cx.notify();
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let text = self.input.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.messages.push(ChatMessage::user(&text));
        self.msg_views.push(MsgView::default());
        self.busy = true;
        self.error = None;
        self.menu = Menu::None;
        cx.notify();

        let model = self.model.clone();
        let effort = self.effort.clone();
        let history = self.messages.clone();

        // The agent turn needs a Tokio reactor for reqwest, which the GPUI
        // executor does not provide. Run it on our own runtime and stream
        // progress back over a channel.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<UiMsg>();
        runtime().spawn(async move {
            run_turn(model, effort, history, tx).await;
        });

        cx.spawn_in(window, async move |this, cx| {
            while let Some(msg) = rx.recv().await {
                let applied = this.update_in(cx, |this, window, cx| {
                    match msg {
                        UiMsg::Assistant(message) => this.push_message(message, window, cx),
                        UiMsg::Tool(message) => this.push_message(message, window, cx),
                        UiMsg::Error(err) => this.error = Some(err),
                        UiMsg::Done => {}
                    }
                    cx.notify();
                });
                if applied.is_err() {
                    return;
                }
            }
            let _ = this.update_in(cx, |this, _, cx| {
                this.busy = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Push an incoming message and build its render state (code editors need
    /// a window, so this happens on arrival, never during render).
    fn push_message(
        &mut self,
        message: ChatMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.messages.push(message);
        let idx = self.messages.len() - 1;
        let view = self.build_view(idx, window, cx);
        self.msg_views.push(view);
    }

    fn build_view(
        &self,
        idx: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> MsgView {
        let mut view = MsgView::default();
        let msg = &self.messages[idx];
        if msg.role == "assistant" {
            if let Some(calls) = &msg.tool_calls {
                view.calls = calls.clone();
            }
            let body = msg.content.clone().unwrap_or_default();
            for seg in markdown::segments(&body) {
                if let Segment::Code(block) = seg {
                    let (code, hidden) = markdown::cap_lines(&block.code, CODE_PREVIEW_LINES);
                    let code = match hidden {
                        Some(n) => format!("{code}\n… {n} more lines"),
                        None => code,
                    };
                    view.code_langs.push(block.lang.clone());
                    view.code_rows.push(block.code.lines().count().max(1));
                    view.editors.push(cx.new(|cx| {
                        EditorState::new(window, cx)
                            .language(block.lang.clone())
                            .default_value(code)
                    }));
                }
            }
        } else if msg.role == "tool" {
            let lang = self.tool_lang_for(idx).to_string();
            let body = msg.content.clone().unwrap_or_default();
            let (code, hidden) = markdown::cap_lines(&body, TOOL_PREVIEW_LINES);
            let code = match hidden {
                Some(n) => format!("{code}\n… {n} more lines"),
                None => code,
            };
            view.code_langs.push(lang.clone());
            view.code_rows.push(code.lines().count().max(1));
            view.editors.push(cx.new(|cx| {
                EditorState::new(window, cx)
                    .language(lang)
                    .default_value(code)
            }));
        }
        view
    }

    /// Guess an editor language for a tool result by pairing it with its call:
    /// the Nth tool message after an assistant-with-calls belongs to the Nth call.
    fn tool_lang_for(&self, tool_idx: usize) -> &'static str {
        let mut calls: Option<&Vec<ToolCall>> = None;
        let mut since = 0usize;
        for msg in &self.messages[..tool_idx] {
            if msg.role == "assistant" && msg.tool_calls.is_some() {
                calls = msg.tool_calls.as_ref();
                since = 0;
            } else if msg.role == "tool" {
                since += 1;
            }
        }
        let call = calls.and_then(|c| c.get(since));
        let Some(call) = call else {
            return "text";
        };
        if call.function.name == "bash" {
            return "text";
        }
        let args: serde_json::Value =
            serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::Value::Null);
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        markdown::editor_lang(ext)
    }

    /// Forward scan for the tool result belonging to `call_id`.
    fn find_tool_result(&self, from: usize, call_id: &str) -> Option<usize> {
        self.messages.iter().enumerate().skip(from).find_map(|(i, m)| {
            if m.role == "tool" && m.tool_call_id.as_deref() == Some(call_id) {
                Some(i)
            } else {
                None
            }
        })
    }

    /// Assistant message: markdown prose, highlighted code fences, then one
    /// collapsible per tool call (each rendering its result inline).
    fn render_assistant(&self, idx: usize, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let msg = &self.messages[idx];
        let view = &self.msg_views[idx];
        let tid = self.thread_id;

        let mut col = v_flex().w_full().gap_2().child(
            h_flex()
                .gap_1()
                .items_center()
                .child(
                    Icon::new(IconName::Sparkles)
                        .xsmall()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("apollo"),
                ),
        );

        let body = msg.content.clone().unwrap_or_default();
        let mut fence_no = 0usize;
        for (si, seg) in markdown::segments(&body).into_iter().enumerate() {
            match seg {
                Segment::Prose(text) => {
                    col = col.child(TextView::markdown(
                        SharedString::from(format!("t{tid}-m{idx}-s{si}")),
                        text,
                    ));
                }
                Segment::Code(_) => {
                    if let (Some(editor), Some(lang), Some(rows)) = (
                        view.editors.get(fence_no),
                        view.code_langs.get(fence_no),
                        view.code_rows.get(fence_no),
                    ) {
                        col = col.child(self.code_block(lang, *rows, editor, cx));
                    }
                    fence_no += 1;
                }
            }
        }

        if let Some(calls) = &msg.tool_calls {
            for (ci, _) in calls.iter().enumerate() {
                col = col.child(self.render_tool_call(idx, ci, cx));
            }
        }

        col
    }

    /// One tool call: collapsed header row (`icon + summary`, like the CLI's
    /// `→ read src/config.rs`), expandable body with args + capped output.
    fn render_tool_call(
        &self,
        msg_idx: usize,
        call_idx: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let call = self.msg_views[msg_idx].calls[call_idx].clone();
        let open = self.open_calls.contains(&(msg_idx, call_idx));
        let summary = tools::summary(&call.function.name, &call.function.arguments);
        let result_idx = self.find_tool_result(msg_idx + 1, &call.id);

        let header = h_flex()
            .id(SharedString::from(format!("tool-{msg_idx}-{call_idx}")))
            .w_full()
            .items_center()
            .gap_2()
            .px_2()
            .py_1p5()
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(theme.muted))
            .child(
                Icon::new(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .xsmall()
                .text_color(theme.muted_foreground),
            )
            .child(
                Icon::new(tool_icon(&call.function.name))
                    .xsmall()
                    .text_color(theme.muted_foreground),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child(format!("{} {}", call.function.name, summary)),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                let key = (msg_idx, call_idx);
                if this.open_calls.contains(&key) {
                    this.open_calls.remove(&key);
                } else {
                    this.open_calls.insert(key);
                }
                cx.notify();
            }));

        let mut body = v_flex().w_full().gap_2();
        if open {
            if let Some(ri) = result_idx {
                if let (Some(editor), Some(lang), Some(rows)) = (
                    self.msg_views[ri].editors.first(),
                    self.msg_views[ri].code_langs.first(),
                    self.msg_views[ri].code_rows.first(),
                ) {
                    body = body.child(self.code_block(lang, *rows, editor, cx));
                }
            } else {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("running…"),
                );
            }
        }

        v_flex()
            .w_full()
            .child(header)
            .child(
                Collapsible::new()
                    .open(open)
                    .content(div().w_full().pl_6().pr_1().pb_1().child(body)),
            )
    }

    /// Tool result with no owning call (shouldn't happen; kept as fallback).
    fn render_orphan_tool(&self, idx: usize, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let body = self.messages[idx].content.clone().unwrap_or_default();
        div()
            .w_full()
            .rounded_md()
            .bg(theme.muted)
            .px_3()
            .py_2()
            .font_family(theme.mono_font_family.clone())
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(truncate(&body, 400))
    }

    /// Read-only highlighted editor with a language label header.
    ///
    /// The editor gets an explicit height from its line count: without one it
    /// measures short and the last line clips. Taller than the cap scrolls
    /// inside the editor instead of stretching the thread.
    fn code_block(
        &self,
        lang: &str,
        rows: usize,
        editor: &Entity<EditorState>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        // Must match the editor's own row height (mono size × its 1.5 factor).
        let row_h = theme.mono_font_size * 1.5;
        let height = row_h * rows.min(MAX_CODE_ROWS) as f32 + px(8.);
        v_flex()
            .w_full()
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .px_2()
                    .py_1()
                    .bg(theme.muted)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(lang.to_string()),
            )
            .child(
                Editor::new(editor)
                    .readonly(true)
                    .appearance(false)
                    .bordered(false)
                    .h(height),
            )
    }

    fn render_messages(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut list = v_flex().w_full().gap_3();

        if self.messages.is_empty() {
            list = list.child(
                div()
                    .py_8()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(format!(
                        "Start a conversation with {}.",
                        models::display_name(&self.model)
                    )),
            );
        }

        // Tool results render inside their call's collapsible; collect the
        // owned ones first so the main loop can skip them.
        let mut owned_results: HashMap<usize, (usize, usize)> = HashMap::new();
        for (ai, msg) in self.messages.iter().enumerate() {
            if msg.role == "assistant" {
                if let Some(calls) = &msg.tool_calls {
                    for (ci, call) in calls.iter().enumerate() {
                        if let Some(ri) = self.find_tool_result(ai + 1, &call.id) {
                            owned_results.insert(ri, (ai, ci));
                        }
                    }
                }
            }
        }

        for (idx, msg) in self.messages.iter().enumerate() {
            let body = msg.content.clone().unwrap_or_default();
            match msg.role.as_str() {
                "user" => {
                    list = list.child(
                        v_flex()
                            .w_full()
                            .items_end()
                            .child(
                                div()
                                    .max_w(px(560.))
                                    .rounded_lg()
                                    .bg(theme.secondary)
                                    .px_3()
                                    .py_2()
                                    .text_sm()
                                    .text_color(theme.secondary_foreground)
                                    .child(body),
                            ),
                    );
                }
                "assistant" => {
                    list = list.child(self.render_assistant(idx, cx));
                }
                "tool" => {
                    if owned_results.contains_key(&idx) {
                        continue;
                    }
                    list = list.child(self.render_orphan_tool(idx, cx));
                }
                _ => {}
            }
        }

        if self.busy {
            list = list.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Icon::new(IconName::Sparkles)
                            .xsmall()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("thinking…"),
                    ),
            );
        }

        if let Some(err) = &self.error {
            list = list.child(
                div()
                    .w_full()
                    .rounded_md()
                    .bg(theme.danger.opacity(0.12))
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(theme.danger)
                    .child(err.clone()),
            );
        }

        v_flex()
            .id("messages")
            .flex_1()
            .min_h_0()
            .w_full()
            .items_center()
            .overflow_y_scroll()
            .px_5()
            .py_4()
            .child(div().w_full().max_w(px(760.)).child(list))
    }

    fn chip(
        &self,
        id: &'static str,
        icon: IconName,
        label: String,
        active: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let color = if active {
            theme.foreground
        } else {
            theme.muted_foreground
        };
        h_flex()
            .id(id)
            .gap_1()
            .items_center()
            .px_2()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .text_xs()
            .text_color(color)
            .hover(|s| s.bg(theme.muted))
            .child(Icon::new(icon).xsmall().text_color(color))
            .child(div().child(label))
            .child(Icon::new(IconName::ChevronDown).xsmall().text_color(color))
            .on_click(on_click)
    }

    fn render_model_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let query = self.model_query.read(cx).value().to_lowercase();
        let current = self.model.clone();

        let mut list = v_flex().w_full().gap_1().p_1();
        for provider in models::providers() {
            let matches: Vec<&models::ModelSpec> = models::models_for(provider)
                .into_iter()
                .filter(|m| {
                    query.is_empty()
                        || m.name.to_lowercase().contains(&query)
                        || m.id.to_lowercase().contains(&query)
                })
                .collect();
            if matches.is_empty() {
                continue;
            }
            list = list.child(
                div()
                    .px_2()
                    .pt_2()
                    .pb_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(provider),
            );
            for model in matches {
                let id = model.id;
                let is_current = id == current;
                let name = model.name;
                let efforts = model.efforts.len();
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("model-{id}")))
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1p5()
                        .rounded_md()
                        .cursor_pointer()
                        .text_sm()
                        .when(is_current, |s| s.bg(theme.list_active))
                        .hover(|s| s.bg(theme.muted))
                        .child(
                            Icon::new(IconName::Bot)
                                .xsmall()
                                .text_color(theme.muted_foreground),
                        )
                        .child(div().flex_1().child(name.to_string()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("•".repeat(efforts.min(4))),
                        )
                        .when(is_current, |s| {
                            s.child(
                                Icon::new(IconName::Check)
                                    .xsmall()
                                    .text_color(theme.foreground),
                            )
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_model(id, cx);
                        })),
                );
            }
        }

        v_flex()
            .absolute()
            .bottom(px(52.))
            .left(px(0.))
            .w(px(MENU_W))
            .max_h(px(360.))
            .overflow_hidden()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_lg()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        Icon::new(IconName::Search)
                            .xsmall()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.model_query).appearance(false)),
                    )
                    .child(
                        div()
                            .id("close-model-menu")
                            .cursor_pointer()
                            .child(
                                Icon::new(IconName::X)
                                    .xsmall()
                                    .text_color(theme.muted_foreground),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.menu = Menu::None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .id("model-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
    }

    fn render_effort_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut list = v_flex().w_full().gap_1().p_1();
        let levels = models::efforts_for(&self.model);
        let current = self.effort.clone();

        // `auto` means "whatever the model defaults to".
        let mut options: Vec<(String, Option<String>)> =
            vec![("auto".to_string(), None)];
        for level in levels {
            options.push(((*level).to_string(), Some((*level).to_string())));
        }

        for (label, value) in options {
            let selected = current == value;
            let value_for_click = value.clone();
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("effort-{label}")))
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1p5()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .when(selected, |s| s.bg(theme.list_active))
                    .hover(|s| s.bg(theme.muted))
                    .child(
                        Icon::new(IconName::Zap)
                            .xsmall()
                            .text_color(theme.muted_foreground),
                    )
                    .child(div().flex_1().child(label))
                    .when(selected, |s| {
                        s.child(
                            Icon::new(IconName::Check)
                                .xsmall()
                                .text_color(theme.foreground),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.effort = value_for_click.clone();
                        this.menu = Menu::None;
                        cx.notify();
                    })),
            );
        }

        v_flex()
            .absolute()
            .bottom(px(52.))
            .left(px(0.))
            .w(px(200.))
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_lg()
            .child(list)
    }

    fn render_composer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        // Copy the palette out before the mutable borrows below (top-level menus).
        let t = cx.theme();
        let (border, popover, muted, muted_fg) =
            (t.border, t.popover, t.muted, t.muted_foreground);
        let busy = self.busy;
        let model_label = models::display_name(&self.model).to_string();
        let effort_label = self.effort.clone().unwrap_or_else(|| "auto".to_string());
        let menu = self.menu;

        let menu_element = match menu {
            Menu::Model => Some(self.render_model_menu(cx).into_any_element()),
            Menu::Effort => Some(self.render_effort_menu(cx).into_any_element()),
            Menu::None => None,
        };

        let send_button = div()
            .id("send")
            .flex()
            .items_center()
            .justify_center()
            .size_8()
            .flex_shrink_0()
            .rounded_md()
            .bg(if busy { muted } else { Hsla::from(rgb(SEND_BLUE)) })
            .when(!busy, |s| {
                s.cursor_pointer()
                    .hover(|s| s.bg(rgb(0x1d4ed8)))
            })
            .child(
                Icon::new(IconName::CornerDownLeft)
                    .xsmall()
                    .text_color(if busy {
                        muted_fg
                    } else {
                        Hsla::from(rgb(0xffffff))
                    }),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.send(window, cx);
            }));

        v_flex()
            .relative()
            .w_full()
            .max_w(px(760.))
            .rounded_lg()
            .border_1()
            .border_color(border)
            .bg(popover)
            .p_2()
            .gap_2()
            .child(
                v_flex()
                    .id("composer-input")
                    .w_full()
                    .max_h(px(COMPOSER_MAX_H))
                    .overflow_y_scroll()
                    .px_1()
                    .child(Textarea::new(&self.input).appearance(false)),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id("attach")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_7()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(muted))
                            .child(
                                Icon::new(IconName::Plus)
                                    .xsmall()
                                    .text_color(muted_fg),
                            ),
                    )
                    .child(self.chip(
                        "model-chip",
                        IconName::Bot,
                        model_label,
                        menu == Menu::Model,
                        cx.listener(|this, _, window, cx| {
                            this.menu = if this.menu == Menu::Model {
                                Menu::None
                            } else {
                                Menu::Model
                            };
                            if this.menu == Menu::Model {
                                this.model_query
                                    .update(cx, |state, cx| state.set_value("", window, cx));
                            }
                            cx.notify();
                        }),
                        cx,
                    ))
                    .child(self.chip(
                        "effort-chip",
                        IconName::Zap,
                        effort_label,
                        menu == Menu::Effort,
                        cx.listener(|this, _, _, cx| {
                            this.menu = if this.menu == Menu::Effort {
                                Menu::None
                            } else {
                                Menu::Effort
                            };
                            cx.notify();
                        }),
                        cx,
                    ))
                    .child(div().flex_1())
                    .child(send_button),
            )
            .children(menu_element)
    }
}

/// Messages streamed from the agent turn back to the UI.
enum UiMsg {
    Assistant(ChatMessage),
    Tool(ChatMessage),
    Error(String),
    Done,
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to start tokio runtime")
    })
}

/// One agent turn: stream an assistant reply, run any tool calls, loop until
/// the model answers without tools (bounded by `MAX_TOOL_STEPS`).
async fn run_turn(
    model: String,
    effort: Option<String>,
    mut history: Vec<ChatMessage>,
    tx: tokio::sync::mpsc::UnboundedSender<UiMsg>,
) {
    let mut client = match Client::new() {
        Ok(client) => client,
        Err(e) => {
            let _ = tx.send(UiMsg::Error(e.to_string()));
            let _ = tx.send(UiMsg::Done);
            return;
        }
    };
    client.set_model(&model);
    client.set_effort(effort.as_deref());
    let defs = tools::definitions();

    for _ in 0..MAX_TOOL_STEPS {
        let completion = match client.complete_stream(&history, Some(&defs), |_| {}).await {
            Ok(completion) => completion,
            Err(e) => {
                let _ = tx.send(UiMsg::Error(e.to_string()));
                break;
            }
        };

        if completion.tool_calls.is_empty() {
            let message = ChatMessage::assistant(completion.content, completion.reasoning);
            history.push(message.clone());
            let _ = tx.send(UiMsg::Assistant(message));
            break;
        }

        let calls = completion.tool_calls.clone();
        let message =
            ChatMessage::assistant_tool_calls(completion.content, completion.reasoning, calls.clone());
        history.push(message.clone());
        let _ = tx.send(UiMsg::Assistant(message));

        for call in &calls {
            let name = call.function.name.clone();
            let args = call.function.arguments.clone();
            let outcome = match tokio::task::spawn_blocking(move || tools::execute(&name, &args)).await
            {
                Ok(outcome) => outcome,
                Err(e) => {
                    let _ = tx.send(UiMsg::Error(format!("tool task failed: {e}")));
                    continue;
                }
            };
            let result = ChatMessage::tool_result(&call.id, outcome.model_text);
            history.push(result.clone());
            let _ = tx.send(UiMsg::Tool(result));
        }
    }

    let _ = tx.send(UiMsg::Done);
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max).collect();
    format!("{head}…")
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .size_full()
            .min_h_0()
            .bg(theme.background)
            .child(self.render_messages(cx))
            .child(
                v_flex().w_full().px_5().pb_4().items_center().child(self.render_composer(cx)),
            )
    }
}
