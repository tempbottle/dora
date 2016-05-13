use libc;

use std::ffi::CStr;
use std::io::{self, Write};
use std::mem;
use std::os::raw::c_char;
use std::slice;
use std::str;
use ctxt::get_ctxt;
use mem::ptr::Ptr;
use object::{Handle, IntArray, Str2};

pub extern "C" fn assert(val: bool) {
    if !val {
        unsafe {
            println!("assert failed");
            libc::_exit(101);
        }
    }
}

pub extern "C" fn int_to_string(val: i32) -> Handle<Str2> {
    let buffer = val.to_string();
    Str2::from(buffer.as_bytes())
}

pub extern "C" fn bool_to_string(val: bool) -> Handle<Str2> {
    let val = if val {
        "true"
    } else {
        "false"
    };

    Str2::from(val.as_bytes())
}

pub extern "C" fn bool_to_int(val: bool) -> i32 {
    if val { 1 } else { 0 }
}

pub extern "C" fn print(val: Handle<Str2>) {
    unsafe {
        let buf = CStr::from_ptr(val.data() as *const c_char);
        io::stdout().write(buf.to_bytes()).unwrap();
    };
}

pub extern "C" fn println(val: Handle<Str2>) {
    print(val);
    println!("");
}

pub extern "C" fn strcmp(lhs: Handle<Str2>, rhs: Handle<Str2>) -> i32 {
    unsafe {
        libc::strcmp(lhs.data() as *const i8, rhs.data() as *const i8)
    }
}

pub extern "C" fn strcat(lhs: Handle<Str2>, rhs: Handle<Str2>) -> Handle<Str2> {
    Str2::concat(lhs, rhs)
}

pub extern "C" fn gc_alloc(size: usize) -> Ptr {
    let ctxt = get_ctxt();
    let mut gc = ctxt.gc.lock().unwrap();

    gc.alloc(size)
}

pub extern "C" fn gc_collect() {
    let ctxt = get_ctxt();
    let mut gc = ctxt.gc.lock().unwrap();

    gc.collect();
}

pub extern "C" fn ctor_int_array_empty(ptr: Ptr) -> Ptr {
    let data = IntArray::empty();

    unsafe { *(ptr.raw() as *mut IntArray) = data; }

    ptr
}

pub extern "C" fn ctor_int_array_elem(ptr: Ptr, len: i32, value: i32) -> Ptr {
    let ctxt = get_ctxt();
    let mut gc = ctxt.gc.lock().unwrap();

    let data = IntArray::with_element(&mut gc, len as usize, value as isize);

    unsafe { *(ptr.raw() as *mut IntArray) = data; }

    ptr
}

pub extern "C" fn int_array_len(ptr: *const IntArray) -> i32 {
    let array = unsafe { &*ptr };

    array.len() as i32
}

pub extern "C" fn str_array_len(s: Handle<Str2>) -> i32 {
    s.len() as i32
}

pub extern "C" fn argc() -> i32 {
    let ctxt = get_ctxt();

    if let Some(ref args) = ctxt.args.arg_argument {
        args.len() as i32
    } else {
        0
    }
}

pub extern "C" fn argv(ind: i32) -> Handle<Str2> {
    let ctxt = get_ctxt();

    if let Some(ref args) = ctxt.args.arg_argument {
        if ind >= 0 && ind < args.len() as i32 {
            let value = &args[ind as usize];

            return Str2::from(value.as_bytes());
        }
    }

    panic!("argument does not exist");
}

pub extern "C" fn parse(val: Handle<Str2>) -> i32 {
    let slice = unsafe { slice::from_raw_parts(val.data(), val.len()) };
    let val = str::from_utf8(slice).unwrap();

    val.parse::<i32>().unwrap_or(0)
}
