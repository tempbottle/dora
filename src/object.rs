use std;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::slice;

use class::ClassId;
use ctxt::Context;
use gc::root::IndirectObj;
use mem;
use ty::BuiltinType;
use vtable::VTable;

pub struct Header {
    // ptr to class
    vtable: *mut VTable,

    // additional information>
    // bit 0 - marked flag
    info: usize,

    // next allocated object
    next: *mut Obj,
}

impl Header {
    pub fn size() -> i32 {
        std::mem::size_of::<Header>() as i32
    }

    pub fn vtblptr(&self) -> *mut VTable {
        self.vtable
    }

    pub fn vtbl(&self) -> &mut VTable {
        unsafe { &mut *self.vtable }
    }

    pub fn set_mark(&mut self, value: bool) {
        self.info = if value {
            self.info | 1
        } else {
            self.info & (!1)
        };
    }

    pub fn marked(&self) -> bool {
        if (self.info & 1) != 0 { true } else { false }
    }

    pub fn set_succ(&mut self, ptr: *mut Obj) {
        self.next = ptr;
    }

    pub fn succ(&self) -> *mut Obj {
        self.next
    }
}

// is used to reference any object
pub struct Obj {
    header: Header,
    data: u8,
}

impl Obj {
    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    pub fn data(&self) -> *const u8 {
        &self.data as *const u8
    }

    pub fn size(&self) -> usize {
        let size = self.header().vtbl().class().size;
        if size > 0 {
            return size as usize;
        }

        let ty = self.header().vtbl().class().ty;

        match ty {
            BuiltinType::Str => {
                let handle: Handle<Str> = Handle { ptr: self as *const Obj as *const Str };
                handle.size()
            }

            BuiltinType::BoolArray => {
                let handle: Handle<BoolArray> =
                    Handle { ptr: self as *const Obj as *const BoolArray };
                handle.size()
            }

            BuiltinType::ByteArray => {
                let handle: Handle<ByteArray> =
                    Handle { ptr: self as *const Obj as *const ByteArray };
                handle.size()
            }

            BuiltinType::IntArray => {
                let handle: Handle<IntArray> =
                    Handle { ptr: self as *const Obj as *const IntArray };
                handle.size()
            }

            BuiltinType::LongArray => {
                let handle: Handle<LongArray> =
                    Handle { ptr: self as *const Obj as *const LongArray };
                handle.size()
            }

            BuiltinType::FloatArray => {
                let handle: Handle<FloatArray> =
                    Handle { ptr: self as *const Obj as *const FloatArray };
                handle.size()
            }

            BuiltinType::DoubleArray => {
                let handle: Handle<DoubleArray> =
                    Handle { ptr: self as *const Obj as *const DoubleArray };
                handle.size()
            }

            _ => panic!("size unknown"),
        }
    }

    pub fn visit_reference_fields<F>(&mut self, mut f: F)
        where F: FnMut(IndirectObj)
    {
        let classptr = self.header().vtbl().classptr;
        let cls = unsafe { &*classptr };

        for &offset in &cls.ref_fields {
            let obj = (self as *mut Obj as usize) + offset as usize;
            f(obj.into());
        }
    }
}

pub struct Handle<T> {
    ptr: *const T,
}

impl<T> Handle<T> {
    pub fn raw(&self) -> *const T {
        self.ptr
    }
}

// known limitation of #[derive(Copy, Clone)]
// traits need to be implemented manually
impl<T> Copy for Handle<T> {}
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Handle<T> {
        *self
    }
}

impl<T> Deref for Handle<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for Handle<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *(self.ptr as *mut T) }
    }
}

impl<T> Into<Handle<T>> for usize {
    fn into(self) -> Handle<T> {
        Handle { ptr: self as *const T }
    }
}

pub struct Str {
    header: Header,
    length: usize,
    data: u8,
}

impl Str {
    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn data(&self) -> *const u8 {
        &self.data as *const u8
    }

    pub fn to_cstring(&self) -> CString {
        let view = unsafe { slice::from_raw_parts(self.data(), self.len()) };

        CString::new(view).unwrap()
    }

    pub fn size(&self) -> usize {
        Header::size() as usize         // Object header
            + mem::ptr_width() as usize // length field
            + self.len() + 1   // string content
    }

    /// allocates string from buffer in permanent space
    pub fn from_buffer_in_perm(ctxt: &Context, buf: &[u8]) -> Handle<Str> {
        let mut handle = str_alloc_perm(ctxt, buf.len());
        handle.length = buf.len();

        unsafe {
            let data = handle.data() as *mut u8;

            // copy buffer content into Str
            ptr::copy_nonoverlapping(buf.as_ptr(), data, buf.len());

            // string should end with 0 for C compatibility
            *(data.offset(buf.len() as isize)) = 0;
        }

        handle
    }

    /// allocates string from buffer in permanent space
    pub fn from_buffer(ctxt: &Context, buf: &[u8]) -> Handle<Str> {
        let mut handle = str_alloc_heap(ctxt, buf.len());
        handle.length = buf.len();

        unsafe {
            let data = handle.data() as *mut u8;

            // copy buffer content into Str
            ptr::copy_nonoverlapping(buf.as_ptr(), data, buf.len());

            // string should end with 0 for C compatibility
            *(data.offset(buf.len() as isize)) = 0;
        }

        handle
    }

    pub fn concat(ctxt: &Context, lhs: Handle<Str>, rhs: Handle<Str>) -> Handle<Str> {
        let len = lhs.len() + rhs.len();
        let mut handle = str_alloc_heap(ctxt, len);

        unsafe {
            handle.length = len;

            ptr::copy_nonoverlapping(lhs.data(), handle.data() as *mut u8, lhs.len());
            ptr::copy_nonoverlapping(rhs.data(),
                                     handle.data().offset(lhs.len() as isize) as *mut u8,
                                     rhs.len());

            *(handle.data().offset(len as isize) as *mut u8) = 0;
        }

        handle
    }
}

fn str_alloc_heap(ctxt: &Context, len: usize) -> Handle<Str> {
    str_alloc(ctxt,
              len,
              |ctxt, size| ctxt.gc.lock().unwrap().alloc(ctxt, size) as *const u8)
}

fn str_alloc_perm(ctxt: &Context, len: usize) -> Handle<Str> {
    str_alloc(ctxt,
              len,
              |ctxt, size| ctxt.gc.lock().unwrap().alloc_perm(size))
}

fn str_alloc<F>(ctxt: &Context, len: usize, alloc: F) -> Handle<Str>
    where F: FnOnce(&Context, usize) -> *const u8
{
    let size = Header::size() as usize     // Object header
                + mem::ptr_width() as usize // length field
                + len + 1;                  // string content

    let ptr = alloc(ctxt, size) as usize;

    let clsid = ctxt.primitive_classes.str_class;
    let cls = ctxt.classes[clsid].borrow();
    let vtable: *const VTable = &**cls.vtable.as_ref().unwrap();
    let mut handle: Handle<Str> = ptr.into();
    handle.header_mut().vtable = vtable as *mut VTable;

    handle
}

pub struct Array<T: Copy> {
    header: Header,
    length: usize,
    data: u8,
    phantom: PhantomData<T>,
}

impl<T> Array<T>
    where T: Copy
{
    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn data(&self) -> *const T {
        &self.data as *const u8 as *const T
    }

    pub fn data_mut(&mut self) -> *mut T {
        &self.data as *const u8 as *mut T
    }

    pub fn size(&self) -> usize {
        Header::size() as usize         // Object header
            + mem::ptr_width() as usize // length field
            + self.len() * std::mem::size_of::<T>() // array content
    }

    pub fn alloc(ctxt: &Context, len: usize, elem: T, clsid: ClassId) -> Handle<Array<T>> {
        let size = Header::size() as usize        // Object header
                   + mem::ptr_width() as usize    // length field
                   + len * std::mem::size_of::<T>(); // array content

        let ptr = ctxt.gc.lock().unwrap().alloc(ctxt, size) as usize;
        let cls = ctxt.classes[clsid].borrow();
        let vtable: *const VTable = &**cls.vtable.as_ref().unwrap();
        let mut handle: Handle<Array<T>> = ptr.into();
        handle.header_mut().vtable = vtable as *mut VTable;
        handle.length = len;

        for i in 0..handle.len() {
            unsafe {
                *handle.data_mut().offset(i as isize) = elem;
            }
        }

        handle
    }
}

pub fn offset_of_array_length() -> i32 {
    offset_of!(Array<i32>, length) as i32
}

pub fn offset_of_array_data() -> i32 {
    offset_of!(Array<i32>, data) as i32
}

pub type BoolArray = Array<bool>;
pub type ByteArray = Array<u8>;
pub type CharArray = Array<char>;
pub type IntArray = Array<i32>;
pub type LongArray = Array<i64>;
pub type FloatArray = Array<f32>;
pub type DoubleArray = Array<f64>;

pub fn char_array_empty(ctxt: &Context) -> Handle<CharArray> {
    let clsid = ctxt.primitive_classes.char_array;
    Array::alloc(ctxt, 0, '\0', clsid)
}

pub fn char_array_with(ctxt: &Context, len: usize, elem: char) -> Handle<CharArray> {
    let clsid = ctxt.primitive_classes.char_array;
    Array::alloc(ctxt, len, elem, clsid)
}

pub fn int_array_empty(ctxt: &Context) -> Handle<IntArray> {
    let clsid = ctxt.primitive_classes.int_array;
    Array::alloc(ctxt, 0, 0, clsid)
}

pub fn int_array_with(ctxt: &Context, len: usize, elem: i32) -> Handle<IntArray> {
    let clsid = ctxt.primitive_classes.int_array;
    Array::alloc(ctxt, len, elem, clsid)
}

pub fn byte_array_empty(ctxt: &Context) -> Handle<ByteArray> {
    let clsid = ctxt.primitive_classes.int_array;
    Array::alloc(ctxt, 0, 0, clsid)
}

pub fn byte_array_with(ctxt: &Context, len: usize, elem: u8) -> Handle<ByteArray> {
    let clsid = ctxt.primitive_classes.int_array;
    Array::alloc(ctxt, len, elem, clsid)
}

pub fn bool_array_empty(ctxt: &Context) -> Handle<BoolArray> {
    let clsid = ctxt.primitive_classes.bool_array;
    Array::alloc(ctxt, 0, false, clsid)
}

pub fn bool_array_with(ctxt: &Context, len: usize, elem: bool) -> Handle<BoolArray> {
    let clsid = ctxt.primitive_classes.bool_array;
    Array::alloc(ctxt, len, elem, clsid)
}

pub fn long_array_empty(ctxt: &Context) -> Handle<LongArray> {
    let clsid = ctxt.primitive_classes.long_array;
    Array::alloc(ctxt, 0, 0, clsid)
}

pub fn long_array_with(ctxt: &Context, len: usize, elem: i64) -> Handle<LongArray> {
    let clsid = ctxt.primitive_classes.long_array;
    Array::alloc(ctxt, len, elem, clsid)
}

pub fn float_array_empty(ctxt: &Context) -> Handle<FloatArray> {
    let clsid = ctxt.primitive_classes.float_array;
    Array::alloc(ctxt, 0, 0f32, clsid)
}

pub fn float_array_with(ctxt: &Context, len: usize, elem: f32) -> Handle<FloatArray> {
    let clsid = ctxt.primitive_classes.float_array;
    Array::alloc(ctxt, len, elem, clsid)
}

pub fn double_array_empty(ctxt: &Context) -> Handle<DoubleArray> {
    let clsid = ctxt.primitive_classes.double_array;
    Array::alloc(ctxt, 0, 0f64, clsid)
}

pub fn double_array_with(ctxt: &Context, len: usize, elem: f64) -> Handle<DoubleArray> {
    let clsid = ctxt.primitive_classes.double_array;
    Array::alloc(ctxt, len, elem, clsid)
}
