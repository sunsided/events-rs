#![forbid(unsafe_code)]
#![forbid(unused_must_use)]

use std::cell::Cell;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock, Weak};

/// Alias for trivial function pointers.
pub type FnEventHandlerDelegate<TEventArgs> = fn(TEventArgs) -> ();

/// An event registration.
pub struct EventHandler<TEventArgs = ()> {
    handlers: Arc<MapLocked<TEventArgs>>,
}

/// A key entry for a handler.
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Copy, Clone)]
enum HandlerKey {
    PtrOfBox(usize),
    FunctionPointer(usize),
}

/// Hashing for `HandlerKey` instances.
impl Hash for HandlerKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            HandlerKey::PtrOfBox(ptr) => ptr.hash(state),
            HandlerKey::FunctionPointer(ptr) => ptr.hash(state),
        }
    }
}

/// A concrete type of a handler.
enum HandlerType<TEventArgs> {
    BoxedFn(Box<dyn Fn(TEventArgs)>),
    BoxedFnOnce(Cell<Option<Box<dyn FnOnce(TEventArgs)>>>),
    Function(FnEventHandlerDelegate<TEventArgs>),
}

/// The actual storage type.
type MapInner<TEventArgs> = BTreeMap<HandlerKey, HandlerType<TEventArgs>>;

/// Helper type declaration for a locked `MapInner`.
type MapLocked<TEventArgs> = RwLock<MapInner<TEventArgs>>;

/// A handle to a registration.
/// When the handle is dropped, the registration is revoked.
#[must_use = "This handle must be held alive for as long as the event should be used."]
pub struct Handle<TEventArgs> {
    /// The key in the map.
    key: HandlerKey,
    /// Pointer to the map that (possibly) contains the key.
    pointer: Weak<MapLocked<TEventArgs>>,
}

impl<TEventArgs> Handle<TEventArgs> {
    /// Initializes a new `Handle` from a successful registration.
    fn new(key: HandlerKey, pointer: Weak<MapLocked<TEventArgs>>) -> Self {
        Self { key, pointer }
    }

    /// Determines whether the registration is still valid.
    pub fn is_valid(&self) -> bool {
        self.pointer.strong_count() > 0
    }
}

impl<TEventArgs> Drop for Handle<TEventArgs> {
    fn drop(&mut self) {
        if let Some(lock) = self.pointer.upgrade() {
            let mut handlers = lock.write().unwrap();
            handlers.remove(&self.key);
        }
    }
}

impl<TEventArgs> EventHandler<TEventArgs> {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(MapLocked::new(MapInner::new())),
        }
    }

    pub fn add_fn<T>(&mut self, handler: T) -> Result<Handle<TEventArgs>, String>
    where
        T: Fn(TEventArgs) -> () + 'static,
    {
        let handler = Box::new(handler);
        let key = HandlerKey::PtrOfBox((&handler as *const _) as usize);
        let mut handlers = self.handlers.write().unwrap();
        let entry = HandlerType::BoxedFn(handler);
        match handlers.insert(key, entry) {
            None => Ok(Handle::new(key, Arc::downgrade(&self.handlers))),
            Some(_) => Err(String::from("The handler was already registered")),
        }
    }

    pub fn add_fnonce<T>(&mut self, handler: T) -> Result<Handle<TEventArgs>, String>
    where
        T: FnOnce(TEventArgs) -> () + 'static,
    {
        let handler = Box::new(handler);
        let key = HandlerKey::PtrOfBox((&handler as *const _) as usize);
        let mut handlers = self.handlers.write().unwrap();
        let entry = HandlerType::BoxedFnOnce(Cell::new(Some(handler)));
        match handlers.insert(key, entry) {
            None => Ok(Handle::new(key, Arc::downgrade(&self.handlers))),
            Some(_) => Err(String::from("The handler was already registered")),
        }
    }

    pub fn add_ptr(&mut self, handler: FnEventHandlerDelegate<TEventArgs>) -> Result<Handle<TEventArgs>, String> {
        let key = HandlerKey::FunctionPointer((&handler as *const _) as usize);
        let mut handlers = self.handlers.write().unwrap();
        let entry = HandlerType::Function(handler);
        match handlers.insert(key, entry) {
            None => Ok(Handle::new(key, Arc::downgrade(&self.handlers))),
            Some(_) => Err(String::from("The handler was already registered")),
        }
    }

    /// Returns the number of currently registered handlers.
    pub fn len(&self) -> usize {
        self.handlers.read().unwrap().len()
    }

    /// Invokes the event.
    // TODO: Add event data and sender information.
    pub fn invoke(&self, args: TEventArgs) where TEventArgs: Clone {
        let mut unregister_list = Vec::new();

        {
            let handlers = self.handlers.read().unwrap();
            for (key, entry) in handlers.iter() {
                let args = args.clone();
                match &entry {
                    HandlerType::Function(fun) => fun(args),
                    HandlerType::BoxedFn(fun) => fun(args),
                    HandlerType::BoxedFnOnce(cell) => {
                        let fun = cell.replace(None);
                        if fun.is_some() {
                            (fun.unwrap())(args);
                        }
                        unregister_list.push(key.clone());
                    }
                }
            }
        }

        // Clean up after any FnOnce type.
        if !unregister_list.is_empty() {
            let mut handlers = self.handlers.write().unwrap();
            for key in unregister_list {
                handlers.remove(&key);
            }
        }
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy(_args: ()) {
        println!("Dummy called.");
    }

    #[test]
    fn new_handler_has_no_registrations() {
        let handler = EventHandler::<()>::new();
        assert_eq!(handler.len(), 0);
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_fn() {
        let mut handler = EventHandler::<()>::new();
        let handle = handler.add_fn(dummy).unwrap();
        assert_eq!(handler.len(), 1);
        handler.invoke(());
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_fnonce() {
        let mut handler = EventHandler::new();
        let handle = handler.add_fnonce(dummy).unwrap();
        assert_eq!(handler.len(), 1);
        handler.invoke(());
        assert_eq!(handler.len(), 0);
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_function_pointer() {
        let mut handler = EventHandler::<()>::new();
        let handle = handler.add_ptr(dummy).unwrap();
        assert_eq!(handler.len(), 1);
        handler.invoke(());
    }

    #[test]
    #[allow(unused_variables)]
    fn cannot_register_same_function_twice() {
        let mut handler = EventHandler::new();
        let handle = handler.add_ptr(dummy).unwrap();
        assert!(handler.add_ptr(dummy).is_err());
    }

    #[test]
    fn can_remove_handlers() {
        let mut handler = EventHandler::new();
        let handle = handler.add_fn(dummy).unwrap();
        assert_eq!(handler.len(), 1);
        drop(handle);
        assert_eq!(handler.len(), 0);
    }
}
