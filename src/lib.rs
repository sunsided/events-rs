use std::cell::Cell;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock, Weak};

type FnEventHandlerDelegate = fn() -> ();

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
enum HandlerType {
    BoxedFn(Box<dyn Fn()>),
    BoxedFnOnce(Cell<Option<Box<dyn FnOnce()>>>),
    Function(fn() -> ()),
}

/// The actual storage type.
type MapInner = BTreeMap<HandlerKey, HandlerType>;

/// Helper type declaration for a locked `MapInner`.
type MapLocked = RwLock<MapInner>;

/// An event registration.
pub struct EventHandler {
    handlers: Arc<MapLocked>,
}

/// A handle to a registration.
/// When the handle is dropped, the registration is revoked.
pub struct Handle {
    /// The key in the map.
    key: HandlerKey,
    /// Pointer to the map that (possibly) contains the key.
    pointer: Weak<MapLocked>,
}

impl Handle {
    /// Initializes a new `Handle` from a successful registration.
    fn new(key: HandlerKey, pointer: Weak<MapLocked>) -> Self {
        Self { key, pointer }
    }

    /// Determines whether the registration is still valid.
    pub fn is_valid(&self) -> bool {
        self.pointer.strong_count() > 0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(lock) = self.pointer.upgrade() {
            let mut handlers = lock.write().unwrap();
            handlers.remove(&self.key);
        }
    }
}

impl EventHandler {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(MapLocked::new(MapInner::new())),
        }
    }

    fn add_fn<T>(&mut self, handler: Box<T>) -> Result<Handle, String>
    where
        T: Fn() -> () + 'static,
    {
        let key = HandlerKey::PtrOfBox((&handler as *const _) as usize);
        let mut handlers = self.handlers.write().unwrap();
        let entry = HandlerType::BoxedFn(handler);
        match handlers.insert(key, entry) {
            None => Ok(Handle::new(key, Arc::downgrade(&self.handlers))),
            Some(_) => Err(String::from("The handler was already registered")),
        }
    }

    fn add_fnonce<T>(&mut self, handler: Box<T>) -> Result<Handle, String>
    where
        T: FnOnce() -> () + 'static,
    {
        let key = HandlerKey::PtrOfBox((&handler as *const _) as usize);
        let mut handlers = self.handlers.write().unwrap();
        let entry = HandlerType::BoxedFnOnce(Cell::new(Some(handler)));
        match handlers.insert(key, entry) {
            None => Ok(Handle::new(key, Arc::downgrade(&self.handlers))),
            Some(_) => Err(String::from("The handler was already registered")),
        }
    }

    fn add_ptr(&mut self, handler: FnEventHandlerDelegate) -> Result<Handle, String> {
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
    pub fn invoke(&self) {
        let mut unregister_list = Vec::new();

        {
            let handlers = self.handlers.read().unwrap();
            for (key, entry) in handlers.iter() {
                match &entry {
                    HandlerType::Function(fun) => fun(),
                    HandlerType::BoxedFn(fun) => fun(),
                    HandlerType::BoxedFnOnce(cell) => {
                        let fun = cell.replace(None);
                        if fun.is_some() {
                            (fun.unwrap())();
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

/// Provides functionality to register a handler.
pub trait AddHandler<T> {
    #[must_use = "this handle must be held alive for as long as the event should be used"]
    fn add(&mut self, handler: T) -> Result<Handle, String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn dummy() {
        println!("Dummy called.");
    }

    #[test]
    fn new_handler_has_no_registrations() {
        let handler = EventHandler::new();
        assert_eq!(handler.len(), 0);
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_fn() {
        let mut handler = EventHandler::new();
        let handle = handler.add_fn(Box::new(|| dummy())).unwrap();
        assert_eq!(handler.len(), 1);
        handler.invoke();
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_fnonce() {
        let mut handler = EventHandler::new();
        let handle = handler.add_fnonce(Box::new(dummy)).unwrap();
        assert_eq!(handler.len(), 1);
        handler.invoke();
        assert_eq!(handler.len(), 0);
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_function_pointer() {
        let mut handler = EventHandler::new();
        let handle = handler.add_ptr(dummy).unwrap();
        assert_eq!(handler.len(), 1);
        handler.invoke();
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
        let handle = handler.add_fn(Box::new(|| dummy())).unwrap();
        assert_eq!(handler.len(), 1);
        drop(handle);
        assert_eq!(handler.len(), 0);
    }

    #[test]
    #[allow(dead_code, unused_variables)]
    fn it_works<'i>() {
        let x = Box::new(0);
        let y = Box::new(0);
        let addr = *x as *const usize;
        let addr2 = *x as *const usize;
        let addr3 = *y as *const usize;
        println!("{:?}", addr);

        let mut handler = EventHandler::new();
        let value = Arc::new(Cell::new(0));

        fn do_something(ptr: Arc<Cell<i32>>) {
            ptr.set(42);
        }

        let value2 = value.clone();
        let handle = handler
            .add_fn(Box::new(move || do_something(value2.clone())))
            .unwrap();

        handler.invoke();

        assert_eq!(handler.len(), 1);
        assert_eq!(value.get(), 42);
    }
}
