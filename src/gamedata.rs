use std::collections::HashMap;
use std::process::exit;
use tracing::{error, info};
use windows::core::PCSTR;
use windows::Win32::Foundation::{CloseHandle, FALSE};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};
use lazy_static::lazy_static;

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
                loading_flag: 0x14005F4,
                cutscene_flag: 0x102A244,
            },
            _ => {
                panic!("Game version not yet supported!");
            }
            // GameVersion::V102 => {},
            // GameVersion::WinStore => {},
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