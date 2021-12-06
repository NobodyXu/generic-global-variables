use core::any::{Any, TypeId};
use core::fmt;
use core::marker::PhantomData;
use core::ops::Deref;

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{RwLock, RwLockUpgradableReadGuard};

/// ```
/// use once_cell::sync::OnceCell;
/// use generic_global_variables::*;
///
/// use std::thread::spawn;
/// use std::sync::Mutex;
///
/// fn get_buffer<T: Send + Sync>(f: impl FnOnce() -> T) -> Entry<T> {
///     static GLOBALS: OnceCell<GenericGlobal> = OnceCell::new();
///
///     let globals = GLOBALS.get_or_init(GenericGlobal::new);
///     globals.get_or_init(f)
/// }
///
/// let handles1: Vec<_> = (0..24).map(|_| {
///     spawn(|| {
///         let arc = get_buffer(Mutex::<Vec::<Box<[u8]>>>::default);
///         let buffer = arc.lock()
///             .unwrap()
///             .pop()
///             .unwrap_or_else(|| vec![0 as u8; 20].into_boxed_slice());
///         // Read some data into buffer and process it
///         // ...
///
///         arc.lock().unwrap().push(buffer);
///     })
/// }).collect();
///
/// let handles2: Vec<_> = (0..50).map(|_| {
///     spawn(|| {
///         let arc = get_buffer(Mutex::<Vec::<Box<[u32]>>>::default);
///         let buffer = arc.lock()
///             .unwrap()
///             .pop()
///             .unwrap_or_else(|| vec![1 as u32; 20].into_boxed_slice());
///         // Read some data into buffer and process it
///         // ...
///
///         arc.lock().unwrap().push(buffer);
///     })
/// }).collect();
///
/// for handle in handles1 {
///     handle.join();
/// }
///
/// for handle in handles2 {
///     handle.join();
/// }
/// ```
#[derive(Default, Debug)]
pub struct GenericGlobal(RwLock<HashMap<TypeId, Arc<dyn Any>>>);

impl GenericGlobal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_init<T: 'static + Send + Sync>(&self, f: impl FnOnce() -> T) -> Entry<T> {
        let typeid = TypeId::of::<T>();

        if let Some(val) = self.0.read().get(&typeid) {
            return Entry::new(Arc::clone(val));
        }

        // Use an upgradable_read to check if the key has already
        // been added by another thread.
        //
        // Unlike write guard, this UpgradableReadGuard only blocks
        // other UpgradableReadGuard and WriteGuard, so the readers
        // will not be blocked while ensuring that there is no other
        // writer.
        let guard = self.0.upgradable_read();

        // If another writer has already added that typeid, return.
        if let Some(val) = guard.get(&typeid) {
            return Entry::new(Arc::clone(val));
        }

        // If no other writer has added that typeid, add one now.
        let mut guard = RwLockUpgradableReadGuard::upgrade(guard);
        let arc: Arc<dyn Any> = Arc::new(f());
        let option = guard.insert(typeid, Arc::clone(&arc));

        // There cannot be any other write that insert the key.
        debug_assert!(option.is_none());

        Entry::new(arc)
    }
}

unsafe impl Send for GenericGlobal {}
unsafe impl Sync for GenericGlobal {}

/// A reference to the entry
#[derive(Debug)]
pub struct Entry<T: 'static>(Arc<dyn Any>, PhantomData<T>);

unsafe impl<T: 'static + Send + Sync> Send for Entry<T> {}
unsafe impl<T: 'static + Send + Sync> Sync for Entry<T> {}

impl<T: 'static> Clone for Entry<T> {
    fn clone(&self) -> Self {
        Self::new(self.0.clone())
    }
}

impl<T: 'static> Entry<T> {
    fn new(arc: Arc<dyn Any>) -> Self {
        Self(arc, PhantomData)
    }
}

impl<T: 'static> Deref for Entry<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        <dyn Any>::downcast_ref::<T>(&*self.0).unwrap()
    }
}

impl<T: 'static + fmt::Display> fmt::Display for Entry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.deref(), f)
    }
}

impl<T: 'static> fmt::Pointer for Entry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {}
