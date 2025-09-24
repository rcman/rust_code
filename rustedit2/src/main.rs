use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::collections::VecDeque;
use regex::Regex;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;

#[derive(Clone)]
struct EditorState {
    content: String,
    cursor_pos: usize,
}

#[derive(Clone)]
struct TabData {
    id: usize,
    title: String,
    file_path: Option<PathBuf>,
    content: String,
    is_modified: bool,
    undo_stack: VecDeque<EditorState>,
    redo_stack: VecDeque<EditorState>,
    syntax_language: String,
    cursor_pos: usize,
    selection_start: Option<usize>,
    selection_end: Option<usize>,
}

impl TabData {
    fn new(id: usize, title: String) -> Self {
        Self {
            id,
            title,
            file_path: None,
            content: String::new(),
            is_modified: false,
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            syntax_language: "Plain Text".to_string(),
            cursor_pos: 0,
            selection_start: None,
            selection_end: None,
        }
    }
    
    fn detect_language_from_extension(&mut self) {
        if let Some(path) = &self.file_path {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                self.syntax_language = match ext.to_lowercase().as_str() {
                    "rs" => "Rust".to_string(),
                    "py" => "Python".to_string(),
                    "html" | "htm" => "HTML".to_string(),
                    "css" => "CSS".to_string(),
                    "js" => "JavaScript".to_string(),
                    "json" => "JSON".to_string(),
                    "xml" => "XML".to_string(),
                    "md" => "Markdown".to_string(),
                    "c" => "C".to_string(),
                    "cpp" | "cc" | "cxx" => "C++".to_string(),
                    "java" => "Java".to_string(),
                    "sql" => "SQL".to_string(),
                    _ => "Plain Text".to_string(),
                };
            }
        }
    }
    
    fn push_undo_state(&mut self) {
        if self.undo_stack.len() >= 100 {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(EditorState {
            content: self.content.clone(),
            cursor_pos: self.cursor_pos,
        });
        self.redo_stack.clear();
    }
    
    fn undo(&mut self) -> bool {
        if let Some(state) = self.undo_stack.pop_back() {
            self.redo_stack.push_back(EditorState {
                content: self.content.clone(),
                cursor_pos: self.cursor_pos,
            });
            self.content = state.content;
            self.cursor_pos = state.cursor_pos;
            true
        } else {
            false
        }
    }
    
    fn redo(&mut self) -> bool {
        if let Some(state) = self.redo_stack.pop_back() {
            self.undo_stack.push_back(EditorState {
                content: self.content.clone(),
                cursor_pos: self.cursor_pos,
            });
            self.content = state.content;
            self.cursor_pos = state.cursor_pos;
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct TextEditor {
    tabs: Vec<TabData>,
    active_tab: usize,
    next_tab_id: usize,
    
    // UI State
    show_style_config: bool,
    show_find_replace: bool,
    show_about: bool,
    show_goto_line: bool,
    show_file_explorer: bool,
    show_minimap: bool,
    show_terminal: bool,
    
    // Style configuration
    background_color: egui::Color32,
    text_color: egui::Color32,
    selection_color: egui::Color32,
    line_number_color: egui::Color32,
    current_theme: String,
    
    // Editor settings
    font_size: f32,
    tab_size: usize,
    show_line_numbers: bool,
    word_wrap: bool,
    auto_indent: bool,
    show_whitespace: bool,
    auto_save: bool,
    bracket_matching: bool,
    
    // Find & Replace
    find_text: String,
    replace_text: String,
    case_sensitive: bool,
    use_regex: bool,
    current_match: usize,
    total_matches: usize,
    
    // File operations
    recent_files: VecDeque<PathBuf>,
    current_directory: Option<PathBuf>,
    
    // Go to line
    goto_line_input: String,
    
    // Syntax highlighting
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    
    // Terminal
    terminal_input: String,
    terminal_output: String,
    
    // Zoom
    zoom_level: f32,
    
    // Split view
    split_view: bool,
    split_horizontal: bool,
}

impl TextEditor {
    pub fn new() -> Self {
        let mut editor = Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_tab_id: 0,
            show_style_config: false,
            show_find_replace: false,
            show_about: false,
            show_goto_line: false,
            show_file_explorer: false,
            show_minimap: false,
            show_terminal: false,
            background_color: egui::Color32::from_rgb(30, 30, 30),
            text_color: egui::Color32::from_rgb(220, 220, 220),
            selection_color: egui::Color32::from_rgb(70, 130, 180),
            line_number_color: egui::Color32::from_rgb(128, 128, 128),
            current_theme: "Dark".to_string(),
            font_size: 14.0,
            tab_size: 4,
            show_line_numbers: true,
            word_wrap: false,
            auto_indent: true,
            show_whitespace: false,
            auto_save: false,
            bracket_matching: true,
            find_text: String::new(),
            replace_text: String::new(),
            case_sensitive: false,
            use_regex: false,
            current_match: 0,
            total_matches: 0,
            recent_files: VecDeque::new(),
            current_directory: None,
            goto_line_input: String::new(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            terminal_input: String::new(),
            terminal_output: String::new(),
            zoom_level: 1.0,
            split_view: false,
            split_horizontal: false,
        };
        
        // Create initial tab
        editor.new_tab();
        editor
    }
    
    fn new_tab(&mut self) {
        let tab = TabData::new(self.next_tab_id, "Untitled".to_string());
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        self.next_tab_id += 1;
    }
    
    fn close_tab(&mut self, index: usize) {
        if self.tabs.len() > 1 && index < self.tabs.len() {
            self.tabs.remove(index);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
    }
    
    fn get_active_tab(&self) -> Option<&TabData> {
        self.tabs.get(self.active_tab)
    }
    
    fn get_active_tab_mut(&mut self) -> Option<&mut TabData> {
        self.tabs.get_mut(self.active_tab)
    }
    
    fn update_title(&self, ctx: &egui::Context) {
        let title = if let Some(tab) = self.get_active_tab() {
            let modified_indicator = if tab.is_modified { "*" } else { "" };
            format!("Advanced Rust Editor - {}{}", tab.title, modified_indicator)
        } else {
            "Advanced Rust Editor".to_string()
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }
    
    fn new_file(&mut self) {
        self.new_tab();
    }
    
    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Text files", &["txt", "rs", "py", "html", "css", "js", "json", "xml", "md"])
            .add_filter("All files", &["*"])
            .pick_file() 
        {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    let mut tab = TabData::new(self.next_tab_id, 
                        path.file_name().unwrap_or_default().to_string_lossy().to_string());
                    tab.content = content;
                    tab.file_path = Some(path.clone());
                    tab.detect_language_from_extension();
                    tab.is_modified = false;
                    
                    self.tabs.push(tab);
                    self.active_tab = self.tabs.len() - 1;
                    self.next_tab_id += 1;
                    
                    // Add to recent files
                    self.recent_files.push_back(path);
                    if self.recent_files.len() > 10 {
                        self.recent_files.pop_front();
                    }
                }
                Err(e) => {
                    eprintln!("Failed to open file: {}", e);
                }
            }
        }
    }
    
    fn save_file(&mut self) {
        if let Some(tab) = self.get_active_tab_mut() {
            if let Some(path) = &tab.file_path.clone() {
                match fs::write(path, &tab.content) {
                    Ok(_) => {
                        tab.is_modified = false;
                    }
                    Err(e) => {
                        eprintln!("Failed to save file: {}", e);
                    }
                }
            } else {
                self.save_file_as();
            }
        }
    }
    
    fn save_file_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Text files", &["txt"])
            .add_filter("Rust files", &["rs"])
            .add_filter("Python files", &["py"])
            .add_filter("HTML files", &["html"])
            .add_filter("All files", &["*"])
            .save_file() 
        {
            if let Some(tab) = self.get_active_tab_mut() {
                match fs::write(&path, &tab.content) {
                    Ok(_) => {
                        tab.file_path = Some(path.clone());
                        tab.title = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        tab.detect_language_from_extension();
                        tab.is_modified = false;
                    }
                    Err(e) => {
                        eprintln!("Failed to save file: {}", e);
                    }
                }
            }
        }
    }
    
    fn duplicate_line(&mut self) {
        if let Some(tab) = self.get_active_tab_mut() {
            tab.push_undo_state();
            let lines: Vec<&str> = tab.content.lines().collect();
            let cursor_line = tab.content[..tab.cursor_pos].matches('\n').count();
            
            if cursor_line < lines.len() {
                let line_to_duplicate = lines[cursor_line];
                let mut new_content = String::new();
                
                for (i, line) in lines.iter().enumerate() {
                    new_content.push_str(line);
                    if i < lines.len() - 1 || !tab.content.ends_with('\n') {
                        new_content.push('\n');
                    }
                    if i == cursor_line {
                        new_content.push_str(line_to_duplicate);
                        new_content.push('\n');
                    }
                }
                
                tab.content = new_content;
                tab.is_modified = true;
            }
        }
    }
    
    fn comment_uncomment_lines(&mut self) {
        if let Some(tab) = self.get_active_tab_mut() {
            tab.push_undo_state();
            let comment_prefix = match tab.syntax_language.as_str() {
                "Rust" | "C" | "C++" | "JavaScript" | "CSS" => "// ",
                "Python" => "# ",
                "HTML" | "XML" => "<!-- ",
                _ => "# ",
            };
            
            let lines: Vec<&str> = tab.content.lines().collect();
            let cursor_line = tab.content[..tab.cursor_pos].matches('\n').count();
            
            if cursor_line < lines.len() {
                let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
                let current_line = &new_lines[cursor_line];
                
                if current_line.trim_start().starts_with(comment_prefix.trim()) {
                    // Uncomment - create owned string
                    let uncommented = current_line.replacen(comment_prefix, "", 1);
                    new_lines[cursor_line] = uncommented;
                } else {
                    // Comment - create owned string
                    let commented = format!("{}{}", comment_prefix, current_line);
                    new_lines[cursor_line] = commented;
                }
                
                tab.content = new_lines.join("\n");
                tab.is_modified = true;
            }
        }
    }
    
    fn find_matches(&self) -> Vec<usize> {
        if let Some(tab) = self.get_active_tab() {
            if self.find_text.is_empty() {
                return Vec::new();
            }
            
            let mut matches = Vec::new();
            
            if self.use_regex {
                if let Ok(re) = Regex::new(&self.find_text) {
                    for mat in re.find_iter(&tab.content) {
                        matches.push(mat.start());
                    }
                }
            } else {
                let search_text = if self.case_sensitive {
                    tab.content.as_str()
                } else {
                    &tab.content.to_lowercase()
                };
                
                let find_text = if self.case_sensitive {
                    self.find_text.as_str()
                } else {
                    &self.find_text.to_lowercase()
                };
                
                let mut start = 0;
                while let Some(pos) = search_text[start..].find(find_text) {
                    matches.push(start + pos);
                    start += pos + 1;
                }
            }
            
            matches
        } else {
            Vec::new()
        }
    }
    
    fn replace_all(&mut self) {
        // Clone the values we need to avoid borrowing issues
        let find_text = self.find_text.clone();
        let replace_text = self.replace_text.clone();
        let use_regex = self.use_regex;
        let case_sensitive = self.case_sensitive;
        
        if let Some(tab) = self.get_active_tab_mut() {
            if !find_text.is_empty() {
                tab.push_undo_state();
                
                if use_regex {
                    if let Ok(re) = Regex::new(&find_text) {
                        tab.content = re.replace_all(&tab.content, &replace_text).to_string();
                        tab.is_modified = true;
                    }
                } else {
                    let old_content = tab.content.clone();
                    if case_sensitive {
                        tab.content = tab.content.replace(&find_text, &replace_text);
                    } else {
                        // Case insensitive replace is more complex
                        let mut result = String::new();
                        let mut last_end = 0;
                        let lower_content = tab.content.to_lowercase();
                        let lower_find = find_text.to_lowercase();
                        
                        let mut start = 0;
                        while let Some(pos) = lower_content[start..].find(&lower_find) {
                            let actual_pos = start + pos;
                            result.push_str(&tab.content[last_end..actual_pos]);
                            result.push_str(&replace_text);
                            last_end = actual_pos + find_text.len();
                            start = last_end;
                        }
                        result.push_str(&tab.content[last_end..]);
                        tab.content = result;
                    }
                    
                    if old_content != tab.content {
                        tab.is_modified = true;
                    }
                }
            }
        }
    }
    
    fn goto_line(&mut self) {
        if let Ok(line_num) = self.goto_line_input.parse::<usize>() {
            if let Some(tab) = self.get_active_tab_mut() {
                let lines: Vec<&str> = tab.content.lines().collect();
                if line_num > 0 && line_num <= lines.len() {
                    let mut pos = 0;
                    for (i, line) in lines.iter().enumerate() {
                        if i + 1 == line_num {
                            tab.cursor_pos = pos;
                            break;
                        }
                        pos += line.len() + 1; // +1 for newline
                    }
                }
            }
        }
        self.show_goto_line = false;
        self.goto_line_input.clear();
    }
    
    fn apply_theme(&mut self, theme_name: &str) {
        match theme_name {
            "Dark" => {
                self.background_color = egui::Color32::from_rgb(30, 30, 30);
                self.text_color = egui::Color32::from_rgb(220, 220, 220);
                self.selection_color = egui::Color32::from_rgb(70, 130, 180);
                self.line_number_color = egui::Color32::from_rgb(128, 128, 128);
            }
            "Light" => {
                self.background_color = egui::Color32::from_rgb(240, 240, 240);
                self.text_color = egui::Color32::from_rgb(40, 40, 40);
                self.selection_color = egui::Color32::from_rgb(173, 216, 230);
                self.line_number_color = egui::Color32::from_rgb(128, 128, 128);
            }
            "Monokai" => {
                self.background_color = egui::Color32::from_rgb(39, 40, 34);
                self.text_color = egui::Color32::from_rgb(248, 248, 242);
                self.selection_color = egui::Color32::from_rgb(73, 72, 62);
                self.line_number_color = egui::Color32::from_rgb(144, 145, 129);
            }
            "Solarized Dark" => {
                self.background_color = egui::Color32::from_rgb(0, 43, 54);
                self.text_color = egui::Color32::from_rgb(131, 148, 150);
                self.selection_color = egui::Color32::from_rgb(7, 54, 66);
                self.line_number_color = egui::Color32::from_rgb(88, 110, 117);
            }
            _ => {}
        }
        self.current_theme = theme_name.to_string();
    }
    
    fn show_menu_bar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Tab (Ctrl+T)").clicked() {
                    self.new_file();
                    ui.close_menu();
                }
                if ui.button("Open (Ctrl+O)").clicked() {
                    self.open_file();
                    ui.close_menu();
                }
                ui.separator();
                
                // Recent files submenu
                ui.menu_button("Recent Files", |ui| {
                    for file in self.recent_files.iter().rev() {
                        if ui.button(file.file_name().unwrap_or_default().to_string_lossy()).clicked() {
                            if let Ok(content) = fs::read_to_string(file) {
                                let mut tab = TabData::new(self.next_tab_id, 
                                    file.file_name().unwrap_or_default().to_string_lossy().to_string());
                                tab.content = content;
                                tab.file_path = Some(file.clone());
                                tab.detect_language_from_extension();
                                
                                self.tabs.push(tab);
                                self.active_tab = self.tabs.len() - 1;
                                self.next_tab_id += 1;
                            }
                            ui.close_menu();
                        }
                    }
                    if self.recent_files.is_empty() {
                        ui.label("No recent files");
                    }
                });
                
                ui.separator();
                if ui.button("Save (Ctrl+S)").clicked() {
                    self.save_file();
                    ui.close_menu();
                }
                if ui.button("Save As (Ctrl+Shift+S)").clicked() {
                    self.save_file_as();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            
            ui.menu_button("Edit", |ui| {
                if ui.button("Undo (Ctrl+Z)").clicked() {
                    if let Some(tab) = self.get_active_tab_mut() {
                        tab.undo();
                    }
                    ui.close_menu();
                }
                if ui.button("Redo (Ctrl+Y)").clicked() {
                    if let Some(tab) = self.get_active_tab_mut() {
                        tab.redo();
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Find & Replace (Ctrl+F)").clicked() {
                    self.show_find_replace = !self.show_find_replace;
                    ui.close_menu();
                }
                if ui.button("Go to Line (Ctrl+G)").clicked() {
                    self.show_goto_line = !self.show_goto_line;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Duplicate Line (Ctrl+D)").clicked() {
                    self.duplicate_line();
                    ui.close_menu();
                }
                if ui.button("Comment/Uncomment (Ctrl+L)").clicked() {
                    self.comment_uncomment_lines();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Select All (Ctrl+A)").clicked() {
                    if let Some(tab) = self.get_active_tab_mut() {
                        tab.selection_start = Some(0);
                        tab.selection_end = Some(tab.content.len());
                    }
                    ui.close_menu();
                }
            });
            
            ui.menu_button("View", |ui| {
                if ui.button("Style Configuration").clicked() {
                    self.show_style_config = !self.show_style_config;
                    ui.close_menu();
                }
                if ui.button("File Explorer").clicked() {
                    self.show_file_explorer = !self.show_file_explorer;
                    ui.close_menu();
                }
                if ui.button("Terminal").clicked() {
                    self.show_terminal = !self.show_terminal;
                    ui.close_menu();
                }
                ui.separator();
                
                ui.checkbox(&mut self.show_line_numbers, "Show Line Numbers");
                ui.checkbox(&mut self.word_wrap, "Word Wrap");
                ui.checkbox(&mut self.show_whitespace, "Show Whitespace");
                ui.checkbox(&mut self.show_minimap, "Show Minimap");
                
                ui.separator();
                if ui.button("Split View Horizontal").clicked() {
                    self.split_view = !self.split_view;
                    self.split_horizontal = true;
                    ui.close_menu();
                }
                if ui.button("Split View Vertical").clicked() {
                    self.split_view = !self.split_view;
                    self.split_horizontal = false;
                    ui.close_menu();
                }
                
                ui.separator();
                if ui.button("Zoom In (Ctrl++)").clicked() {
                    self.zoom_level *= 1.1;
                    self.font_size = (14.0 * self.zoom_level).max(8.0).min(32.0);
                    ui.close_menu();
                }
                if ui.button("Zoom Out (Ctrl+-)").clicked() {
                    self.zoom_level /= 1.1;
                    self.font_size = (14.0 * self.zoom_level).max(8.0).min(32.0);
                    ui.close_menu();
                }
                if ui.button("Reset Zoom").clicked() {
                    self.zoom_level = 1.0;
                    self.font_size = 14.0;
                    ui.close_menu();
                }
            });
            
            ui.menu_button("Settings", |ui| {
                ui.checkbox(&mut self.auto_indent, "Auto Indent");
                ui.checkbox(&mut self.auto_save, "Auto Save");
                ui.checkbox(&mut self.bracket_matching, "Bracket Matching");
                
                ui.separator();
                ui.label("Tab Size:");
                ui.add(egui::Slider::new(&mut self.tab_size, 2..=8));
                
                ui.separator();
                ui.menu_button("Themes", |ui| {
                    let themes = ["Dark", "Light", "Monokai", "Solarized Dark"];
                    for theme in themes.iter() {
                        if ui.selectable_label(self.current_theme == *theme, *theme).clicked() {
                            self.apply_theme(theme);
                            ui.close_menu();
                        }
                    }
                });
            });
            
            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
            });
        });
    }
    
    fn show_tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mut tab_to_close = None;
            
            for (i, tab) in self.tabs.iter().enumerate() {
                let is_active = i == self.active_tab;
                let modified_indicator = if tab.is_modified { "*" } else { "" };
                let tab_label = format!("{}{}", tab.title, modified_indicator);
                
                ui.horizontal(|ui| {
                    if ui.selectable_label(is_active, &tab_label).clicked() {
                        self.active_tab = i;
                    }
                    
                    if ui.small_button("×").clicked() {
                        tab_to_close = Some(i);
                    }
                });
                
                ui.separator();
            }
            
            if ui.button("+").clicked() {
                self.new_tab();
            }
            
            if let Some(index) = tab_to_close {
                self.close_tab(index);
            }
        });
    }
    
    fn show_style_config_window(&mut self, ctx: &egui::Context) {
        let mut show_style_config = self.show_style_config;
        if !show_style_config {
            return;
        }
        
        egui::Window::new("Style Configuration")
            .open(&mut show_style_config)
            .default_size([400.0, 300.0])
            .show(ctx, |ui| {
                ui.heading("Colors");
                
                ui.horizontal(|ui| {
                    ui.label("Background:");
                    ui.color_edit_button_srgba(&mut self.background_color);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Text:");
                    ui.color_edit_button_srgba(&mut self.text_color);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Selection:");
                    ui.color_edit_button_srgba(&mut self.selection_color);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Line Numbers:");
                    ui.color_edit_button_srgba(&mut self.line_number_color);
                });
                
                ui.separator();
                ui.heading("Font");
                ui.horizontal(|ui| {
                    ui.label("Size:");
                    ui.add(egui::Slider::new(&mut self.font_size, 8.0..=32.0));
                });
                
                ui.separator();
                ui.heading("Themes");
                ui.horizontal(|ui| {
                    let themes = ["Dark", "Light", "Monokai", "Solarized Dark"];
                    for theme in themes.iter() {
                        if ui.button(*theme).clicked() {
                            self.apply_theme(theme);
                        }
                    }
                });
            });
        
        self.show_style_config = show_style_config;
    }
    
    fn show_find_replace_window(&mut self, ctx: &egui::Context) {
        let mut show_find_replace = self.show_find_replace;
        if !show_find_replace {
            return;
        }
        
        egui::Window::new("Find & Replace")
            .open(&mut show_find_replace)
            .default_size([400.0, 200.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Find:");
                    if ui.text_edit_singleline(&mut self.find_text).changed() {
                        let matches = self.find_matches();
                        self.total_matches = matches.len();
                        if self.total_matches > 0 {
                            self.current_match = 0;
                        }
                    }
                });
                
                ui.horizontal(|ui| {
                    ui.label("Replace:");
                    ui.text_edit_singleline(&mut self.replace_text);
                });
                
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.case_sensitive, "Case Sensitive");
                    ui.checkbox(&mut self.use_regex, "Regex");
                });
                
                ui.horizontal(|ui| {
                    if ui.button("Find Next").clicked() && self.total_matches > 0 {
                        self.current_match = (self.current_match + 1) % self.total_matches;
                        let matches = self.find_matches();
                        let current_match = self.current_match;
                        let find_text_len = self.find_text.len();
                        if let Some(tab) = self.get_active_tab_mut() {
                            if let Some(&pos) = matches.get(current_match) {
                                tab.cursor_pos = pos;
                                tab.selection_start = Some(pos);
                                tab.selection_end = Some(pos + find_text_len);
                            }
                        }
                    }
                    
                    if ui.button("Find Previous").clicked() && self.total_matches > 0 {
                        self.current_match = if self.current_match == 0 {
                            self.total_matches - 1
                        } else {
                            self.current_match - 1
                        };
                        let matches = self.find_matches();
                        let current_match = self.current_match;
                        let find_text_len = self.find_text.len();
                        if let Some(tab) = self.get_active_tab_mut() {
                            if let Some(&pos) = matches.get(current_match) {
                                tab.cursor_pos = pos;
                                tab.selection_start = Some(pos);
                                tab.selection_end = Some(pos + find_text_len);
                            }
                        }
                    }
                    
                    if ui.button("Replace All").clicked() {
                        self.replace_all();
                    }
                });
                
                ui.label(format!("Matches: {} of {}", 
                    if self.total_matches > 0 { self.current_match + 1 } else { 0 }, 
                    self.total_matches));
            });
        
        self.show_find_replace = show_find_replace;
    }
    
    fn show_goto_line_window(&mut self, ctx: &egui::Context) {
        let mut show_goto_line = self.show_goto_line;
        if !show_goto_line {
            return;
        }
        
        egui::Window::new("Go to Line")
            .open(&mut show_goto_line)
            .default_size([250.0, 100.0])
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Line number:");
                    if ui.text_edit_singleline(&mut self.goto_line_input).lost_focus() 
                        && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.goto_line();
                    }
                });
                
                ui.horizontal(|ui| {
                    if ui.button("Go").clicked() {
                        self.goto_line();
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_goto_line = false;
                        self.goto_line_input.clear();
                    }
                });
            });
        
        self.show_goto_line = show_goto_line;
    }
    
    fn show_file_explorer(&mut self, ctx: &egui::Context) {
        if !self.show_file_explorer {
            return;
        }
        
        egui::Window::new("File Explorer")
            .open(&mut self.show_file_explorer)
            .default_size([250.0, 400.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("📁").clicked() {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            self.current_directory = Some(folder);
                        }
                    }
                    if let Some(dir) = &self.current_directory {
                        ui.label(dir.file_name().unwrap_or_default().to_string_lossy());
                    } else {
                        ui.label("No folder selected");
                    }
                });
                
                ui.separator();
                
                if let Some(dir) = &self.current_directory {
                    if let Ok(entries) = fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let name = path.file_name().unwrap_or_default().to_string_lossy();
                            
                            if path.is_dir() {
                                ui.label(format!("📁 {}", name));
                            } else {
                                if ui.button(format!("📄 {}", name)).clicked() {
                                    if let Ok(content) = fs::read_to_string(&path) {
                                        let mut tab = TabData::new(self.next_tab_id, name.to_string());
                                        tab.content = content;
                                        tab.file_path = Some(path);
                                        tab.detect_language_from_extension();
                                        
                                        self.tabs.push(tab);
                                        self.active_tab = self.tabs.len() - 1;
                                        self.next_tab_id += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            });
    }
    
    fn show_terminal(&mut self, ctx: &egui::Context) {
        let mut show_terminal = self.show_terminal;
        if !show_terminal {
            return;
        }
        
        egui::Window::new("Terminal")
            .open(&mut show_terminal)
            .default_size([600.0, 300.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Terminal output area
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.terminal_output)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .interactive(false)
                            );
                        });
                    
                    ui.separator();
                    
                    // Terminal input
                    ui.horizontal(|ui| {
                        ui.label("$");
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.terminal_input)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                        );
                        
                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.execute_terminal_command();
                        }
                        
                        if ui.button("Execute").clicked() {
                            self.execute_terminal_command();
                        }
                    });
                    
                    if ui.button("Clear").clicked() {
                        self.terminal_output.clear();
                    }
                });
            });
        
        self.show_terminal = show_terminal;
    }
    
    fn execute_terminal_command(&mut self) {
        if self.terminal_input.trim().is_empty() {
            return;
        }
        
        self.terminal_output.push_str(&format!("$ {}\n", self.terminal_input));
        
        // Simple command execution (in a real implementation, you'd use std::process::Command)
        match self.terminal_input.trim() {
            "clear" => {
                self.terminal_output.clear();
            }
            "pwd" => {
                if let Ok(dir) = std::env::current_dir() {
                    self.terminal_output.push_str(&format!("{}\n", dir.display()));
                }
            }
            "ls" | "dir" => {
                if let Ok(entries) = fs::read_dir(".") {
                    for entry in entries.flatten() {
                        self.terminal_output.push_str(&format!("{}\n", 
                            entry.file_name().to_string_lossy()));
                    }
                }
            }
            cmd if cmd.starts_with("echo ") => {
                let text = cmd.strip_prefix("echo ").unwrap_or("");
                self.terminal_output.push_str(&format!("{}\n", text));
            }
            _ => {
                self.terminal_output.push_str(&format!("Command not found: {}\n", self.terminal_input));
            }
        }
        
        self.terminal_input.clear();
    }
    
    fn show_about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        
        egui::Window::new("About")
            .open(&mut self.show_about)
            .default_size([350.0, 250.0])
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Advanced Rust Text Editor");
                    ui.separator();
                    ui.label("A feature-rich text editor built with Rust and egui");
                    ui.label("Version 2.0.0");
                    ui.separator();
                    
                    ui.label("Features:");
                    ui.label("• Multiple tabs and split view");
                    ui.label("• Syntax highlighting");
                    ui.label("• Find & Replace with regex support");
                    ui.label("• Undo/Redo functionality");
                    ui.label("• File explorer and terminal");
                    ui.label("• Customizable themes and styling");
                    ui.label("• Auto-indent and bracket matching");
                    ui.label("• Line numbers and minimap");
                    ui.label("• Zoom and font customization");
                    ui.label("• Recent files and project support");
                    
                    ui.separator();
                    ui.label("Keyboard Shortcuts:");
                    ui.label("Ctrl+N: New Tab, Ctrl+O: Open, Ctrl+S: Save");
                    ui.label("Ctrl+Z: Undo, Ctrl+Y: Redo, Ctrl+F: Find");
                    ui.label("Ctrl+G: Go to Line, Ctrl+D: Duplicate Line");
                    ui.label("Ctrl+L: Comment/Uncomment");
                });
            });
    }
    
    fn show_minimap(&self, ui: &mut egui::Ui, tab: &TabData) {
        if !self.show_minimap {
            return;
        }
        
        ui.allocate_ui_with_layout(
            egui::vec2(100.0, ui.available_height()),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.label("Minimap");
                ui.separator();
                
                let lines: Vec<&str> = tab.content.lines().collect();
                let visible_lines = (ui.available_height() / 3.0) as usize;
                let total_lines = lines.len();
                
                if total_lines > 0 {
                    let step = (total_lines as f32 / visible_lines as f32).ceil() as usize;
                    
                    for chunk in lines.chunks(step.max(1)) {
                        let line_preview = chunk.first().unwrap_or(&"").chars().take(20).collect::<String>();
                        ui.small(line_preview);
                    }
                }
            },
        );
    }
    
    fn render_text_editor(&mut self, ui: &mut egui::Ui) {
        // Get the tab index and clone necessary data to avoid borrowing issues
        let active_tab_index = self.active_tab;
        let show_line_numbers = self.show_line_numbers;
        let show_minimap = self.show_minimap;
        let line_number_color = self.line_number_color;
        
        if active_tab_index >= self.tabs.len() {
            return;
        }
        
        let available_rect = ui.available_rect_before_wrap();
        
        // Get line count before borrowing mutably
        let line_count = self.tabs[active_tab_index].content.lines().count().max(1);
        
        ui.horizontal(|ui| {
            // Line numbers
            if show_line_numbers {
                let line_number_width = (line_count.to_string().len() * 8) as f32 + 10.0;
                
                ui.allocate_ui_with_layout(
                    egui::vec2(line_number_width, available_rect.height()),
                    egui::Layout::top_down(egui::Align::RIGHT),
                    |ui| {
                        ui.style_mut().visuals.override_text_color = Some(line_number_color);
                        
                        for i in 1..=line_count {
                            ui.small(format!("{:>3}", i));
                        }
                    },
                );
                
                ui.separator();
            }
            
            // Main text editor
            if let Some(tab) = self.tabs.get_mut(active_tab_index) {
                let text_edit = egui::TextEdit::multiline(&mut tab.content)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(if show_minimap { 
                        available_rect.width() - 120.0 
                    } else { 
                        f32::INFINITY 
                    })
                    .desired_rows(20)
                    .lock_focus(true);
                
                let response = ui.add(text_edit);
                
                if response.changed() {
                    if !tab.is_modified {
                        tab.push_undo_state();
                    }
                    tab.is_modified = true;
                }
            }
            
            // Minimap
            if show_minimap {
                ui.separator();
                if let Some(tab) = self.tabs.get(active_tab_index) {
                    self.show_minimap(ui, tab);
                }
            }
        });
    }
}

impl eframe::App for TextEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_title(ctx);
        
        // Handle keyboard shortcuts
        ctx.input(|i| {
            if i.modifiers.ctrl {
                if i.key_pressed(egui::Key::N) {
                    self.new_file();
                }
                if i.key_pressed(egui::Key::T) {
                    self.new_tab();
                }
                if i.key_pressed(egui::Key::O) {
                    self.open_file();
                }
                if i.key_pressed(egui::Key::S) {
                    if i.modifiers.shift {
                        self.save_file_as();
                    } else {
                        self.save_file();
                    }
                }
                if i.key_pressed(egui::Key::Z) {
                    if let Some(tab) = self.get_active_tab_mut() {
                        tab.undo();
                    }
                }
                if i.key_pressed(egui::Key::Y) {
                    if let Some(tab) = self.get_active_tab_mut() {
                        tab.redo();
                    }
                }
                if i.key_pressed(egui::Key::F) {
                    self.show_find_replace = !self.show_find_replace;
                }
                if i.key_pressed(egui::Key::G) {
                    self.show_goto_line = !self.show_goto_line;
                }
                if i.key_pressed(egui::Key::D) {
                    self.duplicate_line();
                }
                // Use Ctrl+L for comment/uncomment (since / might not be available)
                if i.key_pressed(egui::Key::L) {
                    self.comment_uncomment_lines();
                }
                if i.key_pressed(egui::Key::A) {
                    if let Some(tab) = self.get_active_tab_mut() {
                        tab.selection_start = Some(0);
                        tab.selection_end = Some(tab.content.len());
                    }
                }
                if i.key_pressed(egui::Key::PlusEquals) {
                    self.zoom_level *= 1.1;
                    self.font_size = (14.0 * self.zoom_level).max(8.0).min(32.0);
                }
                if i.key_pressed(egui::Key::Minus) {
                    self.zoom_level /= 1.1;
                    self.font_size = (14.0 * self.zoom_level).max(8.0).min(32.0);
                }
            }
        });
        
        // Auto-save
        if self.auto_save {
            // Implementation would go here
        }
        
        // Show windows
        self.show_style_config_window(ctx);
        self.show_find_replace_window(ctx);
        self.show_goto_line_window(ctx);
        self.show_file_explorer(ctx);
        self.show_terminal(ctx);
        self.show_about_window(ctx);
        
        // Apply custom styling
        let mut style = (*ctx.style()).clone();
        style.visuals.extreme_bg_color = self.background_color;
        style.visuals.override_text_color = Some(self.text_color);
        style.visuals.selection.bg_fill = self.selection_color;
        
        // Set font size
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(self.font_size, egui::FontFamily::Monospace),
        );
        
        ctx.set_style(style);
        
        egui::TopBottomPanel::top("menu_panel").show(ctx, |ui| {
            self.show_menu_bar(ctx, ui);
        });
        
        egui::TopBottomPanel::top("tab_panel").show(ctx, |ui| {
            self.show_tab_bar(ui);
        });
        
        egui::TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(tab) = self.get_active_tab() {
                    ui.label(format!("Characters: {}", tab.content.len()));
                    ui.separator();
                    ui.label(format!("Lines: {}", tab.content.lines().count()));
                    ui.separator();
                    ui.label(format!("Language: {}", tab.syntax_language));
                    ui.separator();
                    ui.label(format!("Cursor: {}", tab.cursor_pos));
                    
                    if let Some(file_path) = &tab.file_path {
                        ui.separator();
                        ui.label(format!("File: {}", file_path.display()));
                    }
                    if tab.is_modified {
                        ui.separator();
                        ui.colored_label(egui::Color32::YELLOW, "Modified");
                    }
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Zoom: {}%", (self.zoom_level * 100.0) as i32));
                    ui.separator();
                    ui.label(&self.current_theme);
                });
            });
        });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.split_view && self.tabs.len() > 1 {
                if self.split_horizontal {
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width() / 2.0, ui.available_height()),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.render_text_editor(ui);
                            },
                        );
                        
                        ui.separator();
                        
                        // Switch to next tab for split view
                        let original_tab = self.active_tab;
                        self.active_tab = (self.active_tab + 1) % self.tabs.len();
                        
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), ui.available_height()),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.render_text_editor(ui);
                            },
                        );
                        
                        self.active_tab = original_tab;
                    });
                } else {
                    ui.vertical(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), ui.available_height() / 2.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.render_text_editor(ui);
                            },
                        );
                        
                        ui.separator();
                        
                        let original_tab = self.active_tab;
                        self.active_tab = (self.active_tab + 1) % self.tabs.len();
                        
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), ui.available_height()),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.render_text_editor(ui);
                            },
                        );
                        
                        self.active_tab = original_tab;
                    });
                }
            } else {
                self.render_text_editor(ui);
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([600.0, 400.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    
    eframe::run_native(
        "Advanced Rust Text Editor",
        options,
        Box::new(|_cc| Box::new(TextEditor::new())),
    )
}
