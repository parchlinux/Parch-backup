#![allow(deprecated)]

use std::cell::RefCell;
use std::rc::Rc;
use std::thread;

use crossbeam_channel::{unbounded, Receiver, Sender};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita::prelude::*;

use parch_backup::cli::{BackupArgs, RestoreArgs};
use parch_backup::events::{BackupPhase, ProgressEvent};

fn main() {
    let app = libadwaita::Application::builder()
        .application_id("com.parchlinux.backup")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_startup(|_| {
        if let Some(display) = gdk::Display::default() {
            let icon_theme = gtk4::IconTheme::for_display(&display);
            icon_theme.add_search_path("data/icons/hicolor/scalable/apps");
            icon_theme.add_search_path("assets");
            icon_theme.add_search_path("/usr/share/icons/hicolor/scalable/apps");
        }
    });

    app.connect_activate(|app| build_ui(app, None));
    app.connect_open(|app, files, _| {
        let first_file = files.first().and_then(|f| f.path()).map(|p| p.to_string_lossy().to_string());
        build_ui(app, first_file);
    });

    app.run();
}

fn show_about_dialog(parent: &libadwaita::ApplicationWindow) {
    let about = libadwaita::AboutWindow::builder()
        .transient_for(parent)
        .application_name("Parch Backup")
        .application_icon("com.parchlinux.backup")
        .developer_name("Parch Linux Team")
        .version("0.1.0")
        .copyright("© 2026 Parch Linux Project")
        .website("https://parchlinux.com")
        .issue_url("https://github.com/parchlinux/parch-backup/issues")
        .comments("A modern, fast, and secure system backup utility for Parch Linux.")
        .license_type(gtk4::License::Gpl30)
        .build();

    about.present();
}

fn build_ui(app: &libadwaita::Application, initial_archive_path: Option<String>) {
    let window = libadwaita::ApplicationWindow::builder()
        .application(app)
        .title("Parch Backup")
        .default_width(460)
        .default_height(720)
        .width_request(360)
        .height_request(540)
        .build();

    let header_bar = libadwaita::HeaderBar::new();

    let view_switcher_title = libadwaita::ViewSwitcherTitle::new();
    view_switcher_title.set_title("Parch Backup");
    header_bar.set_title_widget(Some(&view_switcher_title));

    // Header Bar Primary Menu (About Section)
    let menu_button = gtk4::MenuButton::new();
    menu_button.set_icon_name("open-menu-symbolic");

    let menu = gio::Menu::new();
    menu.append(Some("About Parch Backup"), Some("app.about"));
    menu_button.set_menu_model(Some(&menu));
    header_bar.pack_end(&menu_button);

    let window_weak_about = window.downgrade();
    let action_about = gio::SimpleAction::new("about", None);
    action_about.connect_activate(move |_, _| {
        if let Some(win) = window_weak_about.upgrade() {
            show_about_dialog(&win);
        }
    });
    app.add_action(&action_about);

    let view_stack = libadwaita::ViewStack::new();

    // Shared Status Label & Progress Bar
    let progress_bar = gtk4::ProgressBar::new();
    progress_bar.set_fraction(0.0);

    let status_label = gtk4::Label::new(Some("Ready to protect your system"));
    status_label.add_css_class("caption");
    status_label.add_css_class("dim-label");

    // ----------------------------------------------------
    // BACKUP PAGE
    // ----------------------------------------------------
    let backup_page = libadwaita::PreferencesPage::new();

    // Backup Items Group
    let backup_group = libadwaita::PreferencesGroup::new();
    backup_group.set_title("Backup Items");
    backup_group.set_description(Some("Select system components to include in your archive"));

    let switch_apps = libadwaita::SwitchRow::builder()
        .title("Installed Applications")
        .subtitle("Pacman / Paru / Yay explicit package index")
        .active(true)
        .build();

    let switch_home = libadwaita::SwitchRow::builder()
        .title("Home Directory")
        .subtitle("User dotfiles and home directory data")
        .active(true)
        .build();

    let switch_flatpak = libadwaita::SwitchRow::builder()
        .title("Flatpak Applications")
        .subtitle("Installed Flatpak application list")
        .active(true)
        .build();

    let switch_keys = libadwaita::SwitchRow::builder()
        .title("Security Keys")
        .subtitle("GPG (~/.gnupg) and SSH (~/.ssh) key pairs")
        .active(true)
        .build();

    backup_group.add(&switch_apps);
    backup_group.add(&switch_home);
    backup_group.add(&switch_flatpak);
    backup_group.add(&switch_keys);
    backup_page.add(&backup_group);

    // Destination & Security Group
    let storage_group = libadwaita::PreferencesGroup::new();
    storage_group.set_title("Destination and Security");

    let dest_row = libadwaita::ActionRow::builder()
        .title("Destination Folder")
        .subtitle("~/Backups")
        .build();

    let dest_button = gtk4::Button::builder()
        .label("Choose...")
        .valign(gtk4::Align::Center)
        .build();
    dest_row.add_suffix(&dest_button);

    let selected_dest_path = Rc::new(RefCell::new("~/Backups".to_string()));
    let selected_dest_clone = Rc::clone(&selected_dest_path);
    let dest_row_clone = dest_row.clone();
    let window_weak = window.downgrade();

    dest_button.connect_clicked(move |_| {
        let window = match window_weak.upgrade() {
            Some(w) => w,
            None => return,
        };
        let dialog = gtk4::FileDialog::builder()
            .title("Select Backup Destination")
            .build();
        let dest_row_inner = dest_row_clone.clone();
        let selected_dest_inner = Rc::clone(&selected_dest_clone);
        dialog.select_folder(Some(&window), gio::Cancellable::NONE, move |result| {
            if let Ok(folder) = result {
                if let Some(path_str) = folder.path().and_then(|p| p.to_str().map(|s| s.to_string())) {
                    dest_row_inner.set_subtitle(&path_str);
                    *selected_dest_inner.borrow_mut() = path_str;
                }
            }
        });
    });

    let encrypt_switch = libadwaita::SwitchRow::builder()
        .title("Encrypt Archive")
        .subtitle("Argon2id KDF + AES-256-GCM AEAD stream")
        .active(false)
        .build();

    let password_entry = libadwaita::PasswordEntryRow::builder()
        .title("Encryption Password")
        .visible(false)
        .build();

    let password_entry_clone = password_entry.clone();
    encrypt_switch.connect_active_notify(move |sw| {
        password_entry_clone.set_visible(sw.is_active());
    });

    let exclude_entry = libadwaita::EntryRow::builder()
        .title("Exclude Directories (space separated)")
        .build();

    storage_group.add(&dest_row);
    storage_group.add(&encrypt_switch);
    storage_group.add(&password_entry);
    storage_group.add(&exclude_entry);
    backup_page.add(&storage_group);

    // Backup Action Group
    let backup_action_group = libadwaita::PreferencesGroup::new();
    let backup_action_box = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    backup_action_box.set_margin_top(8);
    backup_action_box.set_margin_bottom(8);

    let start_backup_btn = gtk4::Button::builder()
        .label("Create Backup Archive")
        .halign(gtk4::Align::Center)
        .build();
    start_backup_btn.add_css_class("suggested-action");
    start_backup_btn.add_css_class("pill");

    backup_action_box.append(&start_backup_btn);
    backup_action_group.add(&backup_action_box);
    backup_page.add(&backup_action_group);

    let page_backup = view_stack.add_titled(&backup_page, Some("backup"), "Backup");
    page_backup.set_icon_name(Some("document-export-symbolic"));

    // ----------------------------------------------------
    // RESTORE PAGE
    // ----------------------------------------------------
    let restore_page = libadwaita::PreferencesPage::new();

    let restore_group = libadwaita::PreferencesGroup::new();
    restore_group.set_title("Archive Selection");

    let initial_archive = initial_archive_path.clone().unwrap_or_default();
    let archive_row_subtitle = if initial_archive.is_empty() {
        "No file selected".to_string()
    } else {
        initial_archive.clone()
    };

    let archive_row = libadwaita::ActionRow::builder()
        .title("PBAR Archive File")
        .subtitle(&archive_row_subtitle)
        .build();

    let archive_button = gtk4::Button::builder()
        .label("Browse...")
        .valign(gtk4::Align::Center)
        .build();
    archive_row.add_suffix(&archive_button);

    let selected_archive_path = Rc::new(RefCell::new(initial_archive));
    let selected_archive_clone = Rc::clone(&selected_archive_path);
    let archive_row_clone = archive_row.clone();
    let window_weak2 = window.downgrade();

    archive_button.connect_clicked(move |_| {
        let window = match window_weak2.upgrade() {
            Some(w) => w,
            None => return,
        };
        let dialog = gtk4::FileDialog::builder()
            .title("Select PBAR Archive")
            .build();

        let filter = gtk4::FileFilter::new();
        filter.add_pattern("*.pbar");
        filter.set_name(Some("Parch Backup Archives (*.pbar)"));

        let filters = gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));

        let archive_row_inner = archive_row_clone.clone();
        let selected_archive_inner = Rc::clone(&selected_archive_clone);

        dialog.open(Some(&window), gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result {
                if let Some(path_str) = file.path().and_then(|p| p.to_str().map(|s| s.to_string())) {
                    archive_row_inner.set_subtitle(&path_str);
                    *selected_archive_inner.borrow_mut() = path_str;
                }
            }
        });
    });

    let decrypt_switch = libadwaita::SwitchRow::builder()
        .title("Archive is Encrypted")
        .active(false)
        .build();

    let decrypt_password_entry = libadwaita::PasswordEntryRow::builder()
        .title("Decryption Password")
        .visible(false)
        .build();

    let decrypt_password_clone = decrypt_password_entry.clone();
    decrypt_switch.connect_active_notify(move |sw| {
        decrypt_password_clone.set_visible(sw.is_active());
    });

    restore_group.add(&archive_row);
    restore_group.add(&decrypt_switch);
    restore_group.add(&decrypt_password_entry);
    restore_page.add(&restore_group);

    // Restore Action Group
    let restore_action_group = libadwaita::PreferencesGroup::new();
    let restore_action_box = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    restore_action_box.set_margin_top(8);
    restore_action_box.set_margin_bottom(8);

    let start_restore_btn = gtk4::Button::builder()
        .label("Start Restore Process")
        .halign(gtk4::Align::Center)
        .build();
    start_restore_btn.add_css_class("suggested-action");
    start_restore_btn.add_css_class("pill");

    restore_action_box.append(&start_restore_btn);
    restore_action_group.add(&restore_action_box);
    restore_page.add(&restore_action_group);

    let page_restore = view_stack.add_titled(&restore_page, Some("restore"), "Restore");
    page_restore.set_icon_name(Some("document-import-symbolic"));

    view_switcher_title.set_stack(Some(&view_stack));

    if initial_archive_path.is_some() {
        view_stack.set_visible_child_name("restore");
    }

    // Bottom View Switcher Bar for Mobile / Narrow form factors
    let view_switcher_bar = libadwaita::ViewSwitcherBar::new();
    view_switcher_bar.set_stack(Some(&view_stack));

    view_switcher_title
        .bind_property("title-visible", &view_switcher_bar, "reveal")
        .sync_create()
        .build();

    // Progress Bar & Status Section
    let progress_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    progress_box.set_margin_start(20);
    progress_box.set_margin_end(20);
    progress_box.set_margin_top(8);
    progress_box.set_margin_bottom(8);

    progress_box.append(&progress_bar);
    progress_box.append(&status_label);

    let main_content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    main_content.append(&header_bar);

    view_stack.set_vexpand(true);
    main_content.append(&view_stack);

    main_content.append(&progress_box);
    main_content.append(&view_switcher_bar);

    window.set_content(Some(&main_content));

    // ----------------------------------------------------
    // THREAD EVENT MESSAGING & ACTIONS
    // ----------------------------------------------------
    let (tx, rx): (Sender<ProgressEvent>, Receiver<ProgressEvent>) = unbounded();
    let status_label_clone = status_label.clone();
    let progress_bar_clone = progress_bar.clone();
    let start_backup_btn_clone = start_backup_btn.clone();
    let start_restore_btn_clone = start_restore_btn.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        while let Ok(event) = rx.try_recv() {
            match event {
                ProgressEvent::PhaseChanged(phase) => {
                    let text = match phase {
                        BackupPhase::Scanning => "Scanning system...",
                        BackupPhase::Packages => "Backing up package list...",
                        BackupPhase::Flatpaks => "Backing up Flatpaks...",
                        BackupPhase::Home => "Compressing home directory...",
                        BackupPhase::Keys => "Backing up security keys...",
                        BackupPhase::Compressing => "Building PBAR container...",
                        BackupPhase::Encrypting => "Encrypting stream with AES-256-GCM...",
                        BackupPhase::Restoring => "Restoring archive files...",
                        BackupPhase::Done => "Operation complete!",
                    };
                    status_label_clone.set_text(text);
                    progress_bar_clone.pulse();
                }
                ProgressEvent::FileProgress { file_name, .. } => {
                    status_label_clone.set_text(&format!("Processing {}", file_name));
                    progress_bar_clone.pulse();
                }
                ProgressEvent::StatusMessage(msg) => {
                    status_label_clone.set_text(&msg);
                    progress_bar_clone.set_fraction(1.0);
                }
                ProgressEvent::Completed => {
                    status_label_clone.set_text("Operation completed successfully!");
                    progress_bar_clone.set_fraction(1.0);
                    start_backup_btn_clone.set_sensitive(true);
                    start_restore_btn_clone.set_sensitive(true);
                }
                ProgressEvent::Error(err) => {
                    status_label_clone.set_text(&format!("Error: {}", err));
                    progress_bar_clone.set_fraction(0.0);
                    start_backup_btn_clone.set_sensitive(true);
                    start_restore_btn_clone.set_sensitive(true);
                }
                _ => {}
            }
        }
        glib::ControlFlow::Continue
    });

    let selected_dest_for_backup = Rc::clone(&selected_dest_path);
    let start_backup_btn_for_click = start_backup_btn.clone();
    let start_restore_btn_for_backup = start_restore_btn.clone();
    let tx_backup = tx.clone();

    start_backup_btn.connect_clicked(move |_| {
        let dest = selected_dest_for_backup.borrow().clone();
        let apps = switch_apps.is_active();
        let home = switch_home.is_active();
        let flatpak = switch_flatpak.is_active();
        let keys = switch_keys.is_active();
        let encrypt = encrypt_switch.is_active();
        let password = password_entry.text().to_string();
        let excludes: Vec<String> = exclude_entry
            .text()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        start_backup_btn_for_click.set_sensitive(false);
        start_restore_btn_for_backup.set_sensitive(false);

        let tx = tx_backup.clone();

        thread::spawn(move || {
            let args = BackupArgs {
                archive_path: Some(dest),
                apps,
                home,
                exclude_dir: excludes,
                flatpak,
                keys,
                encrypt,
                encrypt_key: if encrypt { Some(password) } else { None },
            };
            parch_backup::backup::backup::handle_backup_with_tx(&args, Some(&tx));
        });
    });

    let selected_archive_for_restore = Rc::clone(&selected_archive_path);
    let start_restore_btn_for_click = start_restore_btn.clone();
    let start_backup_btn_for_restore = start_backup_btn.clone();
    let tx_restore = tx;

    start_restore_btn.connect_clicked(move |_| {
        let archive_path = selected_archive_for_restore.borrow().clone();
        if archive_path.is_empty() {
            status_label.set_text("Please select a .pbar archive file first!");
            return;
        }

        let decrypt = decrypt_switch.is_active();
        let password = decrypt_password_entry.text().to_string();

        start_restore_btn_for_click.set_sensitive(false);
        start_backup_btn_for_restore.set_sensitive(false);

        let tx = tx_restore.clone();

        thread::spawn(move || {
            let args = RestoreArgs {
                archive_path,
                decrypt,
                decrypt_key: if decrypt { Some(password) } else { None },
            };
            if let Err(e) = parch_backup::restore::restore::handle_restore_with_tx(&args, Some(&tx)) {
                let _ = tx.send(ProgressEvent::Error(e.to_string()));
            }
        });
    });

    window.present();
}
