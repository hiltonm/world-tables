
#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;

use world_tables_gui::App;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[arg(short, long, default_value_t = String::from("127.0.0.1:3000"))]
    address: String,
}

impl Cli {
    fn execute(self) -> Result<SocketAddr> {
        Ok(self.address.parse()?)
    }
}

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    let addr = Cli::parse().execute().expect("cli: failed execution");

    let native_options = eframe::NativeOptions {
        min_window_size: Some(egui::vec2(640.0, 480.0)),
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        maximized: true,
        ..Default::default()
    };

    eframe::run_native(
        "World Tables",
        native_options,
        Box::new(move |cc| Box::new(App::new(cc, addr))),
    )
}

// when compiling to web using trunk.
#[cfg(target_arch = "wasm32")]
fn main() {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::start_web(
            "the_canvas_id", // hardcode it
            web_options,
            Box::new(|cc| Box::new(eframe_template::TemplateApp::new(cc))),
        )
        .await
        .expect("failed to start eframe");
    });
}
