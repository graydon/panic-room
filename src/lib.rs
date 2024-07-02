// Need this to actually recover propertly, I don't know how to gain access.
// #![feature(update_panic_count)]

use setjmp::{jmp_buf, longjmp, setjmp};
use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    panic::PanicInfo,
};

thread_local! {
    static NEXT_ROOM_ID: std::cell::RefCell<u64> = std::cell::RefCell::new(0);
    static ROOMS: std::cell::RefCell<Vec<Room>> = std::cell::RefCell::new(Vec::new());
}

pub struct Room {
    id: u64,
    data: Vec<RefCell<Option<Box<dyn Any>>>>,
    jmp_buf: jmp_buf,
    #[allow(dead_code)]
    old_panic_hook: Box<dyn Fn(&std::panic::PanicInfo<'_>) + Sync + Send>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Handle<T> {
    id: u64,
    index: usize,
    _phantom: std::marker::PhantomData<T>,
}

// private helpers
impl Room {
    fn push_room() -> Option<()> {
        let id = NEXT_ROOM_ID.with(|next_room_id| {
            let mut next_room_id = next_room_id.borrow_mut();
            let id = *next_room_id;
            *next_room_id += 1;
            id
        });
        ROOMS.with(|rooms| {
            let mut rooms = rooms.try_borrow_mut().ok()?;
            rooms.reserve(1);
            let new_panic_hook: Box<dyn Fn(&PanicInfo<'_>) + 'static + Sync + Send> =
                Box::new(|_panic_info| {
                    let jmp_buf = Self::current_jmp_buf();
                    if let Some(jmp_buf) = jmp_buf {
                        unsafe { longjmp(jmp_buf, 1) };
                    }
                });
            let data = Vec::new();
            let jmp_buf = unsafe { std::mem::zeroed() };
            let old_panic_hook = std::panic::take_hook();
            let room = Room {
                id,
                data,
                jmp_buf,
                old_panic_hook,
            };
            std::panic::set_hook(new_panic_hook);
            rooms.push(room);
            Some(())
        })
    }

    fn pop_room() -> Option<()> {
        ROOMS.with(|rooms| {
            let mut rooms = rooms.try_borrow_mut().ok()?;
            if let Some(_room) = rooms.pop() {
                // Can't quite recover old state here since we have to tell
                // the runtime we're not panicking, which needs access to the
                // internal panic count. feature(update_panic_count) is not
                // available to the public however.
                // std::panic::set_hook(room.old_panic_hook);
                Some(())
            } else {
                None
            }
        })
    }

    fn current_jmp_buf() -> Option<*mut jmp_buf> {
        Room::with_current_mut(|room| Some(&mut room?.jmp_buf as *mut jmp_buf))
    }
}

// public API
impl Room {
    pub fn with_current_mut<T, F>(f: F) -> Option<T>
    where
        F: FnOnce(Option<&mut Room>) -> Option<T>,
    {
        ROOMS.with(|rooms| {
            let mut rooms = rooms.try_borrow_mut().ok()?;
            let room = rooms.last_mut();
            f(room)
        })
    }

    pub fn with_current<T, F>(f: F) -> Option<T>
    where
        F: FnOnce(Option<&Room>) -> Option<T>,
    {
        ROOMS.with(|rooms| {
            let rooms = rooms.try_borrow().ok()?;
            let room = rooms.last();
            f(room)
        })
    }

    pub fn contain_panics<T, F>(f: F) -> Option<T>
    where
        F: FnOnce() -> T,
    {
        Room::push_room()?;
        let Some(jmp_buf) = Room::current_jmp_buf() else {
            Room::pop_room();
            return None;
        };
        let res = if unsafe { setjmp(jmp_buf) } == 0 {
            Some(f())
        } else {
            // Need access to this internal API to recover. I don't know
            // the right compiler magic to get it.
            // std::panicking::panic_count::decrease();
            None
        };
        Room::pop_room();
        res
    }

    pub fn alloc<T: 'static>(&mut self, value: T) -> Handle<T> {
        let index = self.data.len();
        self.data.push(RefCell::new(Some(Box::new(value))));
        Handle {
            id: self.id,
            index,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn get<T: 'static>(&self, handle: Handle<T>) -> Option<Ref<T>> {
        if handle.id == self.id {
            if let Some(r) = self.data.get(handle.index)?.try_borrow().ok() {
                Ref::filter_map(r, |any_opt| {
                    if let Some(any) = any_opt {
                        any.downcast_ref::<T>()
                    } else {
                        None
                    }
                })
                .ok()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_mut<T: 'static>(&self, handle: Handle<T>) -> Option<RefMut<T>> {
        if handle.id == self.id {
            if let Some(r) = self.data.get(handle.index)?.try_borrow_mut().ok() {
                RefMut::filter_map(r, |any_opt| {
                    if let Some(any) = any_opt {
                        any.downcast_mut::<T>()
                    } else {
                        None
                    }
                })
                .ok()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn take<T: 'static>(&mut self, handle: Handle<T>) -> Option<T> {
        if handle.id == self.id {
            self.data
                .get(handle.index)?
                .borrow_mut()
                .take()?
                .downcast()
                .ok()
                .map(|x| *x)
        } else {
            None
        }
    }
}
