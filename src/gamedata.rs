use windows::core::PCSTR;
use windows::Win32::System::LibraryLoader::GetModuleHandleA;

pub fn get_player_position() -> [f32; 3] {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: usize = unsafe { std::ptr::read((process_start + 0x1020948) as *const _) };
    if player_addr == 0 {
        return [0.0, 0.0, 0.0];
    }

    let player_pos: [f32; 3] = unsafe { std::ptr::read((player_addr + 0x50) as *const _) };

    return player_pos;
}

pub fn get_player_rotation() -> [f32; 3] {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: usize = unsafe { std::ptr::read((process_start + 0x01020BE0) as *const _) };
    if player_addr == 0 {
        return [0.0, 0.0, 0.0];
    }

    let player_rot: [f32; 3] = unsafe { std::ptr::read((player_addr + 0x90) as *const _) };

    return player_rot;
}

pub fn get_view_matrix() -> [[f32; 4]; 4] {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let view_proj: [[f32; 4]; 4] = unsafe { std::ptr::read((process_start + 0x11553B0) as *const _) };

    return view_proj;
}