use cef::{Browser, ImplListValue, ImplProcessMessage, ProcessMessage};
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::sync::Arc;

use crate::cef_string::userfree_to_string;
use crate::client::Inner;
use crate::ipc::{list_int, list_string, BrowserMessage};

fn mpv_command(args: &[&str]) {
    let storage = match args
        .iter()
        .map(|s| CString::new(*s))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(_) => return,
    };

    let ptrs: Vec<*const c_char> = storage.iter().map(|s| s.as_ptr()).collect();
    unsafe {
        jfn_mpv::api::jfn_mpv_command_async(ptrs.as_ptr(), ptrs.len());
    }
}

fn mpv_mouse_position(x: i32, y: i32) {
    let xs = x.to_string();
    let ys = y.to_string();
    mpv_command(&["mouse", &xs, &ys]);
}

fn mpv_mouse_key(button: i32) -> Option<&'static str> {
    match button {
        // DOM MouseEvent.button: 0=left, 1=middle, 2=right.
        0 => Some("MOUSE_BTN0"),
        1 => Some("MOUSE_BTN1"),
        2 => Some("MOUSE_BTN2"),
        _ => None,
    }
}

fn handle_mpv_mouse(args: Option<&cef::ListValue>) {
    let Some(args) = args else { return };
    if args.size() < 3 {
        return;
    }

    let event = list_string(args, 0);
    let x = list_int(args, 1);
    let y = list_int(args, 2);
    let button = if args.size() > 3 { list_int(args, 3) } else { -1 };

    // Always update mpv's mouse position first. Lua scripts using
    // mp.get_mouse_pos()/mouse-pos then see the same coordinates as CEF.
    mpv_mouse_position(x, y);

    match event.as_str() {
        "move" => {}
        "down" => {
            if let Some(key) = mpv_mouse_key(button) {
                mpv_command(&["keydown", key]);
            }
        }
        "up" => {
            if let Some(key) = mpv_mouse_key(button) {
                mpv_command(&["keyup", key]);
            }
        }
        "click" => {
            if let Some(key) = mpv_mouse_key(button) {
                mpv_command(&["keypress", key]);
            }
        }
        "dblclick" => {
            if let Some(key) = mpv_mouse_key(button) {
                let dbl = format!("{key}_DBL");
                mpv_command(&["keypress", &dbl]);
            }
        }
        _ => {}
    }
}

pub(super) fn on_process_message_received(
    inner: &Arc<Inner>,
    browser: Option<&mut Browser>,
    message: Option<&mut ProcessMessage>,
) -> c_int {
    let Some(msg) = message else { return 0 };
    let name = userfree_to_string(&msg.name());
    let args = msg.argument_list();
    match name.as_str() {
        // JavaScript -> CEF renderer -> browser process -> libmpv input bridge.
        // Expected arguments: event, x, y, button.
        "mpvMouse" => {
            handle_mpv_mouse(args.as_ref());
            1
        }
        "popupOptions" => {
            if let Some(args) = args {
                let opts = if let Some(list) = args.list(0) {
                    let n = list.size();
                    let mut v = Vec::with_capacity(n);
                    for i in 0..n {
                        v.push(userfree_to_string(&list.string(i)));
                    }
                    v
                } else {
                    Vec::new()
                };
                let selected = args.int(1);
                let selectable = if let Some(list) = args.list(2) {
                    let n = list.size();
                    let mut v = Vec::with_capacity(n);
                    for i in 0..n {
                        v.push(list.int(i));
                    }
                    v
                } else {
                    Vec::new()
                };
                let anchor = (args.int(5) != 0).then(|| (args.int(3), args.int(4)));
                inner.set_popup_options(opts, selected, selectable, anchor);
            }
            1
        }
        n if crate::window_controls::is_window_message(n) => {
            crate::window_controls::handle_window_op(n, args.as_ref(), browser);
            1
        }
        _ => {
            let browser = browser.map(|b| b.clone());
            let message = BrowserMessage::new(name, args, browser);
            if inner.invoke_message_handler(message) {
                1
            } else {
                0
            }
        }
    }
}
