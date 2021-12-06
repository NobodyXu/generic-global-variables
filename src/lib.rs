use core::any::{Any, TypeId};
use core::marker::PhantomData;
use core::ops::Deref;

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{RwLock, RwLockUpgradableReadGuard};

#[derive(Default, Debug)]
pub struct GenericGlobal(RwLock<HashMap<TypeId, Arc<dyn Any>>>);

impl GenericGlobal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_init<T: 'static>(&self, f: impl FnOnce() -> T) -> Entry<T> {
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

        debug_assert!(option.is_none());

        Entry::new(arc)
    }
}

/// A reference to the entry
#[derive(Debug, Clone)]
pub struct Entry<T: 'static>(Arc<dyn Any>, PhantomData<T>);

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

#[cfg(test)]
mod tests {}
