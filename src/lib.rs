#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::error::Error;
use std::ffi::{CStr, CString};
use std::fmt::{Debug, Display, Formatter};
use std::mem;
use std::os::raw::{c_char, c_int};

include!(concat!(env!("OUT_DIR"), "/deadbeef_bindings.rs"));

const MAX_LEN: u16 = 256;

static mut IS_PLUGIN_ENABLED: bool = false;
static mut DEADBEEF: Option<&DB_functions_t> = None;

fn format() -> Result<String, Box<dyn Error>> {
    let deadbeef = unsafe { DEADBEEF.try_get_api()? };

    let now_playing = PlayingTrack::new();
    if now_playing.0.is_null() {
        return Ok("no song playing".to_string());
    }

    let now_playing_playlist = PlayingPlaylist::new();

    let mut buf: [c_char; MAX_LEN as usize] = [0; MAX_LEN as usize];
    unsafe {
        deadbeef.conf_get_str.unwrap()(
            CStr::from_bytes_with_nul(b"now_playing_to_file.format\0")
                .unwrap()
                .as_ptr(),
            CStr::from_bytes_with_nul(b"%title%$if(%ispaused%,' ('paused')')\0")
                .unwrap()
                .as_ptr(),
            buf.as_mut_ptr(),
            MAX_LEN as c_int,
        );
    }

    let script = FormatScript::new(&buf);
    if script.0.is_null() {
        return Ok(String::new());
    }

    let mut context = ddb_tf_context_t {
        _size: mem::size_of::<ddb_tf_context_t>() as c_int,
        flags: 0,
        it: now_playing.0,
        plt: now_playing_playlist.0,
        idx: 0,
        id: 0,
        iter: PL_MAIN as c_int,
        update: 0,
        dimmed: 0,
    };

    let mut out: [c_char; MAX_LEN as usize] = [0; MAX_LEN as usize];
    unsafe {
        deadbeef.tf_eval.unwrap()(&mut context, script.0, out.as_mut_ptr(), MAX_LEN as c_int);
    }

    let out = unsafe { String::from(CStr::from_ptr(out.as_ptr()).to_str()?) };

    Ok(out)
}

fn write_to_file() -> Result<(), Box<dyn Error>> {
    let deadbeef = unsafe { DEADBEEF.try_get_api()? };

    let mut buf: [c_char; MAX_LEN as usize] = [0; MAX_LEN as usize];
    unsafe {
        deadbeef.conf_get_str.unwrap()(
            CStr::from_bytes_with_nul(b"now_playing_to_file.out_path\0")
                .unwrap()
                .as_ptr(),
            CStr::from_bytes_with_nul(b"\0").unwrap().as_ptr(),
            buf.as_mut_ptr(),
            MAX_LEN as c_int,
        );
    }

    if buf[0] == 0 {
        return Ok(());
    }

    let path = unsafe { CStr::from_ptr(buf.as_ptr()).to_str()? };
    std::fs::write(path, format()?)?;

    Ok(())
}

#[no_mangle]
unsafe extern "C" fn now_playing_to_file_start() -> c_int {
    IS_PLUGIN_ENABLED = DEADBEEF.unwrap().conf_get_int.unwrap()(
        CStr::from_bytes_with_nul(b"now_playing_to_file.enable\0")
            .unwrap()
            .as_ptr(),
        0,
    ) == 1;

    0
}

#[no_mangle]
unsafe extern "C" fn now_playing_to_file_stop() -> c_int {
    IS_PLUGIN_ENABLED = false;
    0
}

#[no_mangle]
unsafe extern "C" fn now_playing_to_file_message(
    id: u32,
    _ctx: usize,
    _p1: u32,
    _p2: u32,
) -> c_int {
    match id {
        DB_EV_SONGCHANGED | DB_EV_PAUSED | DB_EV_STOP => {
            if IS_PLUGIN_ENABLED {
                write_to_file().unwrap_or_else(|err| {
                    let err_string = match CString::new(
                        String::from("now_playing_to_file: ") + &err.to_string() + "\n",
                    ) {
                        Ok(val) => val,
                        Err(err) => {
                            eprintln!("{err}");
                            return;
                        }
                    };
                    DEADBEEF.unwrap().log.unwrap()(err_string.as_ptr())
                });
            }
        }
        DB_EV_CONFIGCHANGED => {
            IS_PLUGIN_ENABLED = DEADBEEF.unwrap().conf_get_int.unwrap()(
                CStr::from_bytes_with_nul(b"now_playing_to_file.enable\0")
                    .unwrap()
                    .as_ptr(),
                0,
            ) == 1;
        }
        _ => (),
    };

    0
}

const CONFIG: &str = "property \"Enable\" checkbox now_playing_to_file.enable 1;\n\
    property \"Format\" entry now_playing_to_file.format \"%title%$if(%ispaused%,' ('paused')')\";\n\
    property \"Output path\" entry now_playing_to_file.out_path \"\";\n\
    \0";

const START: Option<unsafe extern "C" fn() -> c_int> = Some(now_playing_to_file_start);
const STOP: Option<unsafe extern "C" fn() -> c_int> = Some(now_playing_to_file_stop);
const MESSAGE: Option<unsafe extern "C" fn(u32, usize, u32, u32) -> c_int> =
    Some(now_playing_to_file_message);
const PLUGIN: DB_misc_t = DB_misc_t {
    plugin: DB_plugin_t {
        type_: DB_PLUGIN_MISC as i32,
        api_vmajor: 1,
        api_vminor: 16,
        id: "now_playing_to_file\0".as_ptr() as *const c_char,
        name: "Now Playing to File Plugin\0".as_ptr() as *const c_char,
        descr: "Outputs a formatted string based on your current playing track to a file of your choice.\0".as_ptr() as *const c_char,
        copyright:
        "Now Playing to File Plugin for DeaDBeeF\n\
        Copyright (C) 2023 Tassad <Tassadaritze@gmail.com>\n\
        \n\
        This program is free software: you can redistribute it and/or modify\n\
        it under the terms of the GNU General Public License as published by\n\
        the Free Software Foundation, either version 3 of the License, or\n\
        (at your option) any later version.\n\
        \n\
        This program is distributed in the hope that it will be useful,\n\
        but WITHOUT ANY WARRANTY; without even the implied warranty of\n\
        MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the\n\
        GNU General Public License for more details.\n\
        \n\
        You should have received a copy of the GNU General Public License\n\
        along with this program.  If not, see <https://www.gnu.org/licenses/>.\n\
        \0".as_ptr() as *const c_char,
        website: "https://github.com/Tassadaritze/deadbeef_now_playing_to_file\0".as_ptr() as *const c_char,
        start: START,
        stop: STOP,
        message: MESSAGE,
        configdialog: CONFIG.as_ptr() as *const c_char,
        version_major: 1,
        version_minor: 0,
        flags: 0,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
        command: None,
        connect: None,
        disconnect: None,
        exec_cmdline: None,
        get_actions: None,
    },
};

#[no_mangle]
pub fn now_playing_to_file_load(api: &'static DB_functions_t) -> &DB_plugin_t {
    unsafe { DEADBEEF = Some(api) };

    &PLUGIN.plugin
}

#[derive(Debug)]
struct NowPlayingError(String);

impl Display for NowPlayingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for NowPlayingError {}

trait TryAPI {
    fn try_get_api(&self) -> Result<&DB_functions_t, Box<dyn Error>>;
}

impl TryAPI for Option<&DB_functions_t> {
    fn try_get_api(&self) -> Result<&DB_functions_t, Box<dyn Error>> {
        unsafe {
            match DEADBEEF {
                Some(val) => Ok(val),
                None => Err(Box::new(NowPlayingError(String::from(
                    "couldn't get plugin api pointer",
                )))),
            }
        }
    }
}

#[repr(C)]
struct PlayingTrack(*mut ddb_playItem_t);

impl PlayingTrack {
    fn new() -> Self {
        unsafe {
            Self(DEADBEEF
                .try_get_api()
                .unwrap()
                .streamer_get_playing_track_safe
                .unwrap()())
        }
    }
}

impl Drop for PlayingTrack {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                DEADBEEF.try_get_api().unwrap().pl_item_unref.unwrap()(self.0);
            }
        }
    }
}

#[repr(C)]
struct PlayingPlaylist(*mut ddb_playlist_t);

impl PlayingPlaylist {
    fn new() -> Self {
        unsafe { Self(DEADBEEF.try_get_api().unwrap().plt_get_curr.unwrap()()) }
    }
}

impl Drop for PlayingPlaylist {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                DEADBEEF.try_get_api().unwrap().plt_unref.unwrap()(self.0);
            }
        }
    }
}

#[repr(C)]
struct FormatScript(*mut c_char);

impl FormatScript {
    fn new(buf: &[c_char]) -> Self {
        unsafe {
            Self(DEADBEEF.try_get_api().unwrap().tf_compile.unwrap()(
                buf.as_ptr(),
            ))
        }
    }
}

impl Drop for FormatScript {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                DEADBEEF.try_get_api().unwrap().tf_free.unwrap()(self.0);
            }
        }
    }
}
