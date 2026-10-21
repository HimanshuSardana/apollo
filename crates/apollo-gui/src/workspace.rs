//! App shell: threads sidebar (left) + tab strip over the active thread.
//!
//! The tab strip is hand-rolled rather than delegated to the dock's
//! [`TabBar`]: that widget pins any trailing controls to the far right of the
//! strip, which would strand the close/new buttons a thousand pixels from the
//! tabs they act on. Thread panels are kept alive in `panels`, so switching
//! tabs preserves scroll position and composer state.
//!
//! Closing a tab drops its panel; reopening rebuilds it from the [`Thread`]'s
//! messages, so nothing is lost that the thread itself holds.

use std::collections::HashMap;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::sidebar::{
    Sidebar, SidebarCollapsible, SidebarFooter, SidebarHeader, SidebarMenu, SidebarMenuItem,
    SidebarToggleButton,
};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex,
};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::panels::chat::ChatPanel;
use crate::thread::{Thread, ThreadId};

actions!(apollo, [ToggleSidebar, NewThread]);

/// Height of the thread tab strip.
const TAB_STRIP_H: f32 = 34.;

pub struct ApolloWorkspace {
    threads: Vec<Thread>,
    /// One live panel per open tab. Dropping an entry closes its tab.
    panels: HashMap<ThreadId, Entity<ChatPanel>>,
    active: Option<ThreadId>,
    next_id: ThreadId,
    sidebar_collapsed: bool,
}

impl ApolloWorkspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            threads: Vec::new(),
            panels: HashMap::new(),
            active: None,
            next_id: 0,
            sidebar_collapsed: false,
        };
        this.new_thread(window, cx);
        this
    }

    fn new_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.next_id += 1;
        let thread = Thread::new(self.next_id, apollo_core::models::DEFAULT_MODEL);
        let id = thread.id;
        self.threads.push(thread);
        self.open_thread(id, window, cx);
    }

    fn open_thread(&mut self, id: ThreadId, window: &mut Window, cx: &mut Context<Self>) {
        if !self.panels.contains_key(&id) {
            let Some(thread) = self.threads.iter().find(|t| t.id == id).cloned() else {
                return;
            };
            let panel = cx.new(|cx| ChatPanel::from_thread(&thread, window, cx));
            self.panels.insert(id, panel);
        }
        self.active = Some(id);
        cx.notify();
    }

    /// Close the active tab. The thread stays in the sidebar, and the
    /// neighbouring tab takes over so the strip is never left empty while
    /// another tab exists.
    fn close_tab(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.panels.remove(&id);
        if self.active == Some(id) {
            self.active = self.open_order().last().copied();
        }
        cx.notify();
    }

    /// Open tabs in sidebar order, which is also the order they were created.
    fn open_order(&self) -> Vec<ThreadId> {
        self.threads
            .iter()
            .map(|thread| thread.id)
            .filter(|id| self.panels.contains_key(id))
            .collect()
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }
}

impl ApolloWorkspace {
    fn render_tab_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let active = self.active;

        let tabs = self.open_order().into_iter().map(|id| {
            let Some(thread) = self.threads.iter().find(|t| t.id == id) else {
                return div().into_any_element();
            };
            let title = thread.title.clone();
            let selected = Some(id) == active;
            div()
                .id(("tab", id))
                .flex_shrink_0()
                .h_full()
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .rounded_t(px(6.))
                .text_sm()
                .whitespace_nowrap()
                .when(selected, |this| {
                    this.bg(theme.background).text_color(theme.foreground)
                })
                .when(!selected, |this| {
                    this.text_color(theme.muted_foreground)
                        .hover(|this| this.text_color(theme.foreground))
                })
                .child(title)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.open_thread(id, window, cx);
                }))
                .into_any_element()
        });

        h_flex()
            .h(px(TAB_STRIP_H))
            .flex_shrink_0()
            .w_full()
            .items_center()
            .gap_1()
            .px_2()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.tokens.tab_bar)
            .children(tabs)
            .when_some(active, |this, id| {
                this.child(
                    Button::new("tab-close")
                        .icon(IconName::X)
                        .xsmall()
                        .ghost()
                        .tooltip("Close tab")
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.close_tab(id, cx);
                        })),
                )
            })
            .child(
                Button::new("tab-new-thread")
                    .icon(IconName::Plus)
                    .xsmall()
                    .ghost()
                    .tooltip("New thread")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.new_thread(window, cx);
                    })),
            )
    }
}

impl Render for ApolloWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let active = self.active;
        let collapsed = self.sidebar_collapsed;

        let mut menu = SidebarMenu::new();
        for thread in &self.threads {
            let id = thread.id;
            menu = menu.child(
                SidebarMenuItem::new(thread.title.clone())
                    .icon(IconName::MessageCircle)
                    .active(Some(id) == active)
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.open_thread(id, window, cx);
                    })),
            );
        }

        let sidebar = Sidebar::new("threads-sidebar")
            .flex_shrink_0()
            .collapsible(SidebarCollapsible::Icon)
            .collapsed(collapsed)
            .w(px(260.))
            .header(
                SidebarHeader::new().child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .when(!collapsed, |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(theme.sidebar_foreground.opacity(0.7))
                                    .child("Threads"),
                            )
                        })
                        .child(
                            Button::new("new-thread")
                                .icon(IconName::Plus)
                                .xsmall()
                                .ghost()
                                .tooltip("New thread")
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.new_thread(window, cx);
                                })),
                        ),
                ),
            )
            .child(menu)
            .footer(
                SidebarFooter::new().child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            SidebarToggleButton::new()
                                .collapsed(collapsed)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.sidebar_collapsed = !this.sidebar_collapsed;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div().text_xs().child(
                                apollo_core::models::display_name(
                                    apollo_core::models::DEFAULT_MODEL,
                                ),
                            ),
                        ),
                ),
            );

        let body = match active.and_then(|id| self.panels.get(&id)) {
            Some(panel) => AnyView::from(panel.clone()).into_any_element(),
            None => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(theme.muted_foreground)
                .child(Icon::new(IconName::MessageCircle).size(px(20.)))
                .child(div().text_sm().child("No open threads"))
                .child(div().text_xs().child("Pick one in the sidebar to open it"))
                .into_any_element(),
        };

        h_flex()
            .size_full()
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.toggle_sidebar(cx);
            }))
            .on_action(cx.listener(|this, _: &NewThread, window, cx| {
                this.new_thread(window, cx);
            }))
            .child(sidebar)
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .min_h_0()
                    .bg(theme.background)
                    .child(self.render_tab_strip(cx))
                    .child(div().flex_1().min_h_0().w_full().child(body)),
            )
    }
}
