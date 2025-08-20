use std::collections::HashMap;
use std::fs;
use std::process::exit;
use tracing::{error, info};
use windows::core::PCSTR;
use windows::Win32::System::LibraryLoader::{ GetModuleHandleA, GetModuleFileNameA};
use sha2::{Sha256, Digest};
use lazy_static::lazy_static;

#[derive(Debug, Clone, Copy)]
pub enum GameVersion {
    V101,
    V102,
    WinStore,
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
    static ref HASHES : HashMap<Vec<u8>, GameVersion> = {
        let mut hashes = HashMap::new();
        // hashes.insert(vec![81, 113, 190, 208, 158, 111, 236, 123, 33, 191, 14, 164, 121, 219, 210, 225, 178, 40, 105, 92, 103, 209, 240, 180, 120, 84, 154, 155, 226, 245, 114, 106], GameVersion::V102);
        hashes.insert(vec![0xa0, 0x1a, 0xc5, 0x13, 0x2e, 0x10, 0x92, 0x52, 0xd6, 0xd9, 0xa4, 0xcb, 0xf9, 0x74, 0x61, 0x4d, 0xec, 0xfb, 0xe3, 0x23, 0x71, 0x3c, 0x1f, 0xbf, 0x5b, 0xc2, 0x48, 0xf0, 0x12, 0x61, 0x77, 0x3f], GameVersion::V101);
        hashes.insert(vec![0x51, 0x71, 0xbe, 0xd0, 0x9e, 0x6f, 0xec, 0x7b, 0x21, 0xbf, 0x0e, 0xa4, 0x79, 0xdb, 0xd2, 0xe1, 0xb2, 0x28, 0x69, 0x5c, 0x67, 0xd1, 0xf0, 0xb4, 0x78, 0x54, 0x9a, 0x9b, 0xe2, 0xf5, 0x72, 0x6a], GameVersion::V102);
        hashes.insert(vec![0x3d, 0xde, 0x56, 0x6c, 0xea, 0x3e, 0x3b, 0xc1, 0x5e, 0x45, 0x92, 0x66, 0x02, 0xfb, 0x4f, 0x24, 0xd4, 0x8f, 0x77, 0xdf, 0x8a, 0x7b, 0xc5, 0x50, 0xa5, 0xb2, 0xdc, 0xae, 0xcc, 0xcf, 0x09, 0x48], GameVersion::WinStore);
        hashes.insert(vec![0xe9, 0xef, 0x66, 0x01, 0xeb, 0x40, 0xeb, 0x0a, 0x6d, 0x3f, 0x30, 0xa6, 0x63, 0x95, 0x43, 0xec, 0x2f, 0x81, 0x71, 0xc2, 0x6a, 0x3d, 0xe8, 0xb2, 0xb1, 0x30, 0x39, 0xee, 0xbe, 0x3b, 0xc8, 0x1c], GameVersion::Debug);
        hashes
    };

    static ref OFFSETS : Offsets = unsafe {
        match get_game_version() {
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
                error!("Game version not yet supported!");
                exit(1);
            }
            // GameVersion::V102 => {},
            // GameVersion::WinStore => {},
            // GameVersion::Debug => {},
        }
    };
}

// const PLAYER_ACTOR_OFFSET : isize = 0x1020948; // this one is persistent when going back to menu
// const PLAYER_ACTOR_OFFSET : usize = 0x01020BE0;

unsafe fn get_game_version() -> GameVersion {

    let mut raw_filepath : [u8; 256] = [0; 256];

    let hmodule = GetModuleHandleA(PCSTR::null()).unwrap();
    let length = GetModuleFileNameA(hmodule, &mut raw_filepath) as usize;

    let filepath = String::from_utf8(raw_filepath[..length].to_vec()).unwrap();

    let file = fs::read(filepath).unwrap();
    let result = Sha256::digest(file).to_vec();

    info!("hash: {:x?}", result);

    let version = match HASHES.get(&result) {
        Some(&v) => v,
        None => {
            error!("Unknown game version!");
            exit(1);
        }
    };

    info!("version: {:?}", HASHES.get(&result).unwrap());

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