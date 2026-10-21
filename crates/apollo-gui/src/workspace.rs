//! App shell: threads sidebar (left) + dock center (one tab per thread).
//!
//! Clicking a thread opens it in a center tab (or focuses the existing tab).
//! Tabs are closable; closing only removes the tab, the thread stays listed
//! and re-clicking re-opens it.

use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dock::{
    DockArea, DockLayout, DockPlacement, DockSkin, PanelId, PanelStyle, panel_handle,
};
use gpui_kit::component::sidebar::{
    Sidebar, SidebarCollapsible, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu,
    SidebarMenuItem, SidebarToggleButton,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{Sizable as _, StyledExt as _, h_flex};
use gpui_kit::*;

use crate::panels::chat::ChatPanel;
use crate::thread::{Thread, ThreadId};

actions!(apollo, [ToggleSidebar]);

pub struct ApolloWorkspace {
    dock_area: Entity<DockArea>,
    // Retained for follow-ups (panel styling, theme switching).
    #[allow(dead_code)]
    skin: Rc<DockSkin>,
    threads: Vec<Thread>,
    open_tabs: HashMap<ThreadId, PanelId>,
    active: Option<ThreadId>,
    next_id: ThreadId,
    sidebar_collapsed: bool,
}

impl ApolloWorkspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (dock_area, skin) = DockSkin::dock_area("apollo-dock", Some(1), window, cx);
        skin.set_panel_style(PanelStyle::TabBar, cx);
        let mut this = Self {
            dock_area,
            skin,
            threads: Vec::new(),
            open_tabs: HashMap::new(),
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
        self.active = Some(id);
        if let Some(pid) = self.open_tabs.get(&id).copied() {
            self.dock_area.update(cx, |area, cx| {
                area.select_panel(pid, window, cx);
            });
            cx.notify();
            return;
        }
        let Some(thread) = self.threads.iter().find(|t| t.id == id).cloned() else {
            return;
        };
        let panel = cx.new(|cx| ChatPanel::from_thread(&thread, window, cx));
        let pid = PanelId::from(panel.entity_id());
        let handle = panel_handle(panel);

        // The first tab *defines* the center layout; later tabs append to it.
        if self.open_tabs.is_empty() {
            let layout = DockLayout::tabs().panel_view(handle, cx);
            self.dock_area.update(cx, |area, cx| {
                area.set_center(layout, window, cx);
            });
        } else {
            self.dock_area.update(cx, |area, cx| {
                area.add_panel_view(handle, DockPlacement::Center, None, window, cx);
                area.select_panel(pid, window, cx);
            });
        }
        self.open_tabs.insert(id, pid);
        cx.notify();
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }
}

impl Render for ApolloWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                SidebarHeader::new()
                    .child(div().font_bold().child("apollo"))
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
            )
            .child(SidebarGroup::new("Threads").child(menu))
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

        h_flex()
            .size_full()
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.toggle_sidebar(cx);
            }))
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .min_h_0()
                    .child(self.dock_area.clone()),
            )
    }
}
