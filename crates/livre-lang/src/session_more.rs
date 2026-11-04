//! Session utilities: `diary`, `more`, `inmem`, `memory`, `localfunctions`, `canUseGPU`,
//! `canUseParallelPool`, `superiorto` and `inferiorto`.

use crate::builtins::{none, one, Ret};
use crate::error::rt;
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;

pub const NAMES: &[&str] = &["diary", "more", "inmem", "memory", "localfunctions", "canUseGPU", "canUseParallelPool", "superiorto", "inferiorto"];

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    match name {
        "canUseGPU" | "canUseParallelPool" => one(Value::bool(false)),
        // There is no pager: output is never held back.
        "more" => match a.first() {
            None => none(),
            Some(v) if v.is_text() => match v.text()?.as_str() {
                "on" | "off" => none(),
                _ => rt("Argument must be 'on', 'off', or a number."),
            },
            Some(v) => {
                v.scalar()?;
                none()
            }
        },
        "diary" => {
            match a.first() {
                None => it.diary.0 = !it.diary.0,
                Some(v) => {
                    let t = v.text()?;
                    match t.as_str() {
                        "on" => it.diary.0 = true,
                        "off" => it.diary.0 = false,
                        _ => it.diary = (true, t),
                    }
                }
            }
            none()
        }
        "inmem" => {
            let col = |names: Vec<String>| {
                let n = names.len();
                Value::Cell(CellArray::from_dims(vec![n, 1], names.iter().map(|s| Value::chars(s)).collect()))
            };
            let (funcs, classes) = it.loaded_names();
            Ok(Some(vec![col(funcs), col(Vec::new()), col(classes)].into_iter().take(nargout.max(1)).collect()))
        }
        "memory" => memory(it, nargout),
        "localfunctions" => {
            let names = it.local_function_names();
            let n = names.len();
            one(Value::Cell(CellArray::from_dims(vec![n, if n == 0 { 0 } else { 1 }], names.into_iter().map(|s| Value::Func(FuncValue::Named(s))).collect())))
        }
        "superiorto" | "inferiorto" => {
            if !it.in_method() {
                return rt(format!("Function '{}' can only be called from within a method.", name.to_uppercase()));
            }
            none()
        }
        _ => Ok(None),
    }
}

fn st(fields: &[(&str, Value)]) -> Value {
    let mut s = StructArray::scalar();
    for (k, v) in fields {
        s.set(k, v.clone());
    }
    Value::Struct(s)
}

fn memory(it: &mut Interpreter, nargout: usize) -> Ret {
    let Some(m) = sys::status() else {
        return rt("Function MEMORY is not available on this platform.");
    };
    let b = |x: u64| Value::num(x as f64);
    if nargout == 0 {
        let line = |label: &str, bytes: u64, star: bool| {
            let mb = (bytes as f64 / 1_048_576.0).round();
            format!("{label:<33}{mb:>7} MB ({:>8} bytes){}\n", g3(bytes as f64), if star { " *" } else { "" })
        };
        let mut s = String::new();
        s += &line("Maximum possible array:", m.avail_page, true);
        s += &line("Memory available for all arrays:", m.avail_page, true);
        s += &line("Memory used by MATLAB:", m.used, false);
        s += &line("Physical Memory (RAM):", m.total_phys, false);
        s += "\n*  Limited by System Memory (physical + swap file) available.\n";
        it.emit(OutputKind::Text(s));
        return none();
    }
    let user = st(&[("MaxPossibleArrayBytes", b(m.avail_page)), ("MemAvailableAllArrays", b(m.avail_page)), ("MemUsedMATLAB", b(m.used))]);
    let system = st(&[
        ("VirtualAddressSpace", st(&[("Available", b(m.avail_virtual)), ("Total", b(m.total_virtual))])),
        ("SystemMemory", st(&[("Available", b(m.avail_page))])),
        ("PhysicalMemory", st(&[("Available", b(m.avail_phys)), ("Total", b(m.total_phys))])),
    ]);
    Ok(Some(vec![user, system].into_iter().take(nargout).collect()))
}

/// `%.3g` for a byte count.
fn g3(x: f64) -> String {
    if x < 1000.0 {
        return format!("{x}");
    }
    let mut e = x.log10().floor() as i32;
    let mut m = x / 10f64.powi(e);
    if format!("{m:.2}").starts_with("10") {
        m /= 10.0;
        e += 1;
    }
    let t = format!("{m:.2}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    format!("{t}e+{e:02}")
}

mod sys {
    pub struct Status {
        pub total_phys: u64,
        pub avail_phys: u64,
        pub avail_page: u64,
        pub total_virtual: u64,
        pub avail_virtual: u64,
        pub used: u64,
    }

    #[cfg(windows)]
    pub fn status() -> Option<Status> {
        #[repr(C)]
        struct MemoryStatusEx {
            length: u32,
            load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page: u64,
            avail_page: u64,
            total_virtual: u64,
            avail_virtual: u64,
            avail_ext: u64,
        }
        #[repr(C)]
        struct Counters {
            cb: u32,
            page_faults: u32,
            peak_working_set: usize,
            working_set: usize,
            quota_peak_paged: usize,
            quota_paged: usize,
            quota_peak_nonpaged: usize,
            quota_nonpaged: usize,
            pagefile: usize,
            peak_pagefile: usize,
            private: usize,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalMemoryStatusEx(buf: *mut MemoryStatusEx) -> i32;
            fn GetCurrentProcess() -> isize;
            fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
        }
        let mut m = MemoryStatusEx { length: std::mem::size_of::<MemoryStatusEx>() as u32, load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0, total_virtual: 0, avail_virtual: 0, avail_ext: 0 };
        // SAFETY: both structs are laid out as the Win32 API expects and their size fields are set.
        let ok = unsafe { GlobalMemoryStatusEx(&mut m) } != 0;
        if !ok {
            return None;
        }
        let mut c: Counters = unsafe { std::mem::zeroed() };
        c.cb = std::mem::size_of::<Counters>() as u32;
        let used = if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) } != 0 { c.private as u64 } else { 0 };
        Some(Status { total_phys: m.total_phys, avail_phys: m.avail_phys, avail_page: m.avail_page, total_virtual: m.total_virtual, avail_virtual: m.avail_virtual, used })
    }

    /// MATLAB's `memory` is Windows-only.
    #[cfg(not(windows))]
    pub fn status() -> Option<Status> {
        None
    }
}
