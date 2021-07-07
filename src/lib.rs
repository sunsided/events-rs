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
    handle: Option<usize>,
    pointer: Weak<MapLocked>,
}

impl Handle {
    fn new(handle: Option<usize>, pointer: Weak<MapLocked>) -> Self {
        Self { handle, pointer }
    }

    /// Determines whether the registration is still valid.
    pub fn is_valid(&self) -> bool {
        self.pointer.strong_count() > 0
    }

    /// Determines whether this an event was newly registered or
    /// already existed.
    pub fn is_new(&self) -> bool {
        self.handle.is_some()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if self.handle.is_none() {
            return;
        }

        if let Some(map) = self.pointer.upgrade() {
            let mut guard = map.write().unwrap();
            guard.remove(&self.handle.unwrap());
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
    pub fn add(&mut self, handler: Box<EventHandlerDelegate>) -> Handle {
        let p_handler: usize = (&handler as *const _) as usize;
        let entry = HandlerEntry { handler };
        let mut guard = self.handlers.write().unwrap();
        let weak_ptr = Arc::downgrade(&self.handlers);
        match guard.insert(p_handler, entry) {
            None => Handle::new(Some(p_handler), weak_ptr),
            Some(_) => Handle::new(None, weak_ptr),
        }
    }

    pub fn invoke(&self) {
        let guard = self.handlers.read().unwrap();
        for (_, entry) in guard.iter() {
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
    fn can_add_unknown_handlers() {
        let mut handler = EventHandler::new();
        let handle = handler.add(Box::new(|| dummy()));
        assert_eq!(handle.is_new(), true);
        assert_eq!(handler.len(), 1);
    }

    #[test]
    fn can_remove_handlers() {
        let mut handler = EventHandler::new();
        let handle = handler.add(Box::new(|| dummy()));
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
