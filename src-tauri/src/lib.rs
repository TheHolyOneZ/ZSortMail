mod commands;
mod error;
mod events;
mod notifications;
mod protocol;
#[cfg(debug_assertions)]
mod spikes;
mod tray;
mod windows;

use std::sync::Arc;
use std::time::Duration;

use jiff::tz::TimeZone;
use specta_typescript::Typescript;
use tauri::{Manager, RunEvent, Runtime, WindowEvent};
use tauri_specta::{Builder, collect_commands, collect_events};
use zsm_app::{Core, CoreConfig};
use zsm_core::clock::SystemClock;

pub use events::{TauriSink, ZsmEvent};

macro_rules! all_commands {
    ($($extra:tt)*) => {
        collect_commands![
            commands::app_state,
            commands::start_demo,
            commands::start_second_demo,
            commands::list_people,
            commands::get_person,
            commands::list_conversations,
            commands::get_conversation,
            commands::list_files,
            commands::search,
            commands::mark_conversation_read,
            commands::set_message_starred,
            commands::set_person_pinned,
            commands::set_person_notes,
            commands::edit_person,
            commands::reply,
            commands::archive_conversation,
            commands::trash_conversation,
            commands::hide_conversation,
            commands::hide_person,
            commands::decide_sender,
            commands::decide_domain,
            commands::get_person_flags,
            commands::set_person_flags,
            commands::undo,
            commands::undo_label,
            commands::list_new_senders,
            commands::list_hidden,
            commands::set_domain_rule,
            commands::list_domain_rules,
            commands::list_groups,
            commands::create_group,
            commands::delete_group,
            commands::set_person_groups,
            commands::get_person_groups,
            commands::list_rules,
            commands::save_rule,
            commands::delete_rule,
            commands::order_rules,
            commands::apply_rule,
            commands::preview_query,
            commands::list_watches,
            commands::save_watch,
            commands::delete_watch,
            commands::set_watch_active,
            commands::mark_conversation_seen,
            commands::get_today,
            commands::list_inbox,
            commands::snooze_conversation,
            commands::get_notification_settings,
            commands::save_notification_settings,
            commands::list_notifications,
            commands::open_notification,
            commands::get_compose_settings,
            commands::save_compose_settings,
            commands::new_draft,
            commands::reply_draft,
            commands::save_draft,
            commands::get_draft,
            commands::list_drafts,
            commands::delete_draft,
            commands::add_draft_file,
            commands::remove_draft_file,
            commands::send_draft,
            commands::unsend,
            commands::schedule_draft,
            commands::suggest_recipients,
            commands::list_nudges,
            commands::cancel_nudge,
            commands::unsubscribe,
            commands::mark_all_read,
            commands::get_timeline,
            commands::list_people_in,
            commands::set_active_account,
            commands::get_mirror_tags,
            commands::set_mirror_tags,
            commands::save_category,
            commands::list_category_members,
            commands::add_to_category,
            commands::set_member_alias,
            commands::remove_from_category,
            commands::order_categories,
            commands::merge_people,
            commands::split_person,
            commands::import_contacts,
            commands::list_tags,
            commands::get_conversation_tags,
            commands::tag_conversation,
            commands::untag_conversation,
            commands::list_view,
            commands::list_saved_views,
            commands::save_view,
            commands::delete_view,
            commands::bulk_conversations,
            commands::list_routine_variables,
            commands::preview_routine_text,
            commands::list_routines,
            commands::save_routine,
            commands::delete_routine,
            commands::skip_routine,
            commands::list_routine_runs,
            commands::run_routine,
            commands::google_setup,
            commands::save_google_client,
            commands::begin_gmail_link,
            commands::begin_contacts_import,
            commands::cancel_gmail_link,
            commands::unlink_account,
            commands::sync_status,
            commands::sync_now,
            commands::prepare_attachment,
            commands::save_attachment,
            commands::open_link,
            commands::ping,
            commands::debug_emit,
            $($extra)*
        ]
    };
}

pub fn specta_builder<R: Runtime>() -> Builder<R> {
    #[cfg(debug_assertions)]
    let commands =
        all_commands![spikes::spike_attacks, spikes::spike_beacon, spikes::spike_beacon_reset, spikes::spike_report];
    #[cfg(not(debug_assertions))]
    let commands = all_commands![];

    Builder::<R>::new()
        .commands(commands)
        .events(collect_events![events::ZsmEvent])
        .dangerously_cast_bigints_to_number()
        .disable_serde_phases()
}

fn export_bindings(builder: &Builder<tauri::Wry>) {
    builder
        .export(
            Typescript::default().header(
                "// SPDX-License-Identifier: GPL-3.0-or-later\n\
                 // Generated by tauri-specta from the Rust types. Do not edit by hand.\n\
                 // Regenerate with `pnpm bindings`.",
            ),
            "../src/lib/bindings.ts",
        )
        .expect("failed to export TypeScript bindings");
}

pub fn configure<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(specta_builder::<R>().invoke_handler()).register_uri_scheme_protocol("zsm", protocol::handle)
}

pub fn mount_events<R: Runtime>(app: &impl Manager<R>) {
    specta_builder::<R>().mount_events(app);
}

fn open_core<R: Runtime>(app: &tauri::App<R>) -> Result<Arc<Core>, Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    let core = Core::new(CoreConfig {
        db_path: Some(dir.join("zsortmail.db")),
        quota_per_minute: 6000.0,
        downloads_dir: app.path().download_dir().ok(),
        secrets: zsm_app::platform_secrets(dir.join("secrets")),
        data_dir: Some(dir),
        clock: Arc::new(SystemClock),
        sink: Arc::new(TauriSink(app.handle().clone())),
        coalesce: Duration::from_millis(50),
        timezone: TimeZone::system(),
        runtime: Some(tauri::async_runtime::handle().inner().clone()),
        platform: Arc::new(notifications::DesktopNotifications(app.handle().clone())),
        ..CoreConfig::in_memory()
    })?;
    Ok(core)
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,zsortmail_lib=debug,zsm_core=debug,zsm_app=debug".into()),
        )
        .init();

    #[cfg(debug_assertions)]
    export_bindings(&specta_builder::<tauri::Wry>());

    let app = configure(tauri::Builder::default())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            windows::show_main_window(app);
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            mount_events(app);
            let core = open_core(app)?;
            core.start_sync()?;
            core.start_scheduler();
            app.manage(core);
            #[cfg(target_os = "macos")]
            app.set_menu(tauri::menu::Menu::default(app.handle())?)?;
            tray::setup(app)?;
            #[cfg(debug_assertions)]
            app.manage(spikes::Beacon::start()?);
            windows::create_main_window(app.handle())?;

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(4)).await;
                if let Some(window) = handle.get_webview_window(windows::MAIN_WINDOW)
                    && !window.is_visible().unwrap_or(true)
                {
                    let _ = window.show();
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building ZSortMail");

    app.run(|app, event| {
        if let RunEvent::ExitRequested { api, code, .. } = &event
            && code.is_none()
            && !cfg!(target_os = "macos")
        {
            api.prevent_exit();
        }
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { has_visible_windows, .. } = &event
            && !*has_visible_windows
        {
            windows::show_main_window(app);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = app;
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn export_bindings() {
        super::export_bindings(&super::specta_builder::<tauri::Wry>());
    }
}
