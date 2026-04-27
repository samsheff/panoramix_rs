//! Mask operations for bitfield extraction


/// Convert a type name to its bit mask
pub fn type_to_mask(s: &str) -> Option<u64> {
    match s {
        "bool" => Some(1),
        "uint8" | "int8" => Some(8),
        "uint16" | "int16" => Some(16),
        "uint32" | "int32" => Some(32),
        "uint64" | "int64" => Some(64),
        "uint128" | "int128" => Some(128),
        "address" => Some(160),
        "uint256" | "int256" | "uint" | "int" => Some(256),
        _ => None,
    }
}

/// Convert a mask size to a type name
pub fn mask_to_type(num: u64, force: bool) -> Option<String> {
    let lookup: [(u64, &str); 8] = [
        (1, "bool"),
        (8, "uint8"),
        (16, "uint16"),
        (32, "uint32"),
        (64, "uint64"),
        (128, "uint128"),
        (160, "address"),
        (256, "uint256"),
    ];
    
    for (mask, name) in lookup.iter() {
        if *mask == num {
            return Some(name.to_string());
        }
    }
    
    if force && num > 256 {
        return Some(format!("big{}", num));
    }
    
    None
}

/// Find a mask that encompasses the number
pub fn find_mask(num: u64) -> Option<(u64, u64)> {
    if num == 0 { return Some((0, 0)); }
    
    let mut i = 0;
    while i < 64 && (num >> i & 1) == 0 { i += 1; }
    let mask_pos = i - i % 8;
    
    let mut mask_pos_plus_len = 64;
    while i < 64 {
        if (num >> i & 1) != 0 {
            mask_pos_plus_len = i - i % 8 + 8;
        }
        i += 1;
    }
    
    while mask_pos_plus_len < 64 && (num >> mask_pos_plus_len & 1) != 0 {
        mask_pos_plus_len += 8;
    }
    
    let len = mask_pos_plus_len - mask_pos;
    if len == 0 || mask_pos >= 64 { return None; }
    
    Some((len, mask_pos))
}
