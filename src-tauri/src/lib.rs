pub mod commands;
pub mod git;
pub mod omp;
pub mod persist;
pub mod projects;
pub mod roles;
pub mod update;
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
    let projects_state = projects::ProjectsState::new();
    let projects_state_menu = projects_state.clone();
    let projects_state_window = projects_state.clone();
    let projects_state_fetch = projects_state.clone();
    let projects_state_shutdown = projects_state.clone();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(projects_state)
        .invoke_handler(tauri::generate_handler![
            commands::project_open,
            commands::project_close,
            commands::projects_restore,
            commands::omp_subscribe,
            commands::omp_prompt,
            commands::omp_abort,
            commands::omp_get_state,
            commands::omp_new_session,
            commands::omp_command,
            commands::omp_ui_respond,
            commands::omp_list_roles,
            commands::omp_set_role,
            commands::git_status,
            commands::git_stage,
            commands::git_unstage,
            commands::git_add_all,
            commands::git_commit,
            commands::git_push,
            commands::git_pull,
            commands::git_fetch,
            commands::git_status_text,
            commands::app_update,
        ])
        .setup(move |app| {
            tauri::async_runtime::spawn(projects_state_fetch.hourly_fetch());
            let menu = create_menu(app.handle())?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            let _ = app.emit("menu", id);
            if id == "quit" {
                let ps = projects_state_menu.clone();
                tauri::async_runtime::block_on(async move {
                    ps.shutdown_all().await;
                });
                app.exit(0);
            }
        })
        .on_window_event(move |_window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let ps = projects_state_window.clone();
                tauri::async_runtime::block_on(async move {
                    ps.shutdown_all().await;
                });
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building bide application");

    app.run(move |_app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
            let ps = projects_state_shutdown.clone();
            tauri::async_runtime::block_on(async move {
                ps.shutdown_all().await;
            });
        }
    });
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
