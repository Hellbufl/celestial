use ini::Ini;
use egui::{Key, KeyboardShortcut, Modifiers, ecolor};
use egui_keybind::Shortcut;
use serde_json;
use tracing::{info, error};

const CONFIG_FILE_NAME : &str = "celestial.ini";

pub struct ConfigState {
	// pub show_ui: bool,
	pub direct_mode: bool,
    pub autosave: bool,
	// pub toggle_window_keybind: Shortcut,
	pub start_keybind: Shortcut,
	pub stop_keybind: Shortcut,
	pub reset_keybind: Shortcut,
	pub clear_keybind: Shortcut,

	pub trigger_size: [[f32; 3]; 2],
    pub timer_size: f32,
    pub timer_position: [f32; 2],

	pub trigger_color: [[f32; 4]; 2],
    pub fast_color: [f32; 4],
    pub slow_color: [f32; 4],
    pub gold_color: [f32; 4],
    pub select_color: [f32; 4],

    pub accent_colors: [egui::Color32; 2],
}

impl ConfigState {
    pub fn new() -> ConfigState {
        let mut state = ConfigState {
            // show_ui: true,
            direct_mode: false,
            autosave: false,
            // toggle_window_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Home}), None),
            start_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Comma}), None),
            stop_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Period}), None),
            reset_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Minus}), None),
            clear_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Delete}), None),

            trigger_size: [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0]],
            timer_size: 24.,
            timer_position: [400f32, 50f32],

            trigger_color: [[0.0, 0.0, 0.0, 0.8], [1.0, 1.0, 1.0, 0.8]],
            fast_color: [0.4, 1.0, 0.2, 0.8],
            slow_color: [1.0, 0.1, 0.2, 0.8],
            gold_color: [1.0, 0.9, 0.5, 0.9],
            select_color: [0.7, 0.8, 1.0, 0.8],

            accent_colors: [egui::Color32::from_rgb(85, 149, 255), egui::Color32::from_rgb(156, 85, 255)],
        };

        if let Err(_) = state.read("data/".to_string() + CONFIG_FILE_NAME) {
            info!("No config file");
        }

        state
    }

    pub fn read(&mut self, file_path: String) -> Result<i32, ini::Error> {
        let conf = Ini::load_from_file(file_path.clone())?;
        let section = conf.section(Some("Celestial")).unwrap();

        // self.show_ui = section.get("show_ui").unwrap().parse::<bool>().unwrap();
        // self.toggle_window_keybind = Shortcut::from_string(section.get("toggle_window_keybind").unwrap_or(self.toggle_window_keybind.to_string().as_str()));
        self.start_keybind = Shortcut::from_string(section.get("start_keybind").unwrap_or(self.start_keybind.to_string().as_str()));
        self.stop_keybind = Shortcut::from_string(section.get("stop_keybind").unwrap_or(self.stop_keybind.to_string().as_str()));
        self.reset_keybind = Shortcut::from_string(section.get("reset_keybind").unwrap_or(self.reset_keybind.to_string().as_str()));
        self.clear_keybind = Shortcut::from_string(section.get("clear_keybind").unwrap_or(self.clear_keybind.to_string().as_str()));

        self.trigger_size[0] = serde_json::from_str(section.get("start_trigger_size").unwrap()).unwrap();
        self.trigger_size[1] = serde_json::from_str(section.get("end_trigger_size").unwrap()).unwrap();
        self.timer_size = serde_json::from_str(section.get("timer_size").unwrap()).unwrap();
        self.timer_position = serde_json::from_str(section.get("timer_position").unwrap()).unwrap();

        self.trigger_color[0] = serde_json::from_str(section.get("start_trigger_color").unwrap()).unwrap();
        self.trigger_color[1] = serde_json::from_str(section.get("end_trigger_color").unwrap()).unwrap();
        self.fast_color = serde_json::from_str(section.get("fast_color").unwrap()).unwrap();
        self.slow_color = serde_json::from_str(section.get("slow_color").unwrap()).unwrap();
        self.gold_color = serde_json::from_str(section.get("gold_color").unwrap()).unwrap();
        self.select_color = serde_json::from_str(section.get("select_color").unwrap()).unwrap();
        self.accent_colors[0] = serde_json::from_str(section.get("accent_color_0").unwrap()).unwrap();
        self.accent_colors[1] = serde_json::from_str(section.get("accent_color_1").unwrap()).unwrap();

        info!("Config loaded");
        Ok(0)
    }

    pub fn write(&mut self, file_path: String) {
        let mut conf = Ini::new();

        conf.with_section(Some("Celestial"))
            // .set("show_ui", self.show_ui.to_string())
            // .set("toggle_window_keybind", shortcut_to_string(self.toggle_window_keybind))
            .set("start_keybind", self.start_keybind.to_string())
            .set("stop_keybind", self.stop_keybind.to_string())
            .set("reset_keybind", self.reset_keybind.to_string())
            .set("clear_keybind", self.clear_keybind.to_string())

            .set("start_trigger_size", format!("{:?}", self.trigger_size[0]))
            .set("end_trigger_size", format!("{:?}", self.trigger_size[1]))
            .set("timer_size", format!("{:?}", self.timer_size))
            .set("timer_position", format!("{:?}", self.timer_position))

            .set("start_trigger_color", format!("{:?}", self.trigger_color[0]))
            .set("end_trigger_color", format!("{:?}", self.trigger_color[1]))
            .set("fast_color", format!("{:?}", self.fast_color))
            .set("slow_color", format!("{:?}", self.slow_color))
            .set("gold_color", format!("{:?}", self.gold_color))
            .set("select_color", format!("{:?}", self.select_color))
            .set("accent_color_0", format!("{:?}", self.accent_colors[0].to_array()))
            .set("accent_color_1", format!("{:?}", self.accent_colors[1].to_array()));

        conf.write_to_file(file_path).unwrap();

        info!("Config saved");
    }
}

trait ShortcutString {
    fn to_string(&self) -> String;
    fn from_string(stringcut: &str) -> Shortcut;
}

impl ShortcutString for Shortcut {
    fn to_string(&self) -> String {
        let keyboard = self.keyboard();

        if keyboard.is_none() {
            return "".to_string();
        }

        let mut stringcut = "".to_string();

        let mods = keyboard.unwrap().modifiers;
        if mods.alt { stringcut += "alt+"};
        if mods.ctrl { stringcut += "ctrl+"};
        if mods.shift { stringcut += "shift+"};
        if mods.mac_cmd { stringcut += "mac_cmd+"};
        if mods.command { stringcut += "command+"};

        // unsafe { stringcut + std::mem::transmute::<_, u8>(keyboard.logical_key).to_string() }
        stringcut + &(keyboard.unwrap().logical_key as u8).to_string()
    }

    fn from_string(stringcut: &str) -> Shortcut {
        let mut keys: Vec<&str> = stringcut.split("+").collect();
        let key = keys.pop();
    
        if key.is_none() {
            return Shortcut::NONE;
        }
    
        let mut keyboard : KeyboardShortcut;
        unsafe { keyboard = KeyboardShortcut::new(Modifiers::NONE, std::mem::transmute::<_, Key>(key.unwrap().parse::<u8>().unwrap())); }
    
        for m in keys {
            match m {
                "alt" => keyboard.modifiers.alt = true,
                "ctrl" => keyboard.modifiers.ctrl = true,
                "shift" => keyboard.modifiers.shift = true,
                "mac_cmd" => keyboard.modifiers.mac_cmd = true,
                "command" => keyboard.modifiers.command = true,
                _ => error!("Invalid modifier key: {m}"),
            }
        }
    
        Shortcut::new(Some(keyboard), None)
    }
}

pub trait CompareKeybindToEvent {
    fn compare_to_event(&self, event: &egui::Event) -> bool;
}

impl CompareKeybindToEvent for Shortcut {
    fn compare_to_event(&self, event: &egui::Event) -> bool {
        if let (egui::Event::Key{key, physical_key, pressed, repeat, modifiers}, Some(keyboard_short)) = (event, self.keyboard())  {
            *modifiers == keyboard_short.modifiers && *key == keyboard_short.logical_key && *pressed
        }
        else { false }
    }
}

pub trait AsHsva {
    fn as_hsva(&self) -> ecolor::Hsva;
}

impl AsHsva for [f32;4] {
    fn as_hsva(&self) -> ecolor::Hsva {
        ecolor::Hsva::from_rgba_premultiplied(self[0], self[1], self[2], self[3])
    }
}

impl AsHsva for egui::Color32 {
    fn as_hsva(&self) -> ecolor::Hsva {
        let [r, g, b, a] = self.to_array();
        ecolor::Hsva::from_rgba_premultiplied(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
    }
}

pub trait AsColor32 {
    fn as_color32(&self) -> egui::Color32;
}

impl AsColor32 for ecolor::Hsva {
    fn as_color32(&self) -> egui::Color32 {
        let [r, g, b, a] = self.to_rgba_premultiplied();
        egui::Color32::from_rgba_premultiplied((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, (a * 255.0) as u8)
    }
}

impl AsColor32 for [f32; 4] {
    fn as_color32(&self) -> egui::Color32 {
        egui::Color32::from_rgba_premultiplied((self[0] * 255.0) as u8, (self[1] * 255.0) as u8, (self[2] * 255.0) as u8, (self[3] * 255.0) as u8)
    }
}