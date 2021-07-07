use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, Weak};

type EventHandlerDelegate = dyn Fn();

struct HandlerEntry {
    pub handler: Box<EventHandlerDelegate>,
}

type MapInner = BTreeMap<usize, HandlerEntry>;
type MapLocked = RwLock<MapInner>;

pub struct EventHandler {
    handlers: Arc<MapLocked>,
}

/// A handle to a registration.
/// When the handle is dropped, the registration is revoked.
#[derive(Default)]
pub struct Handle {
    /// The key in the map.
    key: usize,
    /// Pointer to the map that (possibly) contains the key.
    pointer: Weak<MapLocked>,
}

impl Handle {
    /// Initializes a new `Handle` from a successful registration.
    fn new(key: usize, pointer: Weak<MapLocked>) -> Self {
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

    pub fn len(&self) -> usize {
        self.handlers.read().unwrap().len()
    }

    #[must_use = "this handle must be held alive for as long as the event should be used"]
    pub fn add(&mut self, handler: Box<EventHandlerDelegate>) -> Result<Handle, String> {
        let p_handler: usize = (&handler as *const _) as usize;
        let mut handlers = self.handlers.write().unwrap();
        let weak_ptr = Arc::downgrade(&self.handlers);
        let entry = HandlerEntry { handler };
        match handlers.insert(p_handler, entry) {
            None => Ok(Handle::new(p_handler, weak_ptr)),
            Some(_) => Err(String::from("The handler was already registered")),
        }
    }

    pub fn invoke(&self) {
        let handlers = self.handlers.read().unwrap();
        for (_, entry) in handlers.iter() {
            (entry.handler)();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy() {}

    #[test]
    fn new_handler_has_no_registrations() {
        let handler = EventHandler::new();
        assert_eq!(handler.len(), 0);
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_unknown_handlers() {
        let mut handler = EventHandler::new();
        let handle = handler.add(Box::new(|| dummy())).unwrap();
        assert_eq!(handler.len(), 1);
    }

    #[test]
    fn can_remove_handlers() {
        let mut handler = EventHandler::new();
        let handle = handler.add(Box::new(|| dummy())).unwrap();
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

        /*
        let mut handler = EventHandler::new();
        let mut value = 0;
        let value_ref: &'i mut i32 = &mut value;

        struct State<'a> {
            pub value: &'a mut i32,
        }

        fn dummy(state: &EventStateType) {
            let state: Box<dyn Any> = state.unwrap();
            let state = state.downcast_ref::<State>().unwrap();
            (*state.value) += 1;
        }

        let state = EventStateType::Some(Box::new(State { value: value_ref }));
        handler.add(dummy, state);

        handler.invoke();

        assert_eq!(value, 1);
        */
    }
}
