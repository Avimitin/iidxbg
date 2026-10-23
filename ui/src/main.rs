#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bilibili;
mod editor;
mod gallery;
mod preview;
mod store;
mod theme;
mod widgets;

use gpui_kit::assets::AllAssets;

fn main() {
    let path = match app::config_path_from_args() {
        Ok(path) => Some(path),
        Err(error) => {
            let app = gpui_kit::application().with_assets(AllAssets);
            app.run(move |cx| {
                gpui_kit::init(cx);
                app::install_theme(cx);
                app::open_window_with_config(Err(error), None, cx).ok();
            });
            return;
        }
    };
    let path = path.expect("config path");
    let store = store::ConfigStore::load(path.clone());
    let app = gpui_kit::application().with_assets(AllAssets);
    app.run(move |cx| {
        gpui_kit::init(cx);
        app::install_theme(cx);
        if let Err(error) = app::open_window_with_config(store, Some(path), cx) {
            eprintln!("{error}");
        }
    });
}
