use ini::Ini;
use egui::{Key, KeyboardShortcut, Modifiers, ecolor};
use egui_keybind::Shortcut;
use serde_json;
use tracing::{info, error};

use crate::error::Error;

pub const CONFIG_FILE_NAME : &str = "celestial.ini";

macro_rules! set_if_ok {
    ($dest:expr, $source:expr) => {
        if let Ok(value) = $source {
            $dest = value;
        }
        else { error!("Value not found. Default: {:?}", $dest); }
    };
}

pub struct ConfigState {
	// pub show_ui: bool,
	pub direct_mode: bool,
    pub autosave: bool,
    pub autoreset: bool,

    pub zoom: f32,

	// pub toggle_window_keybind: Shortcut,
	pub start_keybind: Shortcut,
	pub stop_keybind: Shortcut,
	pub reset_keybind: Shortcut,
	pub clear_keybind: Shortcut,
	pub teleport_keybinds: [Shortcut; 2],
	// pub spawn_teleport_keybinds: [Shortcut; 2],
	pub spawn_checkpoint_keybind: Shortcut,

    pub extra_teleport_modifiers: Modifiers,
    pub spawn_teleport_modifiers: Modifiers,

	pub extra_teleport_keybinds: [Shortcut; 10],
	pub spawn_teleport_keybinds: [Shortcut; 10],

	pub trigger_sizes: [[f32; 3]; 2],
    pub timer_size: f32,
    pub timer_position: [f32; 2],

	pub trigger_colors: [[f32; 4]; 2],
	pub checkpoint_color: [f32; 4],
    pub fast_color: [f32; 4],
    pub slow_color: [f32; 4],
    pub gold_color: [f32; 4],
    pub select_color: [f32; 4],
    pub accent_colors: [egui::Color32; 2],

    pub shapes_enabled: bool,
}

impl ConfigState {
    pub fn new() -> ConfigState {
        let mut config = ConfigState {
            // show_ui: true,
            direct_mode: false,
            autosave: false,
            autoreset: true,
            zoom: 1.0,
            // toggle_window_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Home}), None),
            start_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Comma}), None),
            stop_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Period}), None),
            reset_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Minus}), None),
            clear_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::Delete}), None),
            teleport_keybinds: [
                Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::K}), None),
                Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::L}), None),
            ],
            // spawn_teleport_keybinds: [
            //     Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::SHIFT, logical_key: Key::Comma}), None),
            //     Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::SHIFT, logical_key: Key::Period}), None),
            // ],
            spawn_checkpoint_keybind: Shortcut::new(Some(KeyboardShortcut{modifiers: Modifiers::NONE, logical_key: Key::C}), None),

            extra_teleport_modifiers: Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false },
            spawn_teleport_modifiers: Modifiers { alt: true, ctrl: false, shift: true, mac_cmd: false, command: false },

            extra_teleport_keybinds: [Default::default(); 10],
            spawn_teleport_keybinds: [Default::default(); 10],

            trigger_sizes: [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0]],
            timer_size: 24.,
            timer_position: [400f32, 50f32],

            trigger_colors: [[0.0, 0.0, 0.0, 0.8], [1.0, 1.0, 1.0, 0.8]],
            checkpoint_color: [0.5, 0.5, 0.5, 0.5],
            fast_color: [0.4, 1.0, 0.2, 0.8],
            slow_color: [1.0, 0.1, 0.2, 0.8],
            gold_color: [1.0, 0.9, 0.5, 0.9],
            select_color: [0.7, 0.8, 1.0, 0.8],
            accent_colors: [egui::Color32::from_rgb(85, 149, 255), egui::Color32::from_rgb(156, 85, 255)],

            shapes_enabled: false,
        };

        config.generate_extra_teleport_keybinds();
        config.generate_spawn_teleport_keybinds();

        config
    }

    pub fn init() -> ConfigState {
        let mut state = Self::new();

        if let Err(_) = state.read(CONFIG_FILE_NAME.to_string()) {
            if let Err(e) = state.read("data/".to_string() + CONFIG_FILE_NAME) {
                error!("{e}");
            }
        }

        state
    }

    pub fn read(&mut self, file_path: String) -> Result<(), Error> {
        let conf = Ini::load_from_file(file_path.clone())?;

        let mut general_section = conf.section(Some("General"));
        if general_section.is_none() { general_section = conf.section(Some("Celestial")) } // backwards compatibility

        if let Some(section) = general_section {
            set_if_ok!(self.autoreset, section.get("autoreset").unwrap_or("true").parse::<bool>());
            set_if_ok!(self.zoom, section.get("zoom").unwrap_or("1.0").parse::<f32>());

            set_if_ok!(self.start_keybind, Shortcut::from_string(section.get("start_keybind").unwrap_or("")));
            set_if_ok!(self.stop_keybind, Shortcut::from_string(section.get("stop_keybind").unwrap_or("")));
            set_if_ok!(self.reset_keybind, Shortcut::from_string(section.get("reset_keybind").unwrap_or("")));
            set_if_ok!(self.clear_keybind, Shortcut::from_string(section.get("clear_keybind").unwrap_or("")));
            set_if_ok!(self.teleport_keybinds[0], Shortcut::from_string(section.get("teleport_1_keybind").unwrap_or("")));
            set_if_ok!(self.teleport_keybinds[1], Shortcut::from_string(section.get("teleport_2_keybind").unwrap_or("")));
            // set_if_ok!(self.spawn_teleport_keybinds[0], Shortcut::from_string(section.get("spawn_teleport_1_keybind").unwrap_or("")));
            // set_if_ok!(self.spawn_teleport_keybinds[1], Shortcut::from_string(section.get("spawn_teleport_2_keybind").unwrap_or("")));
            set_if_ok!(self.spawn_checkpoint_keybind, Shortcut::from_string(section.get("spawn_checkpoint_keybind").unwrap_or("")));

            set_if_ok!(self.trigger_sizes[0], serde_json::from_str(section.get("start_trigger_size").unwrap_or("")));
            set_if_ok!(self.trigger_sizes[1], serde_json::from_str(section.get("end_trigger_size").unwrap_or("")));
            set_if_ok!(self.timer_size, serde_json::from_str(section.get("timer_size").unwrap_or("")));
            set_if_ok!(self.timer_position, serde_json::from_str(section.get("timer_position").unwrap_or("")));

            set_if_ok!(self.trigger_colors[0], serde_json::from_str(section.get("start_trigger_color").unwrap_or("")));
            set_if_ok!(self.trigger_colors[1], serde_json::from_str(section.get("end_trigger_color").unwrap_or("")));
            set_if_ok!(self.checkpoint_color, serde_json::from_str(section.get("checkpoint_color").unwrap_or("")));
            set_if_ok!(self.fast_color, serde_json::from_str(section.get("fast_color").unwrap_or("")));
            set_if_ok!(self.slow_color, serde_json::from_str(section.get("slow_color").unwrap_or("")));
            set_if_ok!(self.gold_color, serde_json::from_str(section.get("gold_color").unwrap_or("")));
            set_if_ok!(self.select_color, serde_json::from_str(section.get("select_color").unwrap_or("")));
            set_if_ok!(self.accent_colors[0], serde_json::from_str(section.get("accent_color_0").unwrap_or("")));
            set_if_ok!(self.accent_colors[1], serde_json::from_str(section.get("accent_color_1").unwrap_or("")));
        }
        else { error!("'General' section not found in config file.") }

        let extra_section = conf.section(Some("Extra"));

        if let Some(section) = extra_section {
            set_if_ok!(self.shapes_enabled, section.get("custom_shapes").unwrap_or("").parse::<bool>());
        }
        else { error!("'Extra' section not found in config file.") }

        self.generate_extra_teleport_keybinds();
        self.generate_spawn_teleport_keybinds();

        info!("Config loaded");
        Ok(())
    }

    pub fn write(&mut self, file_path: String) -> Result<(), Error> {
        let mut conf = Ini::new();

        conf.with_section(Some("General"))
            // .set("show_ui", self.show_ui.to_string())
            .set("autoreset", self.autoreset.to_string())
            .set("zoom", self.zoom.to_string())
            // .set("toggle_window_keybind", shortcut_to_string(self.toggle_window_keybind))
            .set("start_keybind", self.start_keybind.to_string())
            .set("stop_keybind", self.stop_keybind.to_string())
            .set("reset_keybind", self.reset_keybind.to_string())
            .set("clear_keybind", self.clear_keybind.to_string())

            .set("teleport_1_keybind", self.teleport_keybinds[0].to_string())
            .set("teleport_2_keybind", self.teleport_keybinds[1].to_string())
            // .set("spawn_teleport_1_keybind", self.spawn_teleport_keybinds[0].to_string())
            // .set("spawn_teleport_2_keybind", self.spawn_teleport_keybinds[1].to_string())
            .set("spawn_checkpoint_keybind", self.spawn_checkpoint_keybind.to_string())

            .set("start_trigger_size", format!("{:?}", self.trigger_sizes[0]))
            .set("end_trigger_size", format!("{:?}", self.trigger_sizes[1]))
            .set("timer_size", format!("{:?}", self.timer_size))
            .set("timer_position", format!("{:?}", self.timer_position))

            .set("start_trigger_color", format!("{:?}", self.trigger_colors[0]))
            .set("end_trigger_color", format!("{:?}", self.trigger_colors[1]))
            .set("checkpoint_color", format!("{:?}", self.checkpoint_color))
            .set("fast_color", format!("{:?}", self.fast_color))
            .set("slow_color", format!("{:?}", self.slow_color))
            .set("gold_color", format!("{:?}", self.gold_color))
            .set("select_color", format!("{:?}", self.select_color))
            .set("accent_color_0", format!("{:?}", self.accent_colors[0].to_array()))
            .set("accent_color_1", format!("{:?}", self.accent_colors[1].to_array()));

        conf.with_section(Some("Extra"))
            .set("custom_shapes", self.shapes_enabled.to_string());

        conf.write_to_file(file_path)?;

        info!("Config saved");
        Ok(())
    }

    pub fn generate_extra_teleport_keybinds(&mut self) {
        let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9, Key::Num0];
        for i in 0..10 {
            self.extra_teleport_keybinds[i] = Shortcut::new(Some(KeyboardShortcut { modifiers: self.extra_teleport_modifiers, logical_key: keys[i] }), None);
        }
    }

    pub fn generate_spawn_teleport_keybinds(&mut self) {
        let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9, Key::Num0];
        for i in 0..10 {
            self.spawn_teleport_keybinds[i] = Shortcut::new(Some(KeyboardShortcut { modifiers: self.spawn_teleport_modifiers, logical_key: keys[i] }), None);
        }
    }
}

trait ShortcutString {
    fn to_string(&self) -> String;
    fn from_string(stringcut: &str) -> Result<Shortcut, std::io::Error>;
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

    fn from_string(stringcut: &str) -> Result<Shortcut, std::io::Error> {
        let mut keys: Vec<&str> = stringcut.split("+").collect();
        let key = keys.pop();

        if key.is_none() {
            return Err(std::io::Error::other("p"));
        }

        let mut keyboard : KeyboardShortcut;
        let keycode = match key.unwrap().parse::<u8>() {
            Ok(v) => { v },
            Err(_) => { return Err(std::io::Error::other("p")) }
        };
        unsafe { keyboard = KeyboardShortcut::new(Modifiers::NONE, std::mem::transmute::<_, Key>(keycode)); }

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

        Ok(Shortcut::new(Some(keyboard), None))
    }
}

pub trait CompareKeybindToEvent {
    fn compare_to_event(&self, event: &egui::Event) -> bool;
}

impl CompareKeybindToEvent for Shortcut {
    fn compare_to_event(&self, event: &egui::Event) -> bool {
        if let (egui::Event::Key{key, physical_key, pressed, repeat, modifiers}, Some(keyboard_short)) = (event, self.keyboard())  {
            let _ = (physical_key, repeat);
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