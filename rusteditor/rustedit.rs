[dependencies]
eframe = "0.24"
egui = "0.24"
rfd = "0.12"
serde = { version = "1.0", features = ["derive"] }

[package]
name = "text_editor"
version = "0.1.0"
edition = "2021"

# src/main.rs
use eframe::egui;
use egui::{Color32, FontId, RichText, Visuals};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
struct EditorTheme {
    background_color: [u8; 3],
    text_color: [u8; 3],
    selection_color: [u8; 3],
    cursor_color: [u8; 3],
}

impl Default for EditorTheme {
    fn default() -> Self {
        Self {
            background_color: [40, 40, 40],
            text_color: [255, 255, 255],
            selection_color: [100, 150, 200],
            cursor_color: [255, 255, 255],
        }
    }
}

struct TextEditor {
    content: String,
    file_path: Option<PathBuf>,
    is_modified: bool,
    show_style_config: bool,
    theme: EditorTheme,
    font_size: f32,
    word_wrap: bool,
    show_line_numbers: bool,
    find_text: String,
    replace_text: String,
    show_find_replace: bool,
    status_message: String,
    cursor_pos: Option<usize>,
}

impl Default for TextEditor {
    fn default() -> Self {
        Self {
            content: String::new(),
            file_path: None,
            is_modified: false,
            show_style_config: false,
            theme: EditorTheme::default(),
            font_size: 14.0,
            word_wrap: true,
            show_line_numbers: true,
            find_text: String::new(),
            replace_text: String::new(),
            show_find_replace: false,
            status_message: "Ready".to_string(),
            cursor_pos: None,
        }
    }
}

impl TextEditor {
    fn new_file(&mut self) {
        if self.is_modified {
            // In a real app, you'd show a save dialog here
            self.status_message = "Warning: Unsaved changes will be lost".to_string();
        }
        self.content.clear();
        self.file_path = None;
        self.is_modified = false;
        self.status_message = "New file created".to_string();
    }

    fn open_file(&mut self) {
        if let Some(path) = FileDialog::new()
            .add_filter("Text files", &["txt", "rs", "py", "html", "css", "js"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    self.content = content;
                    self.file_path = Some(path.clone());
                    self.is_modified = false;
                    self.status_message = format!("Opened: {}", path.display());
                }
                Err(e) => {
                    self.status_message = format!("Error opening file: {}", e);
                }
            }
        }
    }

    fn save_file(&mut self) {
        if let Some(path) = &self.file_path {
            match fs::write(path, &self.content) {
                Ok(_) => {
                    self.is_modified = false;
                    self.status_message = format!("Saved: {}", path.display());
                }
                Err(e) => {
                    self.status_message = format!("Error saving file: {}", e);
                }
            }
        } else {
            self.save_file_as();
        }
    }

    fn save_file_as(&mut self) {
        if let Some(path) = FileDialog::new()
            .add_filter("Text files", &["txt"])
            .add_filter("Rust files", &["rs"])
            .add_filter("Python files", &["py"])
            .add_filter("HTML files", &["html"])
            .save_file()
        {
            match fs::write(&path, &self.content) {
                Ok(_) => {
                    self.file_path = Some(path.clone());
                    self.is_modified = false;
                    self.status_message = format!("Saved as: {}", path.display());
                }
                Err(e) => {
                    self.status_message = format!("Error saving file: {}", e);
                }
            }
        }
    }

    fn find_and_replace(&mut self) {
        if !self.find_text.is_empty() && !self.replace_text.is_empty() {
            let old_content = self.content.clone();
            self.content = self.content.replace(&self.find_text, &self.replace_text);
            if old_content != self.content {
                self.is_modified = true;
                self.status_message = "Text replaced".to_string();
            } else {
                self.status_message = "No matches found".to_string();
            }
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let mut visuals = Visuals::dark();
        visuals.override_text_color = Some(Color32::from_rgb(
            self.theme.text_color[0],
            self.theme.text_color[1],
            self.theme.text_color[2],
        ));
        visuals.panel_fill = Color32::from_rgb(
            self.theme.background_color[0],
            self.theme.background_color[1],
            self.theme.background_color[2],
        );
        visuals.selection.bg_fill = Color32::from_rgb(
            self.theme.selection_color[0],
            self.theme.selection_color[1],
            self.theme.selection_color[2],
        );
        ctx.set_visuals(visuals);
    }

    fn get_line_count(&self) -> usize {
        self.content.lines().count().max(1)
    }

    fn show_style_configurator(&mut self, ctx: &egui::Context) {
        egui::Window::new("Style Configurator")
            .open(&mut self.show_style_config)
            .show(ctx, |ui| {
                ui.heading("Theme Colors");
                
                ui.horizontal(|ui| {
                    ui.label("Background:");
                    ui.color_edit_button_rgb(&mut self.theme.background_color);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Text:");
                    ui.color_edit_button_rgb(&mut self.theme.text_color);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Selection:");
                    ui.color_edit_button_rgb(&mut self.theme.selection_color);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Cursor:");
                    ui.color_edit_button_rgb(&mut self.theme.cursor_color);
                });

                ui.separator();
                
                ui.horizontal(|ui| {
                    ui.label("Font Size:");
                    ui.add(egui::Slider::new(&mut self.font_size, 8.0..=32.0));
                });

                ui.separator();
                
                ui.checkbox(&mut self.word_wrap, "Word Wrap");
                ui.checkbox(&mut self.show_line_numbers, "Show Line Numbers");

                ui.separator();

                if ui.button("Reset to Default").clicked() {
                    self.theme = EditorTheme::default();
                    self.font_size = 14.0;
                    self.word_wrap = true;
                    self.show_line_numbers = true;
                }
            });
    }

    fn show_find_replace(&mut self, ctx: &egui::Context) {
        egui::Window::new("Find & Replace")
            .open(&mut self.show_find_replace)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Find:");
                    ui.text_edit_singleline(&mut self.find_text);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Replace:");
                    ui.text_edit_singleline(&mut self.replace_text);
                });
                
                ui.horizontal(|ui| {
                    if ui.button("Replace All").clicked() {
                        self.find_and_replace();
                    }
                    if ui.button("Close").clicked() {
                        self.show_find_replace = false;
                    }
                });
            });
    }
}

impl eframe::App for TextEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);

        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.new_file();
                        ui.close_menu();
                    }
                    if ui.button("Open").clicked() {
                        self.open_file();
                        ui.close_menu();
                    }
                    if ui.button("Save").clicked() {
                        self.save_file();
                        ui.close_menu();
                    }
                    if ui.button("Save As").clicked() {
                        self.save_file_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Find & Replace").clicked() {
                        self.show_find_replace = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Select All").clicked() {
                        // This would require more complex text selection handling
                        self.status_message = "Select All clicked".to_string();
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Style Configurator").clicked() {
                        self.show_style_config = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.word_wrap, "Word Wrap");
                    ui.checkbox(&mut self.show_line_numbers, "Line Numbers");
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.is_modified {
                        ui.label("Modified");
                    }
                    if let Some(path) = &self.file_path {
                        ui.label(format!("File: {}", path.display()));
                    } else {
                        ui.label("Untitled");
                    }
                    ui.label(format!("Lines: {}", self.get_line_count()));
                });
            });
        });

        // Main editor area
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.show_line_numbers {
                        ui.horizontal_top(|ui| {
                            // Line numbers
                            let line_count = self.get_line_count();
                            let line_height = self.font_size + 4.0;
                            
                            egui::ScrollArea::vertical()
                                .id_source("line_numbers")
                                .show(ui, |ui| {
                                    ui.allocate_ui_with_layout(
                                        egui::Vec2::new(40.0, line_height * line_count as f32),
                                        egui::Layout::top_down(egui::Align::RIGHT),
                                        |ui| {
                                            for i in 1..=line_count {
                                                ui.label(
                                                    RichText::new(format!("{:>3}", i))
                                                        .font(FontId::monospace(self.font_size))
                                                        .color(Color32::GRAY)
                                                );
                                            }
                                        },
                                    );
                                });

                            ui.separator();

                            // Text editor
                            let text_edit = egui::TextEdit::multiline(&mut self.content)
                                .font(FontId::monospace(self.font_size))
                                .desired_width(f32::INFINITY)
                                .desired_rows(20);

                            let response = if self.word_wrap {
                                ui.add(text_edit)
                            } else {
                                ui.add(text_edit.wrap(false))
                            };

                            if response.changed() {
                                self.is_modified = true;
                            }
                        });
                    } else {
                        // Text editor without line numbers
                        let text_edit = egui::TextEdit::multiline(&mut self.content)
                            .font(FontId::monospace(self.font_size))
                            .desired_width(f32::INFINITY)
                            .desired_rows(20);

                        let response = if self.word_wrap {
                            ui.add(text_edit)
                        } else {
                            ui.add(text_edit.wrap(false))
                        };

                        if response.changed() {
                            self.is_modified = true;
                        }
                    }
                });
        });

        // Show dialogs
        if self.show_style_config {
            self.show_style_configurator(ctx);
        }

        if self.show_find_replace {
            self.show_find_replace(ctx);
        }

        // Keyboard shortcuts
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::N) {
                self.new_file();
            }
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::O) {
                self.open_file();
            }
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::S) {
                self.save_file();
            }
            if i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::S) {
                self.save_file_as();
            }
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::F) {
                self.show_find_replace = true;
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Rust Text Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "Rust Text Editor", 
        options, 
        Box::new(|_cc| Box::new(TextEditor::default()))
    )
}
