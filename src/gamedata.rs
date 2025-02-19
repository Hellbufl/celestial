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

pub fn set_player_position(new_pos: [f32; 3]) {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: usize = unsafe { std::ptr::read((process_start + 0x1020948) as *const _) };
    if player_addr == 0 { return }

    unsafe {
        std::ptr::write((player_addr + 0x50) as *mut _, new_pos);
    };
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

pub fn set_player_rotation(new_rot: [f32; 3]) {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: usize = unsafe { std::ptr::read((process_start + 0x01020BE0) as *const _) };
    if player_addr == 0 { return }

    unsafe {
        std::ptr::write((player_addr + 0x90) as *mut _, new_rot);
    };
}

pub fn get_view_matrix() -> [[f32; 4]; 4] {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let view_proj: [[f32; 4]; 4] = unsafe { std::ptr::read((process_start + 0x11553B0) as *const _) };

    return view_proj;
}

pub fn get_camera_rotation() -> [f32; 2] {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let cam_rot: [f32; 2] = unsafe { std::ptr::read((process_start + 0x1020C60) as *const _) };

    return cam_rot;
}

pub fn set_camera_rotation(new_rot: [f32; 2]) {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    unsafe {
        std::ptr::write((process_start + 0x1020C60) as *mut _, new_rot);
    };
}

pub fn get_player_state() -> [u32; 2] {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: usize = unsafe { std::ptr::read((process_start + 0x01020BE0) as *const _) };
    if player_addr == 0 {
        return [0, 0];
    }

    let player_state: [u32; 2] = unsafe { std::ptr::read((player_addr + 0x670) as *const _) };

    return player_state;
}

pub fn set_player_state(new_state: [u32; 2]) {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: usize = unsafe { std::ptr::read((process_start + 0x01020BE0) as *const _) };
    if player_addr == 0 { return }

    unsafe {
        std::ptr::write((player_addr + 0x670) as *mut _, new_state);
    };
}

// Testing

pub fn get_player_actor() -> usize {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: usize = unsafe { std::ptr::read((process_start + 0x1020948) as *const _) };

    return player_addr;
}

pub fn get_player_handle() -> u32 {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: u32 = unsafe { std::ptr::read((process_start + 0x125025C) as *const _) };

    return player_addr;
}

// pub fn get_noclip_state() -> u32 {
//     let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

//     let player_addr: u32 = unsafe { std::ptr::read((process_start + 0x1020948) as *const _) };
//     if player_addr == 0 {
//         return 0;
//     }

//     let noclip_state: [u32; 1] = unsafe { std::ptr::read((player_addr + 0x14E0) as *const _) };

//     return noclip_state[0];
// }

pub fn get_noclip_state() -> u32 {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: usize = unsafe { std::ptr::read((process_start + 0x1020948) as *const _) };
    if player_addr == 0 {
        return 0;
    }

    let noclip_state: u32 = unsafe { std::ptr::read((player_addr + 0x14E0) as *const _) };

    return noclip_state;
}

pub fn teleport_player(location: [f32; 3], rotation: [f32; 3]) {
    let process_start = unsafe { GetModuleHandleA(PCSTR::null()).unwrap().0 };

    let player_addr: usize = unsafe { std::ptr::read((process_start + 0x1020948) as *const _) };
    if player_addr == 0 {
        return;
    }

    let teleport_function = unsafe {std::mem::transmute::<_, extern "C" fn(usize, [f32; 3], [f32; 3], i32, f32)>(process_start + 0x4f0EA0) };

    teleport_function(player_addr, location, rotation, 1, 0.);
}