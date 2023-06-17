//! # Event Handling
//!
//! An .NET `event`, `EventHandler` and `EventArgs` inspired event handling system.
//!
//! ## Examples
//!
//! The following example constructs an `Event` and two handles to it.
//! It invokes the event through one of the handles (de-registering it), then on the event itself.
//!
//! ```
//! use event_handler::prelude::*;
//! use std::sync::{Arc, Mutex};
//!
//! // The values we want to mutate.
//! // These need to be Send such that the handle functions can update them.
//! let value = Arc::new(Mutex::new(0));
//! let value2 = Arc::new(Mutex::new(0));
//!
//! // Create an event handler.
//! let event = Event::new();
//!
//! // Create a closure to mutate the value.
//!
//! let update_first_value = {
//!     let first_value = value.clone();
//!     move |amount| *first_value.lock().unwrap() = amount
//! };
//!
//! // Create a closure to mutate the other value.
//! let update_second_value = {
//!     let second_value = value2.clone();
//!     move |amount| *second_value.lock().unwrap() = amount * 2
//! };
//!
//! // Register the function to the event.
//! let handle = event.add_fn(update_first_value).unwrap();
//! let _handle = event.add_fn(update_second_value).unwrap();
//!
//! // Two handlers are now registered.
//! assert_eq!(event.len(), 2);
//!
//! // Invoke the event on the handle.
//! // Since we move the handle, this will de-register the handler when the scope is left.
//! assert!(std::thread::spawn(move || { handle.invoke(41) })
//!     .join()
//!     .is_ok());
//! assert_eq!(event.len(), 1);
//! assert_eq!(*value.lock().unwrap(), 41);
//! assert_eq!(*value2.lock().unwrap(), 41 * 2);
//!
//! // Invoke the event on the event itself.
//! event.invoke(42);
//! assert_eq!(*value.lock().unwrap(), 41);         // previous value
//! assert_eq!(*value2.lock().unwrap(), 42 * 2);    // updated value
//! ```
//!
//! If the [`Event`] is dropped, calls to [`EventHandle::invoke`] will return an error.
//!
//! ```
//! # use event_handler::prelude::*;
//! # use std::sync::{Arc, Mutex};
//! let event = Event::new();
//!
//! // Register the function to the event.
//! let value = Arc::new(Mutex::new(0));
//! let handle = event.add_fn({
//!         let value = value.clone();
//!         move |amount| *value.lock().unwrap() = amount
//! }).unwrap();
//!
//! // Register another function.
//! let value2 = Arc::new(Mutex::new(0));
//! let late_handle = event.add_fn({
//!         let value = value2.clone();
//!         move |amount| *value.lock().unwrap() = amount * 2
//! }).unwrap();
//!
//! // Invoke the event on the handler itself.
//! // This will move the event, dropping it afterwards.
//! assert!(std::thread::spawn(move || {
//!     event.invoke(42);
//! })
//! .join()
//! .is_ok());
//! assert_eq!(*value.lock().unwrap(), 42);
//! assert_eq!(*value2.lock().unwrap(), 42 * 2);
//!
//! // This event invocation will fail because the event is already dropped.
//! assert_eq!(
//!     std::thread::spawn(move || { late_handle.invoke(41) })
//!         .join()
//!         .unwrap(),
//!     Err(EventInvocationError::EventDropped)
//! );
//! ```

#![allow(unsafe_code)]
#![forbid(unused_must_use)]

use std::cell::Cell;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock, Weak};

pub mod prelude {
    pub use crate::{Event, EventHandle, EventInvocationError, Invoke};
}

/// Alias for trivial function pointers.
pub type FnEventHandlerDelegate<TEventArgs> = fn(TEventArgs) -> ();

/// An event registration.
pub struct Event<TEventArgs = ()> {
    handlers: Arc<MapLocked<TEventArgs>>,
}

unsafe impl<TEventArgs: Send + Sync> Sync for Event<TEventArgs> {}

/// A concrete type of a handler.
enum HandlerType<TEventArgs> {
    BoxedFn(Box<dyn Fn(TEventArgs) + Send>),
    BoxedFnOnce(Cell<Option<Box<dyn FnOnce(TEventArgs) + Send>>>),
    Function(FnEventHandlerDelegate<TEventArgs>),
}

unsafe impl<TEventArgs: Send + Sync> Sync for HandlerType<TEventArgs> {}

/// Helper type declaration for a locked [`MapInner`].
struct MapLocked<TEventArgs>(RwLock<MapInner<TEventArgs>>);

/// The actual storage type.
type MapInner<TEventArgs> = BTreeMap<HandleKey, HandlerType<TEventArgs>>;

/// A handle to a registration.
/// When the handle is dropped, the registration is revoked.
#[must_use = "This handle must be held alive for as long as the event should be used."]
pub struct EventHandle<TEventArgs> {
    /// The key in the map.
    key: HandleKey,
    /// Pointer to the map that (possibly) contains the key.
    pointer: Weak<MapLocked<TEventArgs>>,
}

/// A key entry for a handler.
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Copy, Clone)]
enum HandleKey {
    PtrOfBox(usize),
    FunctionPointer(usize),
}

/// Hashing for [`HandleKey`] instances.
impl Hash for HandleKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            HandleKey::PtrOfBox(ptr) => {
                let address = ptr as *const usize as usize;
                address.hash(state)
            }
            HandleKey::FunctionPointer(ptr) => {
                let address = ptr as *const usize as usize;
                address.hash(state)
            }
        }
    }
}

impl<TEventArgs> EventHandle<TEventArgs> {
    /// Initializes a new `Handle` from a successful registration.
    fn new(key: HandleKey, pointer: &Arc<MapLocked<TEventArgs>>) -> Self {
        Self {
            key,
            pointer: Arc::downgrade(pointer),
        }
    }

    /// Determines whether the registration is still valid.
    pub fn is_valid(&self) -> bool {
        self.pointer.strong_count() > 0
    }

    /// Invokes the event with the specified arguments.
    ///
    /// ## Arguments
    /// * `args` - The event arguments to pass.
    pub fn invoke(&self, args: TEventArgs) -> Result<(), EventInvocationError>
    where
        TEventArgs: Clone,
    {
        if let Some(ptr) = self.pointer.upgrade() {
            ptr.invoke(args);
            Ok(())
        } else {
            Err(EventInvocationError::EventDropped)
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum EventInvocationError {
    /// The event was dropped.
    EventDropped,
}

impl Display for EventInvocationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EventInvocationError::EventDropped => write!(
                f,
                "Event could not be invoked because it was already dropped"
            ),
        }
    }
}

impl Error for EventInvocationError {}

impl<TEventArgs> Drop for EventHandle<TEventArgs> {
    fn drop(&mut self) {
        if let Some(lock) = self.pointer.upgrade() {
            let mut handlers = lock.write().unwrap();
            handlers.remove(&self.key);
        }
    }
}

impl<TEventArgs> Event<TEventArgs> {
    pub fn new() -> Self
    where
        TEventArgs: Clone,
    {
        Self {
            handlers: Arc::new(MapLocked::new(MapInner::new())),
        }
    }

    pub fn add_fn<T>(&self, handler: T) -> Result<EventHandle<TEventArgs>, String>
    where
        T: Fn(TEventArgs) -> () + Send + 'static,
    {
        let handler = Box::new(handler);
        let key = HandleKey::PtrOfBox((&*handler as *const _) as usize);
        let mut handlers = self.handlers.write().unwrap();
        let entry = HandlerType::BoxedFn(handler);
        match handlers.insert(key, entry) {
            None => Ok(EventHandle::new(key, &self.handlers)),
            Some(_) => Err(String::from("The handler was already registered")),
        }
    }

    pub fn add_fnonce<T>(&self, handler: T) -> Result<EventHandle<TEventArgs>, String>
    where
        T: FnOnce(TEventArgs) -> () + Send + 'static,
    {
        let handler = Box::new(handler);
        let key = HandleKey::PtrOfBox((&*handler as *const _) as usize);
        let mut handlers = self.handlers.write().unwrap();
        let entry = HandlerType::BoxedFnOnce(Cell::new(Some(handler)));
        match handlers.insert(key, entry) {
            None => Ok(EventHandle::new(key, &self.handlers)),
            Some(_) => Err(String::from("The handler was already registered")),
        }
    }

    pub fn add_ptr(
        &self,
        handler: FnEventHandlerDelegate<TEventArgs>,
    ) -> Result<EventHandle<TEventArgs>, String> {
        let key = HandleKey::FunctionPointer((&handler as *const _) as usize);
        let mut handlers = self.handlers.write().unwrap();
        let entry = HandlerType::Function(handler);
        match handlers.insert(key, entry) {
            None => Ok(EventHandle::new(key, &self.handlers)),
            Some(_) => Err(String::from("The handler was already registered")),
        }
    }

    /// Returns the number of currently registered handlers.
    pub fn len(&self) -> usize {
        self.handlers.read().unwrap().len()
    }

    /// Invokes the event.
    ///
    /// ## Arguments
    /// * `args` - The event arguments.
    pub fn invoke(&self, args: TEventArgs)
    where
        TEventArgs: Clone,
    {
        self.handlers.invoke(args)
    }
}

impl Default for Event {
    fn default() -> Self {
        Self::new()
    }
}

impl<TEventArgs> MapLocked<TEventArgs>
where
    TEventArgs: Clone,
{
    const fn new(inner: MapInner<TEventArgs>) -> Self {
        Self(RwLock::new(inner))
    }

    fn invoke(&self, args: TEventArgs) {
        let mut unregister_list = Vec::new();

        {
            let handlers = self.read().unwrap();
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
            let mut handlers = self.write().unwrap();
            for key in unregister_list {
                handlers.remove(&key);
            }
        }
    }
}

impl<TEventArgs> Deref for MapLocked<TEventArgs> {
    type Target = RwLock<MapInner<TEventArgs>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<TEventArgs> DerefMut for MapLocked<TEventArgs> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Provides the `invoke` function for an event.
pub trait Invoke<TEventArgs>
where
    TEventArgs: Clone,
{
    fn invoke(&self, args: TEventArgs);
}

impl<TEventArgs> Invoke<TEventArgs> for Event<TEventArgs>
where
    TEventArgs: Clone,
{
    fn invoke(&self, args: TEventArgs) {
        self.invoke(args)
    }
}

impl<TEventArgs> Invoke<TEventArgs> for EventHandle<TEventArgs>
where
    TEventArgs: Clone,
{
    fn invoke(&self, args: TEventArgs) {
        self.invoke(args).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn dummy(_args: ()) {
        println!("Dummy called.");
    }

    #[test]
    fn new_handler_has_no_registrations() {
        let handler = Event::<()>::new();
        assert_eq!(handler.len(), 0);
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_fn() {
        let handler = Event::<()>::new();
        let handle = handler.add_fn(dummy).unwrap();
        assert_eq!(handler.len(), 1);
        handler.invoke(());
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_fnonce() {
        let handler = Event::new();
        let handle = handler.add_fnonce(dummy).unwrap();
        assert_eq!(handler.len(), 1);
        handler.invoke(());
        assert_eq!(handler.len(), 0);
    }

    #[test]
    #[allow(unused_variables)]
    fn can_add_function_pointer() {
        let handler = Event::<()>::new();
        let handle = handler.add_ptr(dummy).unwrap();
        assert_eq!(handler.len(), 1);
        handler.invoke(());
    }

    #[test]
    #[allow(unused_variables)]
    fn cannot_register_same_function_twice() {
        let handler = Event::new();
        let handle = handler.add_ptr(dummy).unwrap();
        assert!(handler.add_ptr(dummy).is_err());
    }

    #[test]
    fn can_remove_handlers() {
        let handler = Event::new();
        let handle = handler.add_fn(dummy).unwrap();
        assert_eq!(handler.len(), 1);
        drop(handle);
        assert_eq!(handler.len(), 0);
    }

    #[test]
    fn handler_is_sync() {
        let handler: Event = Event::new();
        let _sync: Box<dyn Sync> = Box::new(handler);
    }

    #[test]
    fn wtf() {
        // The values we want to mutate.
        // These need to be Send such that the handle functions can update them.
        let value = Arc::new(Mutex::new(0));
        let value2 = Arc::new(Mutex::new(0));

        // Create an event handler.
        let event = Event::new();

        // Create a closure to mutate the value.

        let update_first_value = {
            let first_value = value.clone();
            move |amount| *first_value.lock().unwrap() = amount
        };
        // Create a closure to mutate the other value.
        let update_second_value = {
            let second_value = value2.clone();
            move |amount| *second_value.lock().unwrap() = amount * 2
        };

        // Register the function to the event.
        let handle = event.add_fn(update_first_value).unwrap();
        let _handle = event.add_fn(update_second_value).unwrap();

        // Two handlers are now registered.
        assert_eq!(event.len(), 2);

        // Invoke the event on the handle.
        assert!(std::thread::spawn(move || { handle.invoke(41) })
            .join()
            .is_ok());
        assert_eq!(*value.lock().unwrap(), 41);
        assert_eq!(*value2.lock().unwrap(), 41 * 2);

        // One handler deregistered.
        assert_eq!(event.len(), 1);

        // Invoke the event on the event itself.
        event.invoke(42);
        assert_eq!(*value.lock().unwrap(), 41);
        assert_eq!(*value2.lock().unwrap(), 42 * 2);
    }
}
