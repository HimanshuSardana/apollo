//! apollo-gui: native GPUI client.
//!
//! Layout: threads sidebar (left) + dock center with one tab per open thread.
//! Each tab is a [`ChatPanel`](crate::panels::chat::ChatPanel) bound to a
//! [`Thread`](crate::thread::Thread) in [`ApolloWorkspace`](crate::workspace::ApolloWorkspace).
//! All agent logic (client, tools, models) is shared from `apollo-core`.

mod app_assets;
mod markdown;
mod panels;
mod theme;
mod thread;
mod workspace;

use app_assets::ApolloAssets;
use gpui_kit::component::Root;
use gpui_kit::AppContext as _;
use workspace::{ApolloWorkspace, ToggleSidebar};

fn main() {
    let app = gpui_kit::application().with_assets(ApolloAssets);

    app.run(move |cx| {
        gpui_kit::init(cx);
        theme::init(cx);
        cx.bind_keys(vec![gpui_kit::KeyBinding::new("ctrl-b", ToggleSidebar, None)]);

        let window_options = gpui_kit::WindowOptions {
            window_bounds: Some(gpui_kit::WindowBounds::Windowed(
                gpui_kit::Bounds::centered(
                    None,
                    gpui_kit::size(gpui_kit::px(1400.), gpui_kit::px(900.)),
                    cx,
                ),
            )),
            ..Default::default()
        };

        let (window, _) = gpui_kit::open_window(window_options, cx, |window, cx| {
            cx.new(|cx| ApolloWorkspace::new(window, cx))
        })
        .expect("failed to open apollo window");

        let window = window.downcast::<Root>().expect("kit window has a Root");
        window
            .update(cx, |_, window, _| {
                window.activate_window();
                window.set_window_title("Apollo");
            })
            .expect("failed to activate apollo window");
    });
}
