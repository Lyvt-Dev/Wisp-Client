//! The Microsoft sign-in window: Microsoft's own sign-in page in a small window of the launcher
//! (WebView2), as the official launcher and other launchers sign in. It runs as a child process
//! of the launcher (`wisp-launcher sign-in-window`), so its event loop never meets the
//! dashboard's. When the page goes to the redirect address, the process prints that address
//! (holding the one-time code) on stdout and exits; closing the window exits with code 2.
use std::path::PathBuf;

/// Shows `url` until the page goes to an address starting with `redirect`; never returns.
pub fn run(url: &str, redirect: &str, data: PathBuf) -> ! {
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoop};
    use tao::window::WindowBuilder;
    use wry::{WebContext, WebViewBuilder};

    let fail = |e: &dyn std::fmt::Display| -> ! {
        eprintln!("{e}");
        std::process::exit(3)
    };
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(crate::i18n::tr("Sign in with Microsoft · Wisp"))
        .with_inner_size(tao::dpi::LogicalSize::new(500.0, 680.0))
        .with_resizable(true)
        .build(&event_loop)
        .unwrap_or_else(|e| fail(&e));
    let _ = std::fs::create_dir_all(&data);
    let mut context = WebContext::new(Some(data));
    let redirect = redirect.to_owned();
    let _webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(url)
        .with_navigation_handler(move |to: String| {
            if to.starts_with(&redirect) {
                use std::io::Write;
                let mut out = std::io::stdout();
                let _ = writeln!(out, "{to}");
                let _ = out.flush();
                std::process::exit(0);
            }
            true
        })
        .build(&window)
        .unwrap_or_else(|e| fail(&e));
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            std::process::exit(2);
        }
    })
}
