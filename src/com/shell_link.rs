#![allow(non_snake_case)]
use crate::types::*;
use std::ptr::null_mut;

pub fn resolve_shortcut(p: &str) -> Option<String> {
    if !p.ends_with(".lnk") && !p.ends_with(".LNK") { return None; }
    unsafe {
        let mut sl = null_mut();
        if CoCreateInstance(&CLSID_SHELL_LINK, null_mut(), 1, &IID_ISHELL_LINK_W, &mut sl) != 0 { return None; }
        
        let (v, mut pf, mut r) = (*(sl as *mut *mut IShellLinkWVtbl), null_mut(), None);
        if ((*v).QueryInterface)(sl, &IID_IPERSIST_FILE, &mut pf) == 0 {
            let pv = *(pf as *mut *mut IPersistFileVtbl);
            let mut b = [0u16; 260];
            for (i, c) in p.encode_utf16().take(259).enumerate() { b[i] = c; }
            
            if ((*pv).Load)(pf, b.as_ptr(), 0) == 0 {
                if ((*v).GetPath)(sl, b.as_mut_ptr(), 260, null_mut(), 0) == 0 {
                   let l = (0..260).find(|&i| *b.get_unchecked(i) == 0).unwrap_or(0);
                   if l > 0 { r = Some(String::from_utf16_lossy(&b[..l])); }
                }
            }
            ((*pv).Release)(pf);
        }
        ((*v).Release)(sl);
        r
    }
}
