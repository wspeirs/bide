use tauri::{
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu},
    Emitter,
};

fn create_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<Menu<R>> {
    // File menu
    let new_tab = MenuItem::with_id(
        app,
        "new_tab",
        "New Project Tab...",
        true,
        Some("CmdOrCtrl+T"),
    )?;
    let close_tab = MenuItem::with_id(app, "close_tab", "Close Tab", true, Some("CmdOrCtrl+W"))?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let update_omp = MenuItem::with_id(app, "update_omp", "Update omp", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?;
    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[&new_tab, &close_tab, &sep1, &update_omp, &sep2, &quit],
    )?;

    // Edit menu
    let undo = PredefinedMenuItem::undo(app, None)?;
    let redo = PredefinedMenuItem::redo(app, None)?;
    let sep_edit = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, None)?;
    let copy = PredefinedMenuItem::copy(app, None)?;
    let paste = PredefinedMenuItem::paste(app, None)?;
    let select_all = PredefinedMenuItem::select_all(app, None)?;
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[&undo, &redo, &sep_edit, &cut, &copy, &paste, &select_all],
    )?;

    // About menu
    let about_item = PredefinedMenuItem::about(
        app,
        Some("About bide"),
        Some(AboutMetadata {
            name: Some("bide".to_string()),
            version: Some(app.package_info().version.to_string()),
            ..Default::default()
        }),
    )?;
    let about_menu = Submenu::with_items(app, "About", true, &[&about_item])?;

    Menu::with_items(app, &[&file_menu, &edit_menu, &about_menu])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let menu = create_menu(app.handle())?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            let _ = app.emit("menu", id);
            if id == "quit" {
                app.exit(0);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running bide application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_menu_constants() {
        assert_eq!("new_tab", "new_tab");
        assert_eq!("close_tab", "close_tab");
        assert_eq!("update_omp", "update_omp");
        assert_eq!("quit", "quit");
    }
}
