use cef::*;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::sync::Arc;

use crate::client::Inner;
use crate::client_impl::os_ffi::OsKeyEvent;
use jfn_platform_abi::event_flags::{EVENTFLAG_ALT_DOWN, EVENTFLAG_CONTROL_DOWN};

fn action_modifier() -> u32 {
    jfn_platform_abi::try_get()
        .map(|p| p.display().action_modifier_flag())
        .unwrap_or(EVENTFLAG_CONTROL_DOWN)
}

fn is_raw_key_down(e: &KeyEvent) -> bool {
    let kt: sys::cef_key_event_type_t = e.type_.into();
    kt == sys::cef_key_event_type_t::KEYEVENT_RAWKEYDOWN
}

fn is_paste_shortcut(e: &KeyEvent) -> bool {
    if !is_raw_key_down(e) {
        return false;
    }
    if (e.modifiers & action_modifier()) == 0 {
        return false;
    }
    if (e.modifiers & EVENTFLAG_ALT_DOWN) != 0 {
        return false;
    }
    e.windows_key_code == b'V' as i32
}

fn mpv_cmd(args: &[&str]) {
    let storage = args
        .iter()
        .map(|s| CString::new(*s))
        .collect::<Result<Vec<_>, _>>();

    let Ok(storage) = storage else {
        return;
    };

    let ptrs: Vec<*const c_char> = storage.iter().map(|s| s.as_ptr()).collect();

    unsafe {
        jfn_mpv::api::jfn_mpv_command_async(ptrs.as_ptr(), ptrs.len());
    }
}

fn mpv_keypress(key: &str) {
    mpv_cmd(&["keypress", key]);
}

fn mpv_script_message(name: &str) {
    mpv_cmd(&["script-message", name]);
}

fn forward_lua_key(e: &KeyEvent) -> bool {
    if !is_raw_key_down(e) {
        return false;
    }

    if (e.modifiers & EVENTFLAG_ALT_DOWN) != 0 {
        return false;
    }

    let action_down = (e.modifiers & action_modifier()) != 0;

    match e.windows_key_code {
        code if code == b'T' as i32 && action_down => {
            mpv_script_message("video-tone-adjuster-toggle");
            true
        }
    
        code if code == b'Z' as i32 && action_down => {
            mpv_cmd(&["script-binding", "font_menu/toggle"]);
            true
        }
    
        0x26 => {
            mpv_cmd(&["script-binding", "font_menu/font-menu-up"]);
            true
        }
    
        0x28 => {
            mpv_cmd(&["script-binding", "font_menu/font-menu-down"]);
            true
        }
    
        0x0D => {
            mpv_cmd(&["script-binding", "font_menu/font-menu-enter"]);
            true
        }
    
        0x08 => {
            mpv_cmd(&["script-binding", "font_menu/font-menu-backspace"]);
            true
        }
    
        0x1B => {
            mpv_cmd(&["script-binding", "font_menu/font-menu-esc"]);
            true
        }
    
        code if code == b'R' as i32 => {
            mpv_keypress("r");
            true
        }
    
        0x25 => {
            mpv_keypress("LEFT");
            true
        }
    
        0x27 => {
            mpv_keypress("RIGHT");
            true
        }
    
        _ => false,
    }

wrap_keyboard_handler! {
    pub struct JfnKeyboardHandlerBuilder {
        inner: Arc<Inner>,
    }

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            _browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: OsKeyEvent<'_>,
            _is_keyboard_shortcut: Option<&mut c_int>,
        ) -> c_int {
            let Some(e) = event else { return 0 };

            // 保留原项目的粘贴逻辑。
            if is_paste_shortcut(e) {
                return if self.inner.try_paste() { 1 } else { 0 };
            }

            // 新增：把指定按键转发给 mpv/Lua。
            if forward_lua_key(e) {
                return 1;
            }

            0
        }
    }
}
