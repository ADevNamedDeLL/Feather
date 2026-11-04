use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use sourceview5 as sv;
use sourceview5::prelude::*;
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const APP_ID: &str = "dev.feather.Editor";

#[derive(Clone)]
struct Document {
    path: Option<PathBuf>,
    buffer: sv::Buffer,
    page: gtk::Widget,
    tab_title: gtk::Label,
    editor_view: sv::View,
    _file_monitor: Option<gio::FileMonitor>,
}

#[derive(Clone)]
struct EditorState {
    documents: Rc<RefCell<Vec<Document>>>,
    notebook: gtk::Notebook,
    status: gtk::Label,
    root_dir: Rc<RefCell<PathBuf>>,
    workspace_history: Rc<RefCell<Vec<PathBuf>>>,
    history_index: Rc<Cell<usize>>,
    explorer_tree: gtk::Box,
    root_label: gtk::Label,
    show_hidden: Rc<Cell<bool>>,
    selected_file_row: Rc<RefCell<Option<gtk::Button>>>,
    welcome_page: gtk::Box,
    parent_window: Rc<RefCell<Option<ApplicationWindow>>>,
    recent_files: Rc<RefCell<Vec<PathBuf>>>,
}

fn main() -> glib::ExitCode {
    configure_graphics_backend();
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("Feather {}", env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Feather {} — lightweight native code editor", env!("CARGO_PKG_VERSION"));
        println!("Usage: feather [FILE|DIRECTORY]");
        println!("       feather --version");
        println!("       feather --help");
        return glib::ExitCode::SUCCESS;
    }
    let launch_target = args.first().cloned().map(|arg| expand_user_path(PathBuf::from(arg)));
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| install_css());
    app.connect_activate(move |app| build_ui(app, launch_target.clone()));
    app.run_with_args::<&str>(&[])
}

fn configure_graphics_backend() {
    if std::env::var_os("GSK_RENDERER").is_some() {
        return;
    }

    let is_wsl = std::env::var_os("WSL_DISTRO_NAME").is_some()
        || std::env::var_os("WSL_INTEROP").is_some()
        || fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|release| {
                let release = release.to_ascii_lowercase();
                release.contains("microsoft") || release.contains("wsl")
            })
            .unwrap_or(false);

    if is_wsl {
        std::env::set_var("GSK_RENDERER", "cairo");
    }
}

fn expand_user_path(path: PathBuf) -> PathBuf {
    let value = path.to_string_lossy();
    if value == "~" {
        return std::env::var_os("HOME").map(PathBuf::from).unwrap_or(path);
    }
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    path
}

fn install_css() {
    let css = include_str!("../data/feather.css");
    let provider = gtk::CssProvider::new();
    provider.load_from_data(css);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}



fn vscode_icons_dir() -> Option<PathBuf> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))?;
    let installed = data_home.join("feather/vscode-icons/icons");
    if installed.is_dir() { return Some(installed); }
    if let Some(bundle_dir) = std::env::var_os("FEATHER_BUNDLE_DIR").map(PathBuf::from) {
        let bundled_icons = bundle_dir.join("vscode-icons/icons");
        if bundled_icons.is_dir() { return Some(bundled_icons); }
    }
    let bundled = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/icons/vscode-icons/icons");
    bundled.is_dir().then_some(bundled)
}

fn vscode_file_icon_path(icon_id: &str) -> Option<PathBuf> {
    let icons = vscode_icons_dir()?;
    let specific = icons.join(format!("file_type_{icon_id}.svg"));
    if specific.is_file() { return Some(specific); }
    let default = icons.join("default_file.svg");
    default.is_file().then_some(default)
}

fn vscode_folder_icon_path(folder_name: &str, opened: bool) -> Option<PathBuf> {
    let icons = vscode_icons_dir()?;
    let slug = folder_name.trim_start_matches('.').to_ascii_lowercase().replace(' ', "_").replace('-', "_").replace('.', "_");
    let specific_name = if opened { format!("folder_type_{slug}_opened.svg") } else { format!("folder_type_{slug}.svg") };
    let specific = icons.join(specific_name);
    if specific.is_file() { return Some(specific); }
    let default_name = if opened { "default_folder_opened.svg" } else { "default_folder.svg" };
    let default = icons.join(default_name);
    default.is_file().then_some(default)
}

fn vscode_file_image(icon_id: &str) -> gtk::Image {
    if let Some(path) = vscode_file_icon_path(icon_id) {
        gtk::Image::from_file(path)
    } else {
        gtk::Image::from_icon_name("text-x-generic-symbolic")
    }
}

fn vscode_folder_image(folder_name: &str, opened: bool) -> gtk::Image {
    if let Some(path) = vscode_folder_icon_path(folder_name, opened) {
        gtk::Image::from_file(path)
    } else {
        gtk::Image::from_icon_name("folder-symbolic")
    }
}

const MAX_TEXT_FILE_BYTES: u64 = 10 * 1024 * 1024;
const BINARY_PROBE_BYTES: usize = 8192;

const SHORTCUT_BINDINGS: &[(&str, &str)] = &[
    ("new", "<Primary>n"),
    ("open", "<Primary>o"),
    ("save", "<Primary>s"),
    ("save-as", "<Primary><Shift>s"),
    ("help", "F1"),
    ("open-folder", "<Primary><Shift>o"),
    ("explorer-back", "<Alt>Left"),
    ("explorer-forward", "<Alt>Right"),
    ("explorer-up", "<Alt>Up"),
    ("explorer-refresh", "<Primary><Shift>r"),
    ("command-palette", "<Primary><Shift>p"),
    ("quick-open", "<Primary>p"),
    ("find-replace", "<Primary>h"),
    ("goto-line", "<Primary>g"),
    ("duplicate-line", "<Primary><Shift>d"),
    ("move-line-up", "<Alt><Shift>Up"),
    ("move-line-down", "<Alt><Shift>Down"),
    ("toggle-comment", "<Primary>slash"),
];

fn shortcut_accelerator(action: &str) -> &'static str {
    SHORTCUT_BINDINGS.iter()
        .find(|(name, _)| *name == action)
        .map(|(_, accelerator)| *accelerator)
        .expect("shortcut must be registered in SHORTCUT_BINDINGS")
}


fn config_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("feather").join("state"))
}

fn load_last_workspace() -> Option<PathBuf> {
    let path = config_file()?;
    let saved = fs::read_to_string(path).ok()?;
    let candidate = PathBuf::from(saved.trim());
    candidate.is_dir().then_some(candidate)
}

fn save_last_workspace(path: &Path) {
    let Some(file) = config_file() else { return; };
    let Some(parent) = file.parent() else { return; };
    if fs::create_dir_all(parent).is_ok() {
        let _ = fs::write(file, path.to_string_lossy().as_bytes());
    }
}

fn is_binary_bytes(bytes: &[u8]) -> bool {
    bytes.iter().take(BINARY_PROBE_BYTES).any(|byte| *byte == 0)
}

fn is_binary_file(path: &Path) -> std::io::Result<bool> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut probe = [0u8; BINARY_PROBE_BYTES];
    let count = file.read(&mut probe)?;
    Ok(is_binary_bytes(&probe[..count]))
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    }
}

fn build_ui(app: &Application, launch_target: Option<PathBuf>) {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let initial_dir = launch_target.as_ref()
        .filter(|path| path.is_dir())
        .cloned()
        .or_else(load_last_workspace)
        .filter(|path| path.is_dir())
        .unwrap_or(current_dir);
    save_last_workspace(&initial_dir);
    let explorer_tree = gtk::Box::new(gtk::Orientation::Vertical, 1);
    explorer_tree.add_css_class("explorer-tree");
    let root_label = gtk::Label::new(Some(&initial_dir.display().to_string()));
    root_label.set_xalign(0.0);
    root_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    root_label.add_css_class("workspace-name");

    let state = EditorState {
        documents: Rc::new(RefCell::new(Vec::new())),
        notebook: gtk::Notebook::new(),
        status: gtk::Label::new(Some("Ready")),
        root_dir: Rc::new(RefCell::new(initial_dir.clone())),
        workspace_history: Rc::new(RefCell::new(vec![initial_dir.clone()])),
        history_index: Rc::new(Cell::new(0)),
        explorer_tree: explorer_tree.clone(),
        root_label: root_label.clone(),
        show_hidden: Rc::new(Cell::new(false)),
        selected_file_row: Rc::new(RefCell::new(None)),
        welcome_page: gtk::Box::new(gtk::Orientation::Vertical, 0),
        parent_window: Rc::new(RefCell::new(None)),
        recent_files: Rc::new(RefCell::new(load_recent_files())),
    };

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Feather")
        .default_width(980)
        .default_height(640)
        .build();
    *state.parent_window.borrow_mut() = Some(window.clone());

    let titlebar = gtk::HeaderBar::new();
    titlebar.add_css_class("feather-titlebar");
    titlebar.set_show_title_buttons(true);
    let title_brand = gtk::Box::new(gtk::Orientation::Horizontal, 7);
    let title_logo = make_feather_logo(18);
    title_logo.add_css_class("titlebar-logo");
    title_brand.append(&title_logo);
    let title_text = gtk::Label::new(Some("Feather"));
    title_text.add_css_class("titlebar-title");
    title_brand.append(&title_text);
    titlebar.set_title_widget(Some(&title_brand));
    window.set_titlebar(Some(&titlebar));

    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("toolbar");
    header.set_margin_top(7);
    header.set_margin_bottom(7);
    header.set_margin_start(10);
    header.set_margin_end(10);

    let brand_mark = make_feather_logo(25);
    brand_mark.set_tooltip_text(Some("Feather"));
    header.append(&brand_mark);

    let brand = gtk::Label::new(Some("Feather"));
    brand.add_css_class("brand");
    header.append(&brand);

    let new_button = make_button("New", "document-new-symbolic");
    let open_button = make_button("Open File", "document-open-symbolic");
    let folder_button = make_button("Open Folder", "folder-open-symbolic");
    let save_button = make_button("Save", "document-save-symbolic");
    let help_button = make_button("Help", "help-about-symbolic");
    help_button.set_tooltip_text(Some("Keyboard shortcuts and help (F1)"));
    header.append(&new_button);
    header.append(&open_button);
    header.append(&folder_button);
    header.append(&save_button);
    header.append(&help_button);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    header.append(&spacer);
    let title_label = gtk::Label::new(Some("Lightweight code editor"));
    title_label.add_css_class("muted");
    header.append(&title_label);
    shell.append(&header);

    let body = gtk::Paned::new(gtk::Orientation::Horizontal);
    body.set_wide_handle(true);
    body.set_position(255);

    let explorer = gtk::Box::new(gtk::Orientation::Vertical, 5);
    explorer.set_size_request(190, -1);
    explorer.set_margin_top(9);
    explorer.set_margin_bottom(8);
    explorer.set_margin_start(8);
    explorer.set_margin_end(7);

    let explorer_heading = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    explorer_heading.add_css_class("explorer-heading");
    let explorer_title = gtk::Label::new(Some("EXPLORER"));
    explorer_title.set_margin_start(2);
    explorer_title.set_xalign(0.0);
    explorer_title.add_css_class("section-title");
    explorer_title.set_hexpand(true);
    explorer_heading.append(&explorer_title);

    let refresh_button = icon_button("view-refresh-symbolic", "Refresh Explorer");
    let collapse_button = icon_button("view-list-symbolic", "Collapse All");
    let hidden_button = icon_button("view-reveal-symbolic", "Toggle hidden files");
    explorer_heading.append(&refresh_button);
    explorer_heading.append(&collapse_button);
    explorer_heading.append(&hidden_button);
    explorer.append(&explorer_heading);

    let nav_bar = gtk::Box::new(gtk::Orientation::Horizontal, 3);
    nav_bar.add_css_class("explorer-navigation");
    let back_button = icon_button("go-previous-symbolic", "Back to previous folder");
    let forward_button = icon_button("go-next-symbolic", "Forward to next folder");
    let up_button = icon_button("go-up-symbolic", "Open parent folder as workspace");
    nav_bar.append(&back_button);
    nav_bar.append(&forward_button);
    nav_bar.append(&up_button);
    explorer.append(&nav_bar);

    let workspace_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    workspace_header.add_css_class("workspace-header");
    let workspace_chevron = gtk::Image::from_icon_name("pan-down-symbolic");
    workspace_chevron.add_css_class("tree-chevron");
    workspace_header.append(&workspace_chevron);
    let workspace_label = gtk::Label::new(Some("WORKSPACE"));
    workspace_label.set_xalign(0.0);
    workspace_label.add_css_class("section-title");
    workspace_header.append(&workspace_label);
    explorer.append(&workspace_header);
    root_label.set_margin_start(20);
    root_label.add_css_class("root-path");
    explorer.append(&root_label);

    let file_scroll = gtk::ScrolledWindow::new();
    file_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    file_scroll.set_child(Some(&explorer_tree));
    file_scroll.set_vexpand(true);
    explorer.append(&file_scroll);
    body.set_start_child(Some(&explorer));

    let editor_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    editor_area.add_css_class("editor-area");
    state.notebook.set_scrollable(true);
    state.notebook.set_show_border(false);
    state.notebook.set_hexpand(true);
    state.notebook.set_group_name(Some("feather-tabs"));
    state.notebook.set_vexpand(true);
    state.notebook.set_visible(false);
    state.welcome_page.set_hexpand(true);
    state.welcome_page.set_vexpand(true);
    state.welcome_page.add_css_class("welcome-page");
    editor_area.append(&state.welcome_page);
    editor_area.append(&state.notebook);
    body.set_end_child(Some(&editor_area));
    shell.append(&body);

    let status_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    status_bar.add_css_class("statusbar");
    status_bar.set_margin_start(10);
    status_bar.set_margin_end(10);
    status_bar.set_margin_top(4);
    status_bar.set_margin_bottom(4);
    state.status.set_xalign(0.0);
    state.status.add_css_class("status-text");
    status_bar.append(&state.status);
    shell.append(&status_bar);
    window.set_child(Some(&shell));
    let dismiss_popovers = gtk::GestureClick::new();
    dismiss_popovers.set_button(1);
    dismiss_popovers.set_propagation_phase(gtk::PropagationPhase::Capture);
    dismiss_popovers.connect_pressed({
        let window = window.clone();
        move |_, _, _, _| dismiss_visible_popovers(&window.clone().upcast::<gtk::Widget>())
    });
    window.add_controller(dismiss_popovers);

    refresh_explorer(&state);

    let state_new = state.clone();
    new_button.connect_clicked({ let parent = window.clone(); move |_| prompt_new_file(&parent, &state_new) });

    let state_open = state.clone();
    let parent_open = window.clone();
    open_button.connect_clicked(move |_| choose_file(&parent_open, &state_open));

    let state_folder = state.clone();
    let parent_folder = window.clone();
    folder_button.connect_clicked(move |_| choose_folder(&parent_folder, &state_folder));

    let state_save = state.clone();
    let parent_save = window.clone();
    save_button.connect_clicked(move |_| save_active(&parent_save, &state_save));

    let parent_help = window.clone();
    help_button.connect_clicked(move |_| show_help(&parent_help));

    let state_refresh = state.clone();
    refresh_button.connect_clicked(move |_| refresh_explorer(&state_refresh));

    let state_collapse = state.clone();
    collapse_button.connect_clicked(move |_| refresh_explorer(&state_collapse));

    let state_hidden = state.clone();
    hidden_button.connect_clicked(move |_| {
        state_hidden.show_hidden.set(!state_hidden.show_hidden.get());
        refresh_explorer(&state_hidden);
    });

    let state_back = state.clone();
    back_button.connect_clicked(move |_| navigate_history(&state_back, -1));
    let state_forward = state.clone();
    forward_button.connect_clicked(move |_| navigate_history(&state_forward, 1));
    let state_up = state.clone();
    up_button.connect_clicked(move |_| {
        let parent = state_up.root_dir.borrow().parent().map(Path::to_path_buf);
        if let Some(parent) = parent {
            set_workspace(&state_up, parent, true);
        }
    });

    state.notebook.connect_switch_page({
        let state = state.clone();
        move |_, _, page_num| update_status(&state, page_num as usize)
    });

    add_action(app, "new", shortcut_accelerator("new"), {
        let state = state.clone();
        move || { if let Some(parent) = state.parent_window.borrow().clone() { prompt_new_file(&parent, &state); } }
    });
    add_action(app, "open", shortcut_accelerator("open"), {
        let state = state.clone();
        let parent = window.clone();
        move || choose_file(&parent, &state)
    });
    add_action(app, "save", shortcut_accelerator("save"), {
        let state = state.clone();
        let parent = window.clone();
        move || save_active(&parent, &state)
    });
    add_action(app, "save-as", shortcut_accelerator("save-as"), {
        let state = state.clone();
        let parent = window.clone();
        move || save_as(&parent, &state)
    });
    add_action(app, "help", shortcut_accelerator("help"), {
        let parent = window.clone();
        move || show_help(&parent)
    });
    add_action(app, "open-folder", shortcut_accelerator("open-folder"), {
        let state = state.clone();
        let parent = window.clone();
        move || choose_folder(&parent, &state)
    });
    add_action(app, "explorer-back", shortcut_accelerator("explorer-back"), {
        let state = state.clone();
        move || navigate_history(&state, -1)
    });
    add_action(app, "explorer-forward", shortcut_accelerator("explorer-forward"), {
        let state = state.clone();
        move || navigate_history(&state, 1)
    });
    add_action(app, "explorer-up", shortcut_accelerator("explorer-up"), {
        let state = state.clone();
        move || {
            let parent = state.root_dir.borrow().parent().map(Path::to_path_buf);
            if let Some(parent) = parent { set_workspace(&state, parent, true); }
        }
    });
    add_action(app, "explorer-refresh", shortcut_accelerator("explorer-refresh"), {
        let state = state.clone();
        move || refresh_explorer(&state)
    });

    add_action(app, "command-palette", shortcut_accelerator("command-palette"), {
        let state = state.clone(); let parent = window.clone();
        move || show_command_palette(&parent, &state)
    });
    add_action(app, "quick-open", shortcut_accelerator("quick-open"), {
        let state = state.clone(); let parent = window.clone();
        move || show_quick_open(&parent, &state)
    });
    add_action(app, "find-replace", shortcut_accelerator("find-replace"), {
        let state = state.clone(); let parent = window.clone();
        move || show_find_replace(&parent, &state)
    });
    add_action(app, "goto-line", shortcut_accelerator("goto-line"), {
        let state = state.clone(); let parent = window.clone();
        move || show_goto_line(&parent, &state)
    });
    add_action(app, "duplicate-line", shortcut_accelerator("duplicate-line"), {
        let state = state.clone(); move || duplicate_line(&state)
    });
    add_action(app, "move-line-up", shortcut_accelerator("move-line-up"), {
        let state = state.clone(); move || move_line(&state, -1)
    });
    add_action(app, "move-line-down", shortcut_accelerator("move-line-down"), {
        let state = state.clone(); move || move_line(&state, 1)
    });
    add_action(app, "toggle-comment", shortcut_accelerator("toggle-comment"), {
        let state = state.clone(); move || toggle_comment(&state)
    });

    match launch_target {
        Some(target) if target.is_file() => open_path(&state, &target),
        _ => show_welcome(&state, &window),
    }
    let close_state = state.clone();
    let close_window = window.clone();
    window.connect_close_request(move |_| {
        let has_unsaved = close_state.documents.borrow().iter().any(|doc| doc.buffer.is_modified());
        if has_unsaved {
            request_close_application(&close_window, &close_state);
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });

    window.present();
}


fn make_feather_logo(size: i32) -> gtk::DrawingArea {
    let logo = gtk::DrawingArea::new();
    logo.set_content_width(size);
    logo.set_content_height(size);
    logo.set_size_request(size, size);
    logo.set_halign(gtk::Align::Center);
    logo.set_valign(gtk::Align::Center);
    logo.set_hexpand(false);
    logo.set_vexpand(false);
    logo.add_css_class("feather-logo");

    logo.set_draw_func(|_, cr, width, height| {
        // Always draw into a square 128×128 coordinate space. GTK may allocate
        // a wider area to children in a Box; uniform scaling prevents distortion.
        let side = (width.min(height)) as f64;
        if side <= 0.0 {
            return;
        }
        cr.save().ok();
        cr.translate((width as f64 - side) / 2.0, (height as f64 - side) / 2.0);
        cr.scale(side / 128.0, side / 128.0);
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        cr.set_line_join(gtk::cairo::LineJoin::Round);

        // Quill silhouette.
        cr.move_to(15.0, 114.0);
        cr.curve_to(23.0, 77.0, 38.0, 22.0, 103.0, 12.0);
        cr.curve_to(113.0, 10.0, 118.0, 14.0, 114.0, 25.0);
        cr.curve_to(103.0, 65.0, 76.0, 96.0, 15.0, 114.0);
        cr.close_path();
        cr.set_source_rgba(0.796, 0.651, 0.969, 0.13);
        cr.fill_preserve().ok();
        cr.set_line_width(5.0);
        cr.set_source_rgb(0.796, 0.651, 0.969);
        cr.stroke().ok();

        // Central rachis.
        cr.move_to(15.0, 114.0);
        cr.curve_to(48.0, 82.0, 81.0, 43.0, 114.0, 15.0);
        cr.set_line_width(4.0);
        cr.set_source_rgb(0.537, 0.706, 0.980);
        cr.stroke().ok();

        // Fine barbs, kept sparse so the mark remains legible at 16 px.
        cr.set_line_width(2.5);
        cr.move_to(43.0, 87.0);
        cr.line_to(36.0, 64.0);
        cr.move_to(58.0, 70.0);
        cr.line_to(48.0, 49.0);
        cr.move_to(74.0, 53.0);
        cr.line_to(65.0, 34.0);
        cr.move_to(55.0, 73.0);
        cr.line_to(79.0, 74.0);
        cr.move_to(73.0, 55.0);
        cr.line_to(96.0, 56.0);
        cr.stroke().ok();

        // Tapered nib.
        cr.move_to(15.0, 114.0);
        cr.line_to(28.0, 101.0);
        cr.set_line_width(3.0);
        cr.set_source_rgb(0.961, 0.753, 0.902);
        cr.stroke().ok();
        cr.restore().ok();
    });
    logo
}

fn recent_file_store() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("feather").join("recent"))
}

fn load_recent_files() -> Vec<PathBuf> {
    let Some(path) = recent_file_store() else { return Vec::new(); };
    fs::read_to_string(path).unwrap_or_default().lines()
        .map(PathBuf::from).filter(|path| path.is_file()).take(20).collect()
}

fn remember_recent_file(state: &EditorState, path: PathBuf) {
    let mut files = state.recent_files.borrow_mut();
    files.retain(|item| item != &path);
    files.insert(0, path);
    files.truncate(20);
    if let Some(store) = recent_file_store() {
        if let Some(parent) = store.parent() { let _ = fs::create_dir_all(parent); }
        let content = files.iter().map(|item| item.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n");
        let _ = fs::write(store, content);
    }
}

fn active_editor(state: &EditorState) -> Option<Document> {
    let page = state.notebook.current_page()?;
    let widget = state.notebook.nth_page(Some(page))?;
    state.documents.borrow().iter().find(|doc| doc.page == widget).cloned()
}

fn editor_dialog(parent: &ApplicationWindow, title: &str, width: i32, height: i32) -> (gtk::Window, gtk::Box) {
    let dialog = gtk::Window::builder().title(title).transient_for(parent).modal(true)
        .decorated(false).default_width(width).default_height(height).resizable(false).build();
    dialog.add_css_class("feather-dialog-window");
    let titlebar = gtk::HeaderBar::new();
    titlebar.add_css_class("feather-dialog-titlebar");
    titlebar.set_show_title_buttons(false);
    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("feather-dialog-titlebar-label");
    titlebar.set_title_widget(Some(&title_label));
    dialog.set_titlebar(Some(&titlebar));
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 10);
    shell.set_margin_top(16); shell.set_margin_bottom(16);
    shell.set_margin_start(18); shell.set_margin_end(18);
    shell.add_css_class("dialog-shell");
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0); heading.add_css_class("dialog-title");
    shell.append(&heading);
    dialog.set_child(Some(&shell));
    (dialog, shell)
}

fn show_goto_line(parent: &ApplicationWindow, state: &EditorState) {
    let Some(doc) = active_editor(state) else { state.status.set_text("Open a file before using Go to Line"); return; };
    let (dialog, shell) = editor_dialog(parent, "Go to Line", 360, 150);
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some("Line number"));
    entry.set_input_purpose(gtk::InputPurpose::Digits);
    shell.append(&entry);
    let buttons = dialog_button_row();
    let cancel = dialog_button("Cancel", false);
    let go = dialog_button("Go", true);
    buttons.append(&cancel); buttons.append(&go); shell.append(&buttons);
    let d = dialog.clone(); cancel.connect_clicked(move |_| d.close());
    let d = dialog.clone(); let buffer = doc.buffer.clone(); let status = state.status.clone(); let input = entry.clone();
    go.connect_clicked(move |_| {
        match input.text().trim().parse::<i32>() {
            Ok(line) if line > 0 && line <= buffer.line_count() => {
                let iter = buffer.iter_at_line(line - 1).unwrap_or_else(|| buffer.end_iter());
                buffer.place_cursor(&iter);
                let mut target = iter;
                doc.editor_view.scroll_to_iter(&mut target, 0.12, true, 0.0, 0.5);
                doc.editor_view.grab_focus();
                status.set_text(&format!("Moved to line {line}"));
                d.close();
            }
            _ => status.set_text("Enter a valid line number"),
        }
    });
    dialog.present(); entry.grab_focus();
}

fn show_find_replace(parent: &ApplicationWindow, state: &EditorState) {
    let Some(doc) = active_editor(state) else { state.status.set_text("Open a file before searching"); return; };
    let (dialog, shell) = editor_dialog(parent, "Find & Replace", 430, 230);
    let find = gtk::Entry::new(); find.set_placeholder_text(Some("Find"));
    let replace = gtk::Entry::new(); replace.set_placeholder_text(Some("Replace with"));
    shell.append(&find); shell.append(&replace);
    let buttons = dialog_button_row();
    let cancel = dialog_button("Close", false);
    let next = dialog_button("Find Next", false);
    let replace_one = dialog_button("Replace", false);
    let all = dialog_button("Replace All", true);
    buttons.append(&cancel); buttons.append(&next); buttons.append(&replace_one); buttons.append(&all);
    shell.append(&buttons);
    let d = dialog.clone(); cancel.connect_clicked(move |_| d.close());
    let buffer = doc.buffer.clone(); let status = state.status.clone(); let query = find.clone();
    next.connect_clicked(move |_| {
        let needle = query.text().to_string();
        if needle.is_empty() { return; }
        let start = buffer.iter_at_mark(&buffer.get_insert());
        if let Some((match_start, match_end)) = start.forward_search(&needle, gtk::TextSearchFlags::empty(), None) {
            buffer.select_range(&match_start, &match_end);
        } else {
            let beginning = buffer.start_iter();
            if let Some((match_start, match_end)) = beginning.forward_search(&needle, gtk::TextSearchFlags::empty(), None) {
                buffer.select_range(&match_start, &match_end);
            } else { status.set_text("No matches found"); }
        }
    });
    let buffer = doc.buffer.clone(); let query = find.clone(); let replacement = replace.clone(); let status = state.status.clone();
    replace_one.connect_clicked(move |_| {
        let needle = query.text().to_string();
        if needle.is_empty() { return; }
        let (start, end) = buffer.selection_bounds().unwrap_or_else(|| {
            let iter = buffer.iter_at_mark(&buffer.get_insert()); (iter, iter)
        });
        let selected = buffer.text(&start, &end, true).to_string();
        if selected == needle {
            let mut cursor = start;
            let mut finish = end;
            buffer.delete(&mut cursor, &mut finish);
            buffer.insert(&mut cursor, &replacement.text());
            status.set_text("Occurrence replaced");
        } else { status.set_text("Select a matching occurrence first, or use Find Next"); }
    });
    let buffer = doc.buffer.clone(); let query = find.clone(); let replacement = replace.clone(); let status = state.status.clone();
    all.connect_clicked(move |_| {
        let needle = query.text().to_string();
        if needle.is_empty() { return; }
        let start = buffer.start_iter(); let end = buffer.end_iter();
        let original = buffer.text(&start, &end, true).to_string();
        let count = original.matches(&needle).count();
        if count == 0 { status.set_text("No matches found"); return; }
        buffer.begin_user_action();
        buffer.set_text(&original.replace(&needle, &replacement.text()));
        buffer.end_user_action();
        status.set_text(&format!("Replaced {count} occurrence(s)"));
    });
    dialog.present(); find.grab_focus();
}

fn collect_workspace_files(root: &Path, output: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 || output.len() >= 500 { return; }
    let Ok(entries) = fs::read_dir(root) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') || name.to_string_lossy() == "target" || name.to_string_lossy() == "node_modules" { continue; }
        if path.is_dir() { collect_workspace_files(&path, output, depth + 1); }
        else if path.is_file() { output.push(path); }
        if output.len() >= 500 { break; }
    }
}

fn show_quick_open(parent: &ApplicationWindow, state: &EditorState) {
    let (dialog, shell) = editor_dialog(parent, "Quick Open", 620, 470);
    let entry = gtk::SearchEntry::new(); entry.set_placeholder_text(Some("Search files by name or path"));
    entry.add_css_class("dialog-search");
    shell.append(&entry);
    let scroll = gtk::ScrolledWindow::new(); scroll.set_vexpand(true);
    scroll.add_css_class("dialog-scroll");
    let list = gtk::ListBox::new(); list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("dialog-list");
    scroll.set_child(Some(&list)); shell.append(&scroll);
    let mut files = state.recent_files.borrow().clone();
    let mut workspace = Vec::new(); collect_workspace_files(&state.root_dir.borrow(), &mut workspace, 0);
    for path in workspace { if !files.contains(&path) { files.push(path); } }
    files.truncate(500);
    let paths = Rc::new(files);
    let workspace_root = state.root_dir.borrow().clone();
    let rebuild = {
        let list = list.clone(); let paths = paths.clone(); let workspace_root = workspace_root.clone();
        move |filter: &str| {
            while let Some(child) = list.first_child() { list.remove(&child); }
            for path in paths.iter().filter(|path| path.to_string_lossy().to_lowercase().contains(&filter.to_lowercase())) {
                let label = gtk::Label::new(Some(&path.strip_prefix(&workspace_root).unwrap_or(path).display().to_string()));
                label.set_xalign(0.0); label.set_margin_top(7); label.set_margin_bottom(7); label.set_margin_start(9);
                list.append(&label);
            }
            if let Some(row) = list.row_at_index(0) { list.select_row(Some(&row)); }
        }
    };
    rebuild("");
    entry.connect_search_changed({ let rebuild = rebuild.clone(); move |input| rebuild(&input.text()) });
    let open_selected = {
        let paths = paths.clone(); let list = list.clone(); let state = state.clone(); let dialog = dialog.clone(); let entry_filter = entry.clone();
        move || {
            if let Some(row) = list.selected_row() {
                let index = row.index() as usize;
                let filtered: Vec<PathBuf> = paths.iter().filter(|path| path.to_string_lossy().to_lowercase().contains(&entry_filter.text().to_lowercase())).cloned().collect();
                if let Some(path) = filtered.get(index) { open_path(&state, path); dialog.close(); }
            }
        }
    };
    list.connect_row_activated({ let open_selected = open_selected.clone(); move |_, _| open_selected() });
    let buttons = dialog_button_row(); let open = dialog_button("Open", true); let close = dialog_button("Close", false);
    buttons.append(&close); buttons.append(&open); shell.append(&buttons);
    let d = dialog.clone(); close.connect_clicked(move |_| d.close());
    open.connect_clicked(move |_| open_selected());
    dialog.present(); entry.grab_focus();
}

fn show_editor_settings(parent: &ApplicationWindow, state: &EditorState) {
    let Some(doc) = active_editor(state) else { state.status.set_text("Open a file to edit its settings"); return; };
    let (dialog, shell) = editor_dialog(parent, "Editor Settings", 390, 260);
    let line_numbers = gtk::CheckButton::with_label("Show line numbers");
    line_numbers.set_active(true);
    let current_line = gtk::CheckButton::with_label("Highlight current line");
    let auto_indent = gtk::CheckButton::with_label("Automatic indentation"); auto_indent.set_active(true);
    let spaces = gtk::CheckButton::with_label("Insert spaces instead of tabs"); spaces.set_active(true);
    let width = gtk::SpinButton::with_range(1.0, 8.0, 1.0); width.set_value(4.0);
    shell.append(&line_numbers); shell.append(&current_line); shell.append(&auto_indent); shell.append(&spaces);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8); row.append(&gtk::Label::new(Some("Tab width"))); row.append(&width); shell.append(&row);
    let buttons = dialog_button_row(); let close = dialog_button("Close", true); buttons.append(&close); shell.append(&buttons);
    let view = doc.editor_view.clone(); line_numbers.connect_toggled({ let view = view.clone(); move |item| view.set_show_line_numbers(item.is_active()) });
    let view = doc.editor_view.clone(); current_line.connect_toggled(move |item| view.set_highlight_current_line(item.is_active()));
    let view = doc.editor_view.clone(); auto_indent.connect_toggled(move |item| view.set_auto_indent(item.is_active()));
    let view = doc.editor_view.clone(); spaces.connect_toggled(move |item| view.set_insert_spaces_instead_of_tabs(item.is_active()));
    let view = doc.editor_view.clone(); width.connect_value_changed(move |item| view.set_tab_width(item.value() as u32));
    let d = dialog.clone(); close.connect_clicked(move |_| d.close());
    dialog.present();
}

fn show_command_palette(parent: &ApplicationWindow, state: &EditorState) {
    let (dialog, shell) = editor_dialog(parent, "Command Palette", 520, 470);
    dialog.add_css_class("command-palette-window");
    let entry = gtk::SearchEntry::new(); entry.set_placeholder_text(Some("Type a command"));
    entry.add_css_class("dialog-search");
    shell.append(&entry);
    let list = gtk::ListBox::new(); list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("dialog-list");
    let commands: Vec<(&str, &str)> = vec![
        ("Quick Open", "quick"), ("Find & Replace", "find"), ("Go to Line", "line"),
        ("Duplicate Line", "duplicate"), ("Move Line Up", "up"), ("Move Line Down", "down"),
        ("Comment / Uncomment", "comment"), ("Editor Settings", "settings"), ("Recent Files", "recent"),
        ("Save", "save"), ("Open File", "open"), ("Open Folder", "folder"),
    ];
    let scroll = gtk::ScrolledWindow::new(); scroll.set_vexpand(true);
    scroll.add_css_class("dialog-scroll");
    scroll.set_child(Some(&list)); shell.append(&scroll);
    let rebuild = { let list = list.clone(); let commands = commands.clone(); move |filter: &str| {
        while let Some(child) = list.first_child() { list.remove(&child); }
        for (label, _) in commands.iter().filter(|(label, _)| label.to_lowercase().contains(&filter.to_lowercase())) {
            let row = gtk::Label::new(Some(label)); row.set_xalign(0.0); row.set_margin_top(8); row.set_margin_bottom(8); row.set_margin_start(10); list.append(&row);
        }
        if let Some(row) = list.row_at_index(0) { list.select_row(Some(&row)); }
    }};
    rebuild(""); entry.connect_search_changed({ let rebuild = rebuild.clone(); move |input| rebuild(&input.text()) });
    let execute = {
        let commands = commands.clone(); let list = list.clone(); let state = state.clone(); let parent = parent.clone(); let dialog = dialog.clone();
        move || {
            let Some(row) = list.selected_row() else { return; };
            let selected = row.child().and_downcast::<gtk::Label>().map(|label| label.text().to_string()).unwrap_or_default();
            let command = commands.iter().find(|(label, _)| *label == selected).map(|(_, command)| *command).unwrap_or("");
            dialog.close();
            match command {
                "quick" => show_quick_open(&parent, &state), "find" => show_find_replace(&parent, &state),
                "line" => show_goto_line(&parent, &state), "duplicate" => duplicate_line(&state),
                "up" => move_line(&state, -1), "down" => move_line(&state, 1), "comment" => toggle_comment(&state),
                "settings" => show_editor_settings(&parent, &state), "recent" => show_recent_files(&parent, &state),
                "save" => save_active(&parent, &state), "open" => choose_file(&parent, &state), "folder" => choose_folder(&parent, &state), _ => {}
            }
        }
    };
    list.connect_row_activated({ let execute = execute.clone(); move |_, _| execute() });
    let buttons = dialog_button_row(); let run = dialog_button("Run", true); let close = dialog_button("Close", false);
    buttons.append(&close); buttons.append(&run); shell.append(&buttons);
    let d = dialog.clone(); close.connect_clicked(move |_| d.close()); run.connect_clicked(move |_| execute());
    dialog.present(); entry.grab_focus();
}

fn show_recent_files(parent: &ApplicationWindow, state: &EditorState) {
    let (dialog, shell) = editor_dialog(parent, "Recent Files", 560, 400);
    let list = gtk::ListBox::new(); list.set_selection_mode(gtk::SelectionMode::Single);
    let paths = state.recent_files.borrow().clone();
    for path in &paths {
        let label = gtk::Label::new(Some(&path.display().to_string())); label.set_xalign(0.0);
        label.set_margin_top(7); label.set_margin_bottom(7); label.set_margin_start(8); list.append(&label);
    }
    let scroll = gtk::ScrolledWindow::new(); scroll.set_vexpand(true); scroll.set_child(Some(&list)); shell.append(&scroll);
    let buttons = dialog_button_row(); let open = dialog_button("Open", true); let close = dialog_button("Close", false);
    buttons.append(&close); buttons.append(&open); shell.append(&buttons);
    let d = dialog.clone(); close.connect_clicked(move |_| d.close());
    let state_open = state.clone(); let d = dialog.clone(); open.connect_clicked(move |_| {
        if let Some(row) = list.selected_row() { if let Some(path) = paths.get(row.index() as usize) { open_path(&state_open, path); d.close(); } }
    });
    dialog.present();
}

fn current_line_number(buffer: &sv::Buffer) -> i32 {
    buffer.iter_at_mark(&buffer.get_insert()).line()
}

fn replace_line_text(buffer: &sv::Buffer, line: i32, value: &str) {
    if let Some(mut start) = buffer.iter_at_line(line) {
        let mut end = start.clone();
        end.forward_to_line_end();
        buffer.begin_user_action(); buffer.delete(&mut start, &mut end);
        buffer.insert(&mut start, value); buffer.end_user_action();
        buffer.place_cursor(&start);
    }
}

fn duplicate_line(state: &EditorState) {
    let Some(doc) = active_editor(state) else { return; };
    let buffer = &doc.buffer; let line = current_line_number(buffer);
    if let Some(start) = buffer.iter_at_line(line) {
        let mut end = start.clone(); end.forward_to_line_end();
        let content = buffer.text(&start, &end, true).to_string();
        let mut insert_at = end;
        buffer.begin_user_action(); buffer.insert(&mut insert_at, &format!("\n{content}")); buffer.end_user_action();
        if let Some(cursor) = buffer.iter_at_line(line + 1) { buffer.place_cursor(&cursor); }
    }
}

fn move_line(state: &EditorState, direction: i32) {
    let Some(doc) = active_editor(state) else { return; };
    let buffer = &doc.buffer; let line = current_line_number(buffer); let target = line + direction;
    if target < 0 || target >= buffer.line_count() { return; }
    let start = buffer.iter_at_line(line).unwrap(); let mut end = start.clone(); end.forward_to_line_end();
    let mut lines: Vec<String> = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true).split('\n').map(str::to_string).collect();
    lines.swap(line as usize, target as usize);
    buffer.begin_user_action(); buffer.set_text(&lines.join("\n")); buffer.end_user_action();
    if let Some(cursor) = buffer.iter_at_line(target) { buffer.place_cursor(&cursor); }
}

fn toggle_comment(state: &EditorState) {
    let Some(doc) = active_editor(state) else { return; };
    let extension = doc.path.as_ref().and_then(|path| path.extension()).and_then(|ext| ext.to_str()).unwrap_or("");
    let prefix = match extension {
        "rs" | "c" | "h" | "cpp" | "hpp" | "js" | "jsx" | "ts" | "tsx" | "java" | "cs" | "go" | "swift" | "kt" => "//",
        "py" | "rb" | "sh" | "bash" | "yml" | "yaml" | "toml" | "r" => "#",
        "lua" => "--", "sql" | "hs" => "--", _ => { state.status.set_text("Comment syntax is not defined for this file type"); return; }
    };
    let buffer = &doc.buffer; let line = current_line_number(buffer);
    if let Some(start) = buffer.iter_at_line(line) {
        let mut end = start.clone(); end.forward_to_line_end(); let text = buffer.text(&start, &end, true).to_string();
        let trimmed = text.trim_start(); let leading = text.len() - trimmed.len();
        let updated = if trimmed.starts_with(prefix) {
            let remove_at = leading + prefix.len();
            let mut value = text.clone(); let remove_len = usize::from(value.as_bytes().get(remove_at) == Some(&b' ')); value.drain(remove_at..remove_at + remove_len); value
        } else { format!("{}{} {}", &text[..leading], prefix, trimmed) };
        replace_line_text(buffer, line, &updated);
    }
}

fn add_help_section(parent: &gtk::Box, title: &str, rows: &[(&str, &str)]) {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 5);
    section.add_css_class("help-section");
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("help-section-title");
    section.append(&heading);
    for (keys, description) in rows {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("shortcut-row");
        let key_label = gtk::Label::new(Some(keys));
        key_label.set_xalign(0.0);
        key_label.set_width_chars(19);
        key_label.set_selectable(true);
        key_label.add_css_class("shortcut-key");
        row.append(&key_label);
        let description_label = gtk::Label::new(Some(description));
        description_label.set_xalign(0.0);
        description_label.set_hexpand(true);
        description_label.set_wrap(true);
        description_label.add_css_class("shortcut-description");
        row.append(&description_label);
        section.append(&row);
    }
    parent.append(&section);
}

fn show_help(parent: &ApplicationWindow) {
    let dialog = gtk::Window::builder()
        .title("Feather Help")
        .transient_for(parent)
        .modal(true)
        .default_width(590)
        .default_height(610)
        .resizable(true)
        .build();
    dialog.add_css_class("help-window");

    let titlebar = gtk::HeaderBar::new();
    titlebar.add_css_class("feather-help-titlebar");
    titlebar.set_show_title_buttons(true);

    let title_brand = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    let title_logo = make_feather_logo(22);
    title_brand.append(&title_logo);

    let title_stack = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let title = gtk::Label::new(Some("Feather Help"));
    title.set_xalign(0.0);
    title.add_css_class("help-titlebar-title");
    title_stack.append(&title);
    let subtitle = gtk::Label::new(Some("Keyboard shortcuts and command-line usage"));
    subtitle.set_xalign(0.0);
    subtitle.add_css_class("help-titlebar-subtitle");
    title_stack.append(&subtitle);
    title_brand.append(&title_stack);
    titlebar.set_title_widget(Some(&title_brand));
    dialog.set_titlebar(Some(&titlebar));

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.add_css_class("help-shell");

    let divider = gtk::Separator::new(gtk::Orientation::Horizontal);
    divider.add_css_class("help-divider");
    outer.append(&divider);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.add_css_class("help-scroll");

    let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.set_margin_start(20);
    content.set_margin_end(20);
    content.set_margin_top(17);
    content.set_margin_bottom(18);

    add_help_section(&content, "FILE OPERATIONS", &[
        ("Ctrl + N", "Create a new file"),
        ("Ctrl + O", "Open a file"),
        ("Ctrl + S", "Save the active file"),
        ("Ctrl + Shift + S", "Save as…"),
        ("Ctrl + Shift + O", "Open a folder / workspace"),
    ]);
    add_help_section(&content, "EDITOR ESSENTIALS", &[
        ("Ctrl + Shift + P", "Open the command palette"),
        ("Ctrl + P", "Quick Open workspace files and recent files"),
        ("Ctrl + H", "Find and replace in the active document"),
        ("Ctrl + G", "Go to a line number"),
        ("Ctrl + Shift + D", "Duplicate the current line"),
        ("Alt + Shift + Up / Down", "Move the current line"),
        ("Ctrl + /", "Comment or uncomment the current line"),
    ]);
    add_help_section(&content, "TAB MANAGEMENT", &[
        ("Drag tab", "Reorder open tabs"),
        ("Close button", "Close the selected tab"),
    ]);
    add_help_section(&content, "EXPLORER NAVIGATION", &[
        ("Alt + Left", "Previous workspace folder"),
        ("Alt + Right", "Next workspace folder"),
        ("Alt + Up", "Open the parent folder as workspace"),
        ("Ctrl + Shift + R", "Refresh Explorer"),
    ]);
    add_help_section(&content, "APPLICATION", &[
        ("F1", "Show this help"),
    ]);
    add_help_section(&content, "TERMINAL", &[
        ("feather", "Open the current directory"),
        ("feather .", "Open the current directory explicitly"),
        ("feather ~/Projects", "Open a project folder"),
        ("feather file.rs", "Open a file"),
    ]);

    scroll.set_child(Some(&content));
    outer.append(&scroll);

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    footer.add_css_class("help-footer");
    footer.set_halign(gtk::Align::End);
    footer.set_margin_top(10);
    footer.set_margin_bottom(14);
    footer.set_margin_end(18);
    let close_button = gtk::Button::with_label("Close");
    close_button.add_css_class("help-close-button");
    close_button.set_size_request(92, 34);
    let dialog_close = dialog.clone();
    close_button.connect_clicked(move |_| dialog_close.close());
    footer.append(&close_button);
    outer.append(&footer);

    dialog.set_child(Some(&outer));
    dialog.present();
}

fn show_welcome(state: &EditorState, parent: &ApplicationWindow) {
    while let Some(child) = state.welcome_page.first_child() {
        state.welcome_page.remove(&child);
    }
    let center = gtk::Box::new(gtk::Orientation::Vertical, 12);
    center.set_halign(gtk::Align::Center);
    center.set_valign(gtk::Align::Center);
    center.set_margin_top(34);
    center.set_margin_bottom(34);
    center.set_margin_start(24);
    center.set_margin_end(24);
    center.set_size_request(360, -1);

    let logo = make_feather_logo(58);
    logo.add_css_class("welcome-logo");
    center.append(&logo);
    let heading = gtk::Label::new(Some("Feather"));
    heading.add_css_class("welcome-title");
    center.append(&heading);
    let subtitle = gtk::Label::new(Some("A lightweight code editor"));
    subtitle.add_css_class("welcome-subtitle");
    center.append(&subtitle);

    let actions = gtk::Box::new(gtk::Orientation::Vertical, 8);
    actions.set_margin_top(14);
    actions.add_css_class("welcome-actions");
    let new_btn = make_button("New File", "document-new-symbolic");
    new_btn.add_css_class("welcome-action");
    let open_btn = make_button("Open File", "document-open-symbolic");
    open_btn.add_css_class("welcome-action");
    let folder_btn = make_button("Open Folder", "folder-open-symbolic");
    folder_btn.add_css_class("welcome-action");
    actions.append(&new_btn);
    actions.append(&open_btn);
    actions.append(&folder_btn);
    center.append(&actions);

    let shortcuts = gtk::Label::new(Some(
        "Ctrl + N  New file     Ctrl + O  Open file\nCtrl + Shift + O  Open folder     F1  Help"
    ));
    shortcuts.add_css_class("welcome-shortcuts");
    shortcuts.set_justify(gtk::Justification::Center);
    shortcuts.set_margin_top(18);
    center.append(&shortcuts);

    state.welcome_page.append(&center);
    state.welcome_page.set_visible(true);
    state.notebook.set_visible(false);

    let state_new = state.clone();
    new_btn.connect_clicked(move |_| { if let Some(parent) = state_new.parent_window.borrow().clone() { prompt_new_file(&parent, &state_new); } });
    let state_open = state.clone();
    let parent_open = parent.clone();
    open_btn.connect_clicked(move |_| choose_file(&parent_open, &state_open));
    let state_folder = state.clone();
    let parent_folder = parent.clone();
    folder_btn.connect_clicked(move |_| choose_folder(&parent_folder, &state_folder));
}

fn add_action<F: Fn() + 'static>(app: &Application, name: &str, accelerator: &str, callback: F) {
    let action = gio::SimpleAction::new(name, None);
    action.connect_activate(move |_, _| callback());
    app.add_action(&action);
    app.set_accels_for_action(&format!("app.{name}"), &[accelerator]);
}

fn make_button(label: &str, icon_name: &str) -> gtk::Button {
    let button = gtk::Button::new();
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name(icon_name));
    content.append(&gtk::Label::new(Some(label)));
    button.set_child(Some(&content));
    button.add_css_class("toolbar-button");
    button
}

fn icon_button(icon_name: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon_name);
    button.set_tooltip_text(Some(tooltip));
    button.add_css_class("icon-button");
    button
}


fn apply_feather_syntax_scheme(buffer: &sv::Buffer) {
    let manager = sv::StyleSchemeManager::default();

    // Prefer the per-user installation path, then fall back to the source tree
    // so `cargo run` works without a separate installation step.
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
    {
        let user_styles = data_home.join("gtksourceview-5/styles");
        if user_styles.is_dir() {
            manager.prepend_search_path(&user_styles.to_string_lossy());
        }
    }

    if let Some(bundle_dir) = std::env::var_os("FEATHER_BUNDLE_DIR").map(PathBuf::from) {
        let bundled_styles = bundle_dir.join("gtksourceview-5/styles");
        if bundled_styles.is_dir() {
            manager.prepend_search_path(&bundled_styles.to_string_lossy());
        }
    }

    let source_styles = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/styles");
    if source_styles.is_dir() {
        manager.prepend_search_path(&source_styles.to_string_lossy());
    }

    manager.force_rescan();
    if let Some(scheme) = manager.scheme("feather-mocha") {
        buffer.set_style_scheme(Some(&scheme));
    }
}


fn create_file_monitor(
    buffer: &sv::Buffer,
    status: &gtk::Label,
    path: &Path,
) -> Option<gio::FileMonitor> {
    let monitor = gio::File::for_path(path)
        .monitor_file(gio::FileMonitorFlags::WATCH_MOVES, None::<&gio::Cancellable>)
        .ok()?;
    let monitored_buffer = buffer.clone();
    let status = status.clone();
    let monitored_path = path.to_path_buf();
    monitor.connect_changed(move |_, _, _, _| {
        match fs::read_to_string(&monitored_path) {
            Ok(disk_text) => {
                let start = monitored_buffer.start_iter();
                let end = monitored_buffer.end_iter();
                let current = monitored_buffer.text(&start, &end, true);
                if current.as_str() == disk_text { return; }
                if monitored_buffer.is_modified() {
                    status.set_text("External change detected — your unsaved edits are preserved");
                } else {
                    monitored_buffer.set_text(&disk_text);
                    monitored_buffer.set_modified(false);
                    status.set_text("File changed externally — reloaded");
                }
            }
            Err(err) => status.set_text(&format!("File monitor: {err}")),
        }
    });
    Some(monitor)
}

fn dismiss_visible_popovers(widget: &gtk::Widget) {
    let mut child = widget.first_child();
    while let Some(current) = child {
        let next = current.next_sibling();
        if let Ok(popover) = current.clone().downcast::<gtk::Popover>() {
            if popover.is_visible() {
                popover.popdown();
            }
        } else {
            dismiss_visible_popovers(&current);
        }
        child = next;
    }
}

fn create_document(path: Option<PathBuf>, text: &str) -> Document {
    let buffer = sv::Buffer::new(None);
    buffer.set_text(text);
    buffer.set_modified(false);
    buffer.set_highlight_syntax(true);
    buffer.set_highlight_matching_brackets(true);
    apply_feather_syntax_scheme(&buffer);
    let manager = sv::LanguageManager::default();
    if let Some(ref p) = path {
        if let Some(language) = manager.guess_language(Some(p), None) {
            buffer.set_language(Some(&language));
        }
    }
    let view = sv::View::with_buffer(&buffer);
    view.set_show_line_numbers(true);
    view.set_show_line_marks(true);
    view.set_highlight_current_line(false);
    view.set_auto_indent(true);
    view.set_indent_width(4);
    view.set_tab_width(4);
    view.set_insert_spaces_instead_of_tabs(true);
    view.set_monospace(true);
    view.set_wrap_mode(gtk::WrapMode::None);
    view.set_left_margin(12);
    view.set_right_margin(12);
    view.set_top_margin(10);
    view.set_bottom_margin(10);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&view));
    Document {
        path,
        buffer,
        page: scroll.upcast(),
        tab_title: gtk::Label::new(Some("Untitled")),
        editor_view: view,
        _file_monitor: None,
    }
}

fn make_dialog(parent: &ApplicationWindow, title: &str, width: i32) -> (gtk::Window, gtk::Box) {
    let dialog = gtk::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .decorated(false)
        .resizable(false)
        .default_width(width)
        .build();
    dialog.add_css_class("feather-dialog-window");
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.add_css_class("dialog-shell");
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("dialog-title");
    shell.append(&heading);
    dialog.set_child(Some(&shell));
    (dialog, shell)
}

fn dialog_button_row() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("dialog-buttons");
    row
}

fn dialog_button(label: &str, primary: bool) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("dialog-button");
    if primary { button.add_css_class("dialog-button-primary"); }
    button
}

fn prompt_new_file(parent: &ApplicationWindow, state: &EditorState) {
    let (dialog, shell) = make_dialog(parent, "Create new file", 410);
    let description = gtk::Label::new(Some("Enter a filename including its extension."));
    description.set_xalign(0.0);
    description.add_css_class("dialog-message");
    shell.append(&description);
    let entry = gtk::Entry::new();
    entry.set_text("untitled.txt");
    entry.set_placeholder_text(Some("example.rs"));
    entry.add_css_class("dialog-entry");
    entry.set_margin_top(12);
    shell.append(&entry);
    let buttons = dialog_button_row();
    let cancel = dialog_button("Cancel", false);
    let create = dialog_button("Create File", true);
    buttons.append(&cancel);
    buttons.append(&create);
    shell.append(&buttons);

    let dialog_cancel = dialog.clone();
    cancel.connect_clicked(move |_| dialog_cancel.close());
    let dialog_create = dialog.clone();
    let state_create = state.clone();
    let parent_create = parent.clone();
    let entry_create = entry.clone();
    create.connect_clicked(move |_| {
        let raw_name = entry_create.text().trim().to_string();
        let file_name = Path::new(&raw_name);
        if raw_name.is_empty() || file_name.file_name().and_then(|name| name.to_str()) != Some(raw_name.as_str()) {
            show_error_for(Some(parent_create.upcast_ref()), "Invalid filename", "Enter a filename only, without folder paths.");
            return;
        }
        if file_name.extension().is_none() {
            show_error_for(Some(parent_create.upcast_ref()), "File extension required", "Include the file format in the name, for example main.rs, app.py or notes.txt.");
            return;
        }
        let path = state_create.root_dir.borrow().join(&raw_name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => {
                dialog_create.close();
                open_path(&state_create, &path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                show_error_for(Some(parent_create.upcast_ref()), "File already exists", "Choose a different filename, or open the existing file from the Explorer.");
            }
            Err(error) => show_error_for(Some(parent_create.upcast_ref()), "Could not create file", &error.to_string()),
        }
    });
    dialog.present();
    entry.grab_focus();
    entry.select_region(0, -1);
}

fn new_document(state: &EditorState, path: Option<PathBuf>, text: &str) {
    state.welcome_page.set_visible(false);
    state.notebook.set_visible(true);
    let mut doc = create_document(path.clone(), text);
    let name = path.as_ref().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or("Untitled");
    doc.tab_title.set_text(name);
    let title_for_modified = doc.tab_title.clone();
    doc.buffer.connect_modified_changed(move |buffer| {
        let title = title_for_modified.text().trim_end_matches(" •").to_string();
        let display_title = if buffer.is_modified() {
            format!("{title} •")
        } else {
            title
        };
        title_for_modified.set_text(&display_title);
    });

    if let Some(file_path) = path.clone() {
        doc._file_monitor = create_file_monitor(&doc.buffer, &state.status, &file_path);
    }

    let tab = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    tab.add_css_class("tab-label");
    doc.tab_title.add_css_class("tab-title");
    tab.append(&doc.tab_title);

    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.add_css_class("tab-close");
    close.set_focusable(false);
    close.set_tooltip_text(Some("Close tab"));
    tab.append(&close);

    let page = doc.page.clone();
    let page_num = state.notebook.append_page(&doc.page, Some(&tab));
    state.notebook.set_tab_reorderable(&doc.page, true);
    state.documents.borrow_mut().push(doc);

    let state_close = state.clone();
    close.connect_clicked(move |_| close_document(&state_close, &page));

    state.notebook.set_current_page(Some(page_num));
    update_status(state, page_num as usize);
}

fn open_path(state: &EditorState, path: &Path) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Some(doc) = state.documents.borrow().iter().find(|doc| {
        doc.path.as_ref().map(|p| p.canonicalize().unwrap_or_else(|_| p.clone())) == Some(canonical.clone())
    }).cloned() {
        if let Some(index) = state.notebook.page_num(&doc.page) {
            state.notebook.set_current_page(Some(index));
        }
        return;
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => { show_error_for(state.parent_window.borrow().as_ref().map(|w| w.upcast_ref()), "Could not inspect file", &err.to_string()); return; }
    };
    if !metadata.is_file() {
        show_error_for(state.parent_window.borrow().as_ref().map(|w| w.upcast_ref()), "Cannot open file", "The selected path is not a regular file.");
        return;
    }
    if metadata.len() > MAX_TEXT_FILE_BYTES {
        show_error_for(state.parent_window.borrow().as_ref().map(|w| w.upcast_ref()),
            "File too large",
            &format!(
                "Feather's safe text-file limit is {}. This file is {}. Open it with a dedicated large-file tool instead.",
                format_bytes(MAX_TEXT_FILE_BYTES),
                format_bytes(metadata.len())
            )
        );
        return;
    }
    match is_binary_file(path) {
        Ok(true) => {
            show_error_for(state.parent_window.borrow().as_ref().map(|w| w.upcast_ref()), "Binary file", "This file contains binary data and cannot be opened as text.");
            return;
        }
        Err(err) => { show_error_for(state.parent_window.borrow().as_ref().map(|w| w.upcast_ref()), "Could not inspect file", &err.to_string()); return; }
        Ok(false) => {}
    }
    match fs::read_to_string(path) {
        Ok(contents) => {
            remember_recent_file(state, canonical.clone());
            new_document(state, Some(canonical), &contents);
        },
        Err(err) => show_error_for(state.parent_window.borrow().as_ref().map(|w| w.upcast_ref()), "Could not open file", &err.to_string()),
    }
}

fn choose_file(parent: &ApplicationWindow, state: &EditorState) {
    let dialog = gtk::FileChooserNative::new(Some("Open File"), Some(parent), gtk::FileChooserAction::Open, Some("Open"), Some("Cancel"));
    let state = state.clone();
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(path) = dialog.file().and_then(|f| f.path()) { open_path(&state, &path); }
        }
        dialog.destroy();
    });
    dialog.show();
}

fn choose_folder(parent: &ApplicationWindow, state: &EditorState) {
    let dialog = gtk::FileChooserNative::new(Some("Open Folder"), Some(parent), gtk::FileChooserAction::SelectFolder, Some("Open Folder"), Some("Cancel"));
    let state = state.clone();
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(path) = dialog.file().and_then(|f| f.path()) { set_workspace(&state, path, true); }
        }
        dialog.destroy();
    });
    dialog.show();
}

fn set_workspace(state: &EditorState, path: PathBuf, remember: bool) {
    let path = path.canonicalize().unwrap_or(path);
    if !path.is_dir() { return; }
    *state.root_dir.borrow_mut() = path.clone();
    save_last_workspace(&path);
    if remember {
        let mut history = state.workspace_history.borrow_mut();
        let index = state.history_index.get();
        history.truncate(index + 1);
        if history.last() != Some(&path) { history.push(path); }
        state.history_index.set(history.len().saturating_sub(1));
    }
    refresh_explorer(state);
}

fn navigate_history(state: &EditorState, delta: isize) {
    let history_len = state.workspace_history.borrow().len();
    let current = state.history_index.get() as isize;
    let next = current + delta;
    if next < 0 || next >= history_len as isize { return; }
    state.history_index.set(next as usize);
    if let Some(path) = state.workspace_history.borrow().get(next as usize).cloned() {
        *state.root_dir.borrow_mut() = path;
        refresh_explorer(state);
    }
}

fn active_document(state: &EditorState) -> Option<Document> {
    let page_num = state.notebook.current_page()?;
    let page = state.notebook.nth_page(Some(page_num))?;
    state.documents.borrow().iter().find(|doc| doc.page == page).cloned()
}

fn remove_document(state: &EditorState, page: &gtk::Widget) {
    let Some(page_num) = state.notebook.page_num(page) else { return; };
    state.notebook.remove_page(Some(page_num));
    state.documents.borrow_mut().retain(|doc| doc.page != page.clone());
    if let Some(current) = state.notebook.current_page() {
        update_status(state, current as usize);
    } else {
        state.status.set_text("Ready");
        state.notebook.set_visible(false);
        state.welcome_page.set_visible(true);
    }
}

fn close_document(state: &EditorState, page: &gtk::Widget) {
    let doc = state.documents.borrow().iter().find(|doc| doc.page == *page).cloned();
    let Some(doc) = doc else { return; };
    if !doc.buffer.is_modified() {
        remove_document(state, page);
        return;
    }
    let Some(parent) = state.parent_window.borrow().clone() else { return; };
    let (dialog, shell) = make_dialog(&parent, "Unsaved changes", 390);
    let message = gtk::Label::new(Some(&format!(
        "Save changes to {} before closing?",
        doc.path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("Untitled")
    )));
    message.set_wrap(true);
    message.set_xalign(0.0);
    message.add_css_class("dialog-message");
    shell.append(&message);
    let buttons = dialog_button_row();
    let cancel = dialog_button("Cancel", false);
    let discard = dialog_button("Discard", false);
    let save = dialog_button("Save", true);
    buttons.append(&cancel);
    buttons.append(&discard);
    buttons.append(&save);
    shell.append(&buttons);

    let dialog_cancel = dialog.clone();
    cancel.connect_clicked(move |_| dialog_cancel.close());
    let dialog_discard = dialog.clone();
    let state_discard = state.clone();
    let page_discard = page.clone();
    let doc_discard = doc.clone();
    discard.connect_clicked(move |_| {
        doc_discard.buffer.set_modified(false);
        remove_document(&state_discard, &page_discard);
        dialog_discard.close();
    });
    let dialog_save = dialog.clone();
    let state_save = state.clone();
    let page_save = page.clone();
    let doc_save = doc.clone();
    let parent_save = parent.clone();
    save.connect_clicked(move |_| {
        if let Some(path) = doc_save.path.as_ref() {
            if write_document(&doc_save, path) {
                doc_save.buffer.set_modified(false);
                remove_document(&state_save, &page_save);
            }
        } else {
            save_as(&parent_save, &state_save);
        }
        dialog_save.close();
    });
    dialog.present();
}

fn request_close_application(window: &ApplicationWindow, state: &EditorState) {
    let dirty: Vec<Document> = state.documents.borrow()
        .iter().filter(|doc| doc.buffer.is_modified()).cloned().collect();
    if dirty.is_empty() { return; }
    let names = dirty.iter().map(|doc| {
        doc.path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("Untitled")
    }).collect::<Vec<_>>().join(", ");
    let (dialog, shell) = make_dialog(window, "Unsaved changes", 430);
    let message = gtk::Label::new(Some(&format!("Save changes to these files before exiting?\n{names}")));
    message.set_wrap(true);
    message.set_xalign(0.0);
    message.add_css_class("dialog-message");
    shell.append(&message);
    let buttons = dialog_button_row();
    let cancel = dialog_button("Cancel", false);
    let discard = dialog_button("Discard All", false);
    let save = dialog_button("Save All", true);
    buttons.append(&cancel);
    buttons.append(&discard);
    buttons.append(&save);
    shell.append(&buttons);

    let dialog_cancel = dialog.clone();
    cancel.connect_clicked(move |_| dialog_cancel.close());
    let dialog_discard = dialog.clone();
    let window_discard = window.clone();
    let state_discard = state.clone();
    discard.connect_clicked(move |_| {
        for doc in state_discard.documents.borrow().iter() { doc.buffer.set_modified(false); }
        dialog_discard.close();
        window_discard.close();
    });
    let dialog_save = dialog.clone();
    let state_save = state.clone();
    let parent_save = window.clone();
    let error_parent = window.clone();
    save.connect_clicked(move |_| {
        let docs = state_save.documents.borrow().clone();
        if docs.iter().any(|doc| doc.buffer.is_modified() && doc.path.is_none()) {
            dialog_save.close();
            show_error_for(Some(error_parent.upcast_ref()), "Save As required", "At least one modified document has no filename. Use Save As, then exit again.");
            return;
        }
        let mut all_saved = true;
        for doc in docs.iter().filter(|doc| doc.buffer.is_modified()) {
            if let Some(path) = doc.path.as_ref() {
                if write_document(doc, path) { doc.buffer.set_modified(false); } else { all_saved = false; }
            }
        }
        dialog_save.close();
        if all_saved { parent_save.close(); }
    });
    dialog.present();
}

fn save_active(parent: &ApplicationWindow, state: &EditorState) {
    if let Some(doc) = active_document(state) {
        if let Some(ref path) = doc.path {
            write_document(&doc, path);
        } else {
            save_as(parent, state);
        }
    }
}

fn save_as(parent: &ApplicationWindow, state: &EditorState) {
    let dialog = gtk::FileChooserNative::new(Some("Save File"), Some(parent), gtk::FileChooserAction::Save, Some("Save"), Some("Cancel"));
    dialog.set_current_name("untitled.txt");
    let state = state.clone();
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let (Some(path), Some(doc)) = (dialog.file().and_then(|f| f.path()), active_document(&state)) {
                if !write_document(&doc, &path) { return; }
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("Untitled");
                doc.tab_title.set_text(name);
                if let Some(existing) = state.documents.borrow_mut().iter_mut().find(|item| item.page == doc.page) {
                    existing.path = Some(path.clone());
                    existing._file_monitor = create_file_monitor(&existing.buffer, &state.status, &path);
                }
                if let Some(language) = sv::LanguageManager::default().guess_language(Some(&path), None) {
                    doc.buffer.set_language(Some(&language));
                }
            }
        }
        dialog.destroy();
    });
    dialog.show();
}

fn write_document(doc: &Document, path: &Path) -> bool {
    let start = doc.buffer.start_iter();
    let end = doc.buffer.end_iter();
    let text = doc.buffer.text(&start, &end, true);
    match fs::write(path, text.as_str()) {
        Ok(()) => { doc.buffer.set_modified(false); true }
        Err(err) => {
            let root = doc.page.root().and_then(|root| root.downcast::<gtk::Window>().ok());
            show_error_for(root.as_ref(), "Could not save file", &err.to_string());
            false
        }
    }
}

fn refresh_explorer(state: &EditorState) {
    while let Some(child) = state.explorer_tree.first_child() { state.explorer_tree.remove(&child); }
    let root = state.root_dir.borrow().clone();
    state.root_label.set_text(&root.display().to_string());
    populate_directory(&state.explorer_tree, &root, state, 0);
}

fn populate_directory(container: &gtk::Box, directory: &Path, state: &EditorState, depth: usize) {
    if depth > 64 { return; }
    let mut entries: Vec<_> = match fs::read_dir(directory) {
        Ok(read_dir) => read_dir.filter_map(Result::ok).collect(),
        Err(err) => {
            if depth == 0 { show_error_for(state.parent_window.borrow().as_ref().map(|w| w.upcast_ref()), "Cannot read workspace folder", &err.to_string()); }
            return;
        }
    };
    entries.retain(|entry| {
        state.show_hidden.get() || !entry.file_name().to_string_lossy().starts_with('.')
    });
    entries.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| a.file_name().to_string_lossy().to_lowercase().cmp(&b.file_name().to_string_lossy().to_lowercase()))
    });

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 0);
            row.add_css_class("tree-node");
            let toggle = gtk::Button::new();
            toggle.add_css_class("tree-row");
            toggle.set_has_frame(false);
            toggle.set_hexpand(true);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            line.set_margin_start((depth as i32) * 14 + 2);
            line.set_margin_top(1);
            line.set_margin_bottom(1);
            let chevron = gtk::Image::from_icon_name("pan-end-symbolic");
            chevron.add_css_class("tree-chevron");
            line.append(&chevron);
            let icon = vscode_folder_image(&name, false);
            icon.add_css_class("folder-icon");
            line.append(&icon);
            let label = gtk::Label::new(Some(&name));
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            label.set_hexpand(true);
            line.append(&label);
            toggle.set_child(Some(&line));
            row.append(&toggle);

            let children = gtk::Box::new(gtk::Orientation::Vertical, 0);
            children.add_css_class("tree-children");
            children.set_visible(false);
            row.append(&children);
            let expanded = Rc::new(Cell::new(false));
            let state_click = state.clone();
            let path_click = path.clone();
            let children_click = children.clone();
            let chevron_click = chevron.clone();
            let expanded_click = expanded.clone();
            toggle.connect_clicked(move |_| {
                let opening = !expanded_click.get();
                expanded_click.set(opening);
                children_click.set_visible(opening);
                chevron_click.set_icon_name(Some(if opening { "pan-down-symbolic" } else { "pan-end-symbolic" }));
                if opening && children_click.first_child().is_none() {
                    populate_directory(&children_click, &path_click, &state_click, depth + 1);
                }
            });
            container.append(&row);
        } else {
            let button = gtk::Button::new();
            button.add_css_class("tree-row");
            button.set_has_frame(false);
            button.set_hexpand(true);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            line.set_margin_start((depth as i32) * 14 + 22);
            line.set_margin_top(1);
            line.set_margin_bottom(1);
            let icon_id = file_icon(&name);
            let icon = vscode_file_image(icon_id);
            icon.add_css_class("file-icon");
            line.append(&icon);
            let label = gtk::Label::new(Some(&name));
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            label.set_hexpand(true);
            line.append(&label);
            button.set_child(Some(&line));
            let state_open = state.clone();
            let selected_button = button.clone();
            button.connect_clicked(move |_| {
                if let Some(previous) = state_open.selected_file_row.borrow_mut().take() {
                    previous.remove_css_class("selected");
                }
                selected_button.add_css_class("selected");
                *state_open.selected_file_row.borrow_mut() = Some(selected_button.clone());
                open_path(&state_open, &path);
            });
            container.append(&button);
        }
    }
}

fn file_icon(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match lower.as_str() {
        "dockerfile" => "docker",
        "makefile" | "justfile" => "makefile",
        ".gitignore" | ".gitattributes" | ".gitmodules" => "git",
        ".editorconfig" => "editorconfig",
        "cargo.toml" | "cargo.lock" => "cargo",
        "package.json" => "npm",
        "tsconfig.json" => "tsconfig",
        _ => match ext.as_str() {
            "rs" => "rust",
            "py" | "pyw" => "python",
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "ts" | "tsx" => "typescript",
            "html" | "htm" | "xhtml" => "html",
            "xml" => "xml",
            "css" | "scss" | "sass" | "less" => "css",
            "json" | "jsonc" => "json",
            "toml" => "toml",
            "yaml" | "yml" => "yaml",
            "ini" | "conf" | "cfg" | "env" => "config",
            "lock" => "lock",
            "md" | "markdown" => "markdown",
            "rst" | "txt" | "adoc" => "document",
            "sh" | "bash" | "zsh" | "fish" => "shell",
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "avif" => "image",
            "svg" => "svg",
            "c" | "h" => "c",
            "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
            "go" => "go",
            "java" => "java",
            "kt" | "kts" => "kotlin",
            "swift" => "swift",
            "cs" => "csharp",
            "rb" => "ruby",
            "php" => "php",
            "lua" => "lua",
            "pl" => "perl",
            "vue" => "vue",
            "svelte" => "svelte",
            "astro" => "astro",
            "pdf" => "pdf",
            "mp3" | "wav" | "ogg" | "flac" => "audio",
            "mp4" | "mkv" | "webm" | "mov" => "video",
            "zip" | "rar" | "7z" | "gz" | "tar" | "xz" => "zip",
            _ => "file",
        }
    }
}

fn update_status(state: &EditorState, index: usize) {
    if let Some(page) = state.notebook.nth_page(Some(index as u32)) {
      if let Some(doc) = state.documents.borrow().iter().find(|doc| doc.page == page) {
        let iter = doc.buffer.iter_at_mark(&doc.buffer.get_insert());
        let line = iter.line() + 1;
        let column = iter.line_offset() + 1;
        let language = doc.buffer.language().map(|l| l.name().to_string()).unwrap_or_else(|| "Plain Text".to_string());
        state.status.set_text(&format!("{}  ·  Ln {}, Col {}  ·  UTF-8  ·  LF", language, line, column));
      }
    }
}

fn show_error_for(parent: Option<&gtk::Window>, title: &str, message: &str) {
    let dialog = gtk::Window::builder()
        .title(title)
        .modal(true)
        .decorated(false)
        .resizable(false)
        .default_width(390)
        .build();
    if let Some(parent) = parent { dialog.set_transient_for(Some(parent)); }
    dialog.add_css_class("feather-dialog-window");
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.add_css_class("dialog-shell");
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("dialog-title");
    shell.append(&heading);
    let body = gtk::Label::new(Some(message));
    body.set_wrap(true);
    body.set_xalign(0.0);
    body.add_css_class("dialog-message");
    shell.append(&body);
    let buttons = dialog_button_row();
    let close = dialog_button("Close", true);
    buttons.append(&close);
    shell.append(&buttons);
    dialog.set_child(Some(&shell));
    let dialog_close = dialog.clone();
    close.connect_clicked(move |_| dialog_close.close());
    dialog.present();
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_detection_flags_null_bytes() {
        assert!(is_binary_bytes(b"plain text\0with nul"));
        assert!(!is_binary_bytes(b"plain text source code"));
    }

    #[test]
    fn binary_detection_only_scans_probe_window() {
        let mut bytes = vec![b'a'; BINARY_PROBE_BYTES + 10];
        bytes[BINARY_PROBE_BYTES + 2] = 0;
        assert!(!is_binary_bytes(&bytes));
        bytes[BINARY_PROBE_BYTES - 1] = 0;
        assert!(is_binary_bytes(&bytes));
    }

    #[test]
    fn byte_size_format_is_readable() {
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn size_limit_is_ten_mib() {
        assert_eq!(MAX_TEXT_FILE_BYTES, 10 * 1024 * 1024);
    }

    #[test]
    fn all_documented_actions_have_unique_shortcuts() {
        let mut actions = std::collections::HashSet::new();
        let mut accelerators = std::collections::HashSet::new();
        for (action, accelerator) in SHORTCUT_BINDINGS {
            assert!(actions.insert(*action), "duplicate action: {action}");
            assert!(accelerators.insert(*accelerator), "duplicate shortcut: {accelerator}");
            assert_eq!(shortcut_accelerator(action), *accelerator);
        }
        assert_eq!(SHORTCUT_BINDINGS.len(), 18);
    }

    #[test]
    fn expected_core_shortcuts_are_registered() {
        for action in ["new", "open", "save", "save-as", "help", "open-folder",
                       "explorer-back", "explorer-forward", "explorer-up", "explorer-refresh",
                       "command-palette", "quick-open", "find-replace", "goto-line",
                       "duplicate-line", "move-line-up", "move-line-down", "toggle-comment"] {
            assert!(!shortcut_accelerator(action).is_empty());
        }
    }

    #[test]
    fn vscode_icon_asset_names_follow_upstream_convention() {
        assert_eq!(format!("file_type_{}.svg", file_icon("main.rs")), "file_type_rust.svg");
        assert_eq!(format!("file_type_{}.svg", file_icon("app.py")), "file_type_python.svg");
        assert_eq!(format!("file_type_{}.svg", file_icon("app.js")), "file_type_javascript.svg");
        assert_eq!(format!("file_type_{}.svg", file_icon("app.ts")), "file_type_typescript.svg");
    }

    #[test]
    fn extension_icon_mapping_uses_custom_assets() {
        for (filename, expected) in [
            ("main.rs", "rust"),
            ("app.py", "python"),
            ("app.js", "javascript"),
            ("app.ts", "typescript"),
            ("index.html", "html"),
            ("style.css", "css"),
            ("data.json", "json"),
            ("settings.toml", "toml"),
            ("README.md", "markdown"),
            ("run.sh", "shell"),
            ("photo.png", "image"),
            ("engine.cpp", "cpp"),
        ] {
            assert_eq!(file_icon(filename), expected);
        }
    }

    #[test]
    #[ignore = "requires a running GTK display; run manually on Ubuntu with a desktop session"]
    fn gtk_notebook_supports_reorderable_tabs() {
        gtk::init().expect("GTK display required");
        let notebook = gtk::Notebook::new();
        let first = gtk::Label::new(Some("first"));
        let second = gtk::Label::new(Some("second"));
        notebook.append_page(&first, None::<&gtk::Widget>);
        notebook.append_page(&second, None::<&gtk::Widget>);
        notebook.set_tab_reorderable(&first, true);
        notebook.set_tab_reorderable(&second, true);
        notebook.reorder_child(&second, Some(0));
        assert_eq!(notebook.page_num(&second), Some(0));
        assert_eq!(notebook.page_num(&first), Some(1));
    }
}
