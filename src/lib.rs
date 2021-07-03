use std::collections::BTreeMap;

type EventHandlerDelegate = fn();

pub struct EventHandler {
    handlers: BTreeMap<usize, EventHandlerDelegate>,
}

impl EventHandler {
    pub fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, handler: EventHandlerDelegate) -> bool {
        let p_handler: usize = unsafe { *(handler as *const usize) };
        match self.handlers.insert(p_handler, handler) {
            None => true,
            Some(_) => false,
        }
    }

    pub fn remove(&mut self, handler: EventHandlerDelegate) -> bool {
        let p_handler: usize = unsafe { *(handler as *const usize) };
        match self.handlers.remove(&p_handler) {
            Some(_) => true,
            None => false,
        }
    }

    pub fn invoke(&self) {
        for (_, handler) in self.handlers.iter() {
            handler();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy() {}

    #[test]
    fn can_add_unknown_handlers() {
        let mut handler = EventHandler::new();
        assert_eq!(handler.add(dummy), true);
    }

    #[test]
    fn cannot_add_handlers_twice() {
        let mut handler = EventHandler::new();
        handler.add(dummy);
        assert_eq!(handler.add(dummy), false);
    }

    #[test]
    fn cannot_remove_unknown_handlers() {
        let mut handler = EventHandler::new();
        assert_eq!(handler.remove(dummy), false);
    }

    #[test]
    fn can_remove_known_handlers() {
        let mut handler = EventHandler::new();
        handler.add(dummy);
        assert_eq!(handler.remove(dummy), true);
        assert_eq!(handler.remove(dummy), false);
    }

    #[test]
    fn it_works() {
        let mut handler = EventHandler::new();
        let mut value = 0;
        handler.add(|| value += 1);
        handler.invoke();
        assert_eq!(value, 1);
    }
}
