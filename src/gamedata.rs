use std::collections::HashMap;
use std::process::exit;
use tracing::{error, info};
use windows::core::PCSTR;
use windows::Win32::Foundation::{CloseHandle, FALSE};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};
use windows::Win32::Devices::HumanInterfaceDevice::{DIKEYBOARD_0, DIKEYBOARD_1, DIKEYBOARD_2, DIKEYBOARD_3, DIKEYBOARD_4, DIKEYBOARD_5, DIKEYBOARD_6, DIKEYBOARD_7, DIKEYBOARD_8, DIKEYBOARD_9, DIKEYBOARD_A, DIKEYBOARD_ADD, DIKEYBOARD_B, DIKEYBOARD_BACK, DIKEYBOARD_BACKSLASH, DIKEYBOARD_C, DIKEYBOARD_COLON, DIKEYBOARD_COMMA, DIKEYBOARD_D, DIKEYBOARD_DELETE, DIKEYBOARD_DOWN, DIKEYBOARD_E, DIKEYBOARD_END, DIKEYBOARD_EQUALS, DIKEYBOARD_ESCAPE, DIKEYBOARD_F, DIKEYBOARD_F1, DIKEYBOARD_F10, DIKEYBOARD_F11, DIKEYBOARD_F12, DIKEYBOARD_F13, DIKEYBOARD_F14, DIKEYBOARD_F15, DIKEYBOARD_F2, DIKEYBOARD_F3, DIKEYBOARD_F4, DIKEYBOARD_F5, DIKEYBOARD_F6, DIKEYBOARD_F7, DIKEYBOARD_F8, DIKEYBOARD_F9, DIKEYBOARD_G, DIKEYBOARD_H, DIKEYBOARD_HOME, DIKEYBOARD_I, DIKEYBOARD_INSERT, DIKEYBOARD_J, DIKEYBOARD_K,DIKEYBOARD_L, DIKEYBOARD_LBRACKET, DIKEYBOARD_LEFT, DIKEYBOARD_M, DIKEYBOARD_MINUS, DIKEYBOARD_N, DIKEYBOARD_O, DIKEYBOARD_P, DIKEYBOARD_PERIOD, DIKEYBOARD_Q, DIKEYBOARD_R, DIKEYBOARD_RBRACKET, DIKEYBOARD_RETURN, DIKEYBOARD_RIGHT, DIKEYBOARD_S, DIKEYBOARD_SEMICOLON, DIKEYBOARD_SLASH, DIKEYBOARD_SPACE, DIKEYBOARD_T, DIKEYBOARD_TAB, DIKEYBOARD_U, DIKEYBOARD_UP, DIKEYBOARD_V, DIKEYBOARD_W, DIKEYBOARD_X, DIKEYBOARD_Y, DIKEYBOARD_Z};
use lazy_static::lazy_static;
use egui::Key;
use once_cell::sync::Lazy;

pub const DINPUT_KEYS: Lazy<HashMap<Key, u32>> = Lazy::new(|| HashMap::from([
    (Key::Num0, DIKEYBOARD_0),
    (Key::Num1, DIKEYBOARD_1),
    (Key::Num2, DIKEYBOARD_2),
    (Key::Num3, DIKEYBOARD_3),
    (Key::Num4, DIKEYBOARD_4),
    (Key::Num5, DIKEYBOARD_5),
    (Key::Num6, DIKEYBOARD_6),
    (Key::Num7, DIKEYBOARD_7),
    (Key::Num8, DIKEYBOARD_8),
    (Key::Num9, DIKEYBOARD_9),
    (Key::A, DIKEYBOARD_A),
    (Key::Plus, DIKEYBOARD_ADD),
    (Key::B, DIKEYBOARD_B),
    (Key::Backspace, DIKEYBOARD_BACK),
    (Key::Backslash, DIKEYBOARD_BACKSLASH),
    (Key::C, DIKEYBOARD_C),
    (Key::Colon, DIKEYBOARD_COLON),
    (Key::Comma, DIKEYBOARD_COMMA),
    (Key::D, DIKEYBOARD_D),
    (Key::Delete, DIKEYBOARD_DELETE),
    (Key::ArrowDown, DIKEYBOARD_DOWN),
    (Key::E, DIKEYBOARD_E),
    (Key::End, DIKEYBOARD_END),
    (Key::Equals, DIKEYBOARD_EQUALS),
    (Key::Escape, DIKEYBOARD_ESCAPE),
    (Key::F, DIKEYBOARD_F),
    (Key::F1, DIKEYBOARD_F1),
    (Key::F10, DIKEYBOARD_F10),
    (Key::F11, DIKEYBOARD_F11),
    (Key::F12, DIKEYBOARD_F12),
    (Key::F13, DIKEYBOARD_F13),
    (Key::F14, DIKEYBOARD_F14),
    (Key::F15, DIKEYBOARD_F15),
    (Key::F2, DIKEYBOARD_F2),
    (Key::F3, DIKEYBOARD_F3),
    (Key::F4, DIKEYBOARD_F4),
    (Key::F5, DIKEYBOARD_F5),
    (Key::F6, DIKEYBOARD_F6),
    (Key::F7, DIKEYBOARD_F7),
    (Key::F8, DIKEYBOARD_F8),
    (Key::F9, DIKEYBOARD_F9),
    (Key::G, DIKEYBOARD_G),
    (Key::H, DIKEYBOARD_H),
    (Key::Home, DIKEYBOARD_HOME),
    (Key::I, DIKEYBOARD_I),
    (Key::Insert, DIKEYBOARD_INSERT),
    (Key::J, DIKEYBOARD_J),
    (Key::K, DIKEYBOARD_K),
    (Key::L, DIKEYBOARD_L),
    (Key::OpenBracket, DIKEYBOARD_LBRACKET),
    (Key::ArrowLeft, DIKEYBOARD_LEFT),
    (Key::M, DIKEYBOARD_M),
    (Key::Minus, DIKEYBOARD_MINUS),
    (Key::N, DIKEYBOARD_N),
    (Key::O, DIKEYBOARD_O),
    (Key::P, DIKEYBOARD_P),
    (Key::Period, DIKEYBOARD_PERIOD),
    (Key::Q, DIKEYBOARD_Q),
    (Key::R, DIKEYBOARD_R),
    (Key::CloseBracket, DIKEYBOARD_RBRACKET),
    (Key::Enter, DIKEYBOARD_RETURN),
    (Key::ArrowRight, DIKEYBOARD_RIGHT),
    (Key::S, DIKEYBOARD_S),
    (Key::Semicolon, DIKEYBOARD_SEMICOLON),
    (Key::Slash, DIKEYBOARD_SLASH),
    (Key::Space, DIKEYBOARD_SPACE),
    (Key::T, DIKEYBOARD_T),
    (Key::Tab, DIKEYBOARD_TAB),
    (Key::U, DIKEYBOARD_U),
    (Key::ArrowUp, DIKEYBOARD_UP),
    (Key::V, DIKEYBOARD_V),
    (Key::W, DIKEYBOARD_W),
    (Key::X, DIKEYBOARD_X),
    (Key::Y, DIKEYBOARD_Y),
    (Key::Z, DIKEYBOARD_Z),
]));

#[derive(Debug, Clone, Copy)]
pub enum GameVersion {
    V101,
    V102,
    BAG,
    Debug,
}

struct Offsets {
    process_start : isize,
    player_actor : isize,
    player_position : usize,
    player_rotation : usize,
    view_matrix : isize,
    camera_rotation : isize,
    teleport_function : isize,
    // teleport_wrapper : isize,
    loading_flag : isize,
    cutscene_flag : isize,
}

lazy_static! {
    static ref VERSIONS : HashMap<u32, GameVersion> = {
        let mut versions = HashMap::new();
        versions.insert(106266624, GameVersion::V101);
        versions.insert(26177536, GameVersion::V102);
        versions.insert(26476544, GameVersion::BAG);
        // versions.insert(0, GameVersion::Debug);
        versions
    };

    static ref OFFSETS : Offsets = unsafe {
        match get_game_version() {
            GameVersion::V101 => Offsets {
                process_start: GetModuleHandleA(PCSTR::null()).unwrap().0,
                player_actor: 0x16053E8,
                player_position: 0x50,
                player_rotation: 0x90,
                view_matrix: 0x19C73C0,
                camera_rotation: 0x1605700,
                teleport_function: 0x1AC2C0, // crashes
                // teleport_wrapper: 0x81c0,
                loading_flag: 0x11435c0,
                cutscene_flag: 0xfa54e8,
            },
            GameVersion::V102 => Offsets {
                process_start: GetModuleHandleA(PCSTR::null()).unwrap().0,
                player_actor: 0x1020948,
                player_position: 0x50,
                player_rotation: 0x90,
                view_matrix: 0x11553B0,
                camera_rotation: 0x1020C60,
                teleport_function: 0x4f0EA0,
                // teleport_wrapper: 0,
                loading_flag: 0x14005F4,
                cutscene_flag: 0x102A244,
            },
            _ => {
                panic!("Game version not yet supported!");
            }
            // GameVersion::BAG => {},
            // GameVersion::Debug => {},
        }
    };
}

unsafe fn get_game_version() -> GameVersion {
    let hmodule = GetModuleHandleA(PCSTR::null()).unwrap();

    let mut lpmodinfo: MODULEINFO = std::mem::zeroed();

    let process_id = std::process::id();
    let hprocess = OpenProcess(PROCESS_ALL_ACCESS, FALSE, process_id).unwrap();

    let _ = GetModuleInformation(hprocess, hmodule, &mut lpmodinfo, size_of::<MODULEINFO>() as u32);
    let _ = CloseHandle(hprocess);

    let file_size = lpmodinfo.SizeOfImage;

    info!("Module memory size: {file_size}");

    let version = match VERSIONS.get(&file_size) {
        Some(&v) => v,
        None => {
            error!("Unknown game version!");
            exit(1);
        }
    };

    info!("Game version: {:?}", version);

    version
}

pub fn get_player_position() -> [f32; 3] {
    let player_addr: usize = unsafe { std::ptr::read((OFFSETS.process_start + OFFSETS.player_actor) as *const _) };
    if player_addr == 0 { return [0.0, 0.0, 0.0]; }
    let player_pos: [f32; 3] = unsafe { std::ptr::read((player_addr + OFFSETS.player_position) as *const _) };
    return player_pos;
}

// pub fn set_player_position(new_pos: [f32; 3]) {
//     let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };
//     let player_addr: usize = unsafe { std::ptr::read((process_start + OFFSETS.player_actor) as *const _) };
//     if player_addr == 0 { return }
//     unsafe { std::ptr::write((player_addr + 0x50) as *mut _, new_pos); };
// }

pub fn get_player_rotation() -> [f32; 3] {
    let player_addr: usize = unsafe { std::ptr::read((OFFSETS.process_start + OFFSETS.player_actor) as *const _) };
    if player_addr == 0 { return [0.0, 0.0, 0.0]; }
    let player_rot: [f32; 3] = unsafe { std::ptr::read((player_addr + OFFSETS.player_rotation) as *const _) };
    return player_rot;
}

// pub fn set_player_rotation(new_rot: [f32; 3]) {
//     let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };
//     let player_addr: usize = unsafe { std::ptr::read((process_start + OFFSETS.player_actor) as *const _) };
//     if player_addr == 0 { return }
//     unsafe { std::ptr::write((player_addr + 0x90) as *mut _, new_rot); };
// }

pub fn get_view_matrix() -> [[f32; 4]; 4] {
    let view_proj: [[f32; 4]; 4] = unsafe { std::ptr::read((OFFSETS.process_start + OFFSETS.view_matrix) as *const _) };
    return view_proj;
}

pub fn get_camera_rotation() -> [f32; 2] {
    let cam_rot: [f32; 2] = unsafe { std::ptr::read((OFFSETS.process_start + OFFSETS.camera_rotation) as *const _) };
    return cam_rot;
}

pub fn set_camera_rotation(new_rot: [f32; 2]) {
    unsafe { std::ptr::write((OFFSETS.process_start + OFFSETS.camera_rotation) as *mut _, new_rot) };
}

pub fn teleport_player(location: [f32; 3], rotation: [f32; 3]) {
    let player_addr: usize = unsafe { std::ptr::read((OFFSETS.process_start + OFFSETS.player_actor) as *const _) };
    if player_addr == 0 { return; }
    let teleport_function = unsafe {std::mem::transmute::<_, extern "C" fn(usize, [f32; 3], [f32; 3], i32, f32)>(OFFSETS.process_start + OFFSETS.teleport_function) };
    teleport_function(player_addr, location, rotation, 1, 0.);
}

// pub fn teleport_player_wrapper(location: [f32; 3], rotation: [f32; 3]) {
//     let teleport_function = unsafe {std::mem::transmute::<_, extern "C" fn([f32; 3], [f32; 3], i32)>(OFFSETS.process_start + OFFSETS.teleport_wrapper) };
//     teleport_function(location, rotation, 1);
// }

pub fn get_is_loading() -> bool {
    let loading: bool = unsafe { std::ptr::read((OFFSETS.process_start + OFFSETS.loading_flag) as *const _) };
    return loading;
}

pub fn get_is_cutscene_playing() -> bool {
    let cutscene_playing: bool = unsafe { std::ptr::read((OFFSETS.process_start + OFFSETS.cutscene_flag) as *const _) };
    return cutscene_playing;
}

// pub fn get_player_state() -> [u32; 2] {
//     let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

//     let player_addr: usize = unsafe { std::ptr::read((process_start + OFFSETS.player_actor) as *const _) };
//     if player_addr == 0 {
//         return [0, 0];
//     }

//     let player_state: [u32; 2] = unsafe { std::ptr::read((player_addr + 0x670) as *const _) };

//     return player_state;
// }

// pub fn set_player_state(new_state: [u32; 2]) {
//     let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

//     let player_addr: usize = unsafe { std::ptr::read((process_start + OFFSETS.player_actor) as *const _) };
//     if player_addr == 0 { return }

//     unsafe {
//         std::ptr::write((player_addr + 0x670) as *mut _, new_state);
//     };
// }

// pub fn get_player_actor() -> usize {
//     let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

//     let player_addr: usize = unsafe { std::ptr::read((process_start + OFFSETS.player_actor) as *const _) };

//     return player_addr;
// }

// pub fn get_player_handle() -> u32 {
//     let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

//     let player_addr: u32 = unsafe { std::ptr::read((process_start + 0x125025C) as *const _) };

//     return player_addr;
// }

// pub fn get_noclip_state() -> u32 {
//     let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

//     let player_addr: usize = unsafe { std::ptr::read((process_start + OFFSETS.player_actor) as *const _) };
//     if player_addr == 0 {
//         return 0;
//     }

//     let noclip_state: u32 = unsafe { std::ptr::read((player_addr + 0x14E0) as *const _) };

//     return noclip_state;
// }